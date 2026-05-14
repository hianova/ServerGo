use ahash::AHashMap;
use cdDB::{CdDBDispatcher, WriteCommand};
use dualcache_ff::{DualCacheFF as RawCache, Config};
use io_oi_core::{EpochId, SignedRecord, StateStore, ScopedKey};

pub type Hash32 = [u8; 32];

#[cfg(not(loom))]
use std::sync::{Arc, RwLock};
#[cfg(loom)]
use loom::sync::{Arc, RwLock};

/// Pure Cache Backend using the high-performance DualCache-FF
#[derive(Clone)]
pub struct PureCacheStore {
    cache: Arc<RawCache<ScopedKey, SignedRecord>>,
    namespace: Hash32,
    // To support get_by_epoch, we maintain a secondary index. 
    // In a production system, this might be optimized.
    epoch_index: Arc<RwLock<AHashMap<EpochId, Vec<Hash32>>>>,
}

impl PureCacheStore {
    pub fn new(namespace: Hash32, config: Config) -> Self {
        Self {
            cache: Arc::new(RawCache::new(config)),
            namespace,
            epoch_index: Arc::new(RwLock::new(AHashMap::new())),
        }
    }
}

impl StateStore for PureCacheStore {
    fn get_record(&self, hash: &Hash32) -> Option<SignedRecord> {
        let key = ScopedKey {
            namespace: self.namespace,
            record_hash: *hash,
        };
        self.cache.get(&key)
    }

    fn apply_signed_record(&self, record: SignedRecord) {
        // Simple heuristic for key extraction from payload
        let hash = if record.record_type == 100 && record.payload.len() >= 32 {
            let mut h = [0u8; 32];
            h.copy_from_slice(&record.payload[0..32]);
            h
        } else {
            let mut hasher = ahash::AHasher::default();
            use std::hash::Hasher;
            hasher.write(&record.payload);
            let h_u64 = hasher.finish();
            let mut h = [0u8; 32];
            h[..8].copy_from_slice(&h_u64.to_le_bytes());
            h
        };

        let key = ScopedKey {
            namespace: self.namespace,
            record_hash: hash,
        };

        let epoch = record.epoch_id;
        self.cache.insert(key.clone(), record); 

        let mut index = self.epoch_index.write().unwrap();
        index.entry(epoch).or_default().push(hash);
    }

    fn get_by_epoch(&self, epoch_id: EpochId) -> Vec<SignedRecord> {
        let index = self.epoch_index.read().unwrap();
        if let Some(hashes) = index.get(&epoch_id) {
            hashes.iter()
                .filter_map(|h| {
                    let key = ScopedKey {
                        namespace: self.namespace,
                        record_hash: *h,
                    };
                    self.cache.get(&key)
                })
                .collect()
        } else {
            vec![]
        }
    }

    fn prune(&self, current_epoch: EpochId, k: u64) {
        // Prune the epoch index
        let mut index = self.epoch_index.write().unwrap();
        index.retain(|&epoch, _| {
            if epoch > current_epoch { return true; }
            current_epoch - epoch <= k
        });
    }
}

/// Tiered Storage Backend using DualCache-FF + cdDB
#[derive(Clone)]
pub struct TieredStore {
    cache: PureCacheStore,
    db_tx: crossbeam::channel::Sender<WriteCommand>,
    partition_name: String,
}

impl TieredStore {
    pub fn new(namespace: Hash32, config: Config, db: &mut CdDBDispatcher, partition: String) -> Self {
        let tx = db.register_partition(partition.clone());
        Self {
            cache: PureCacheStore::new(namespace, config),
            db_tx: tx,
            partition_name: partition,
        }
    }
}

impl StateStore for TieredStore {
    fn get_record(&self, hash: &Hash32) -> Option<SignedRecord> {
        // Always try cache first (Read-aside)
        self.cache.get_record(hash)
        // TODO: If miss, read from cdDB
    }

    fn apply_signed_record(&self, record: SignedRecord) {
        // 1. Write to Cache (Write-through)
        self.cache.apply_signed_record(record.clone());

        // 2. Write to cdDB (Async Persistence)
        let mut attrs = AHashMap::new();
        attrs.insert("payload".to_string(), hex::encode(&record.payload));
        
        let mut attrs_int = AHashMap::new();
        attrs_int.insert("epoch".to_string(), record.epoch_id as u32);
        attrs_int.insert("type".to_string(), record.record_type);

        // We use a dummy entity_id for now, or hash of record
        let entity_id = 0; // TODO: Real entity ID mapping

        let _ = self.db_tx.send(WriteCommand::Insert {
            entity_id,
            attributes: attrs,
            attributes_int: attrs_int,
        });
    }

    fn get_by_epoch(&self, epoch_id: EpochId) -> Vec<SignedRecord> {
        self.cache.get_by_epoch(epoch_id)
    }

    fn prune(&self, current_epoch: EpochId, k: u64) {
        self.cache.prune(current_epoch, k);
    }
}
