use ahash::AHashMap;
use cddb::{CdDBDispatcher, WriteCommand};
use dualcache_ff::{DualCacheFF as RawCache, Config};
use io_oi_core::{EpochId, SignedRecord, StateStore, ScopedKey};

pub type Hash32 = [u8; 32];

#[cfg(not(loom))]
use std::sync::Arc;
#[cfg(loom)]
use loom::sync::Arc;
use dashmap::DashMap;

/// Pure Cache Backend using the high-performance DualCache-FF
#[derive(Clone)]
pub struct PureCacheStore {
    cache: Arc<RawCache<ScopedKey, SignedRecord>>,
    namespace: Hash32,
    // To support get_by_epoch, we maintain a secondary index. 
    // In a production system, this might be optimized.
    epoch_index: Arc<DashMap<EpochId, Vec<Hash32>>>,
}

impl PureCacheStore {
    pub fn new(namespace: Hash32, config: Config) -> Self {
        Self {
            cache: Arc::new(RawCache::new(config)),
            namespace,
            epoch_index: Arc::new(DashMap::new()),
        }
    }
}

impl StateStore for PureCacheStore {
    fn get_record(&self, hash: &Hash32) -> Option<SignedRecord> {
        let key = ScopedKey {
            namespace: self.namespace,
            key: hash.to_vec(),
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
            key: hash.to_vec(),
        };

        let epoch = record.epoch_id;
        self.cache.insert(key.clone(), record); 

        self.epoch_index.entry(epoch).or_default().push(hash);
    }

    fn get_by_epoch(&self, epoch_id: EpochId) -> Vec<SignedRecord> {
        if let Some(hashes) = self.epoch_index.get(&epoch_id) {
            hashes.value().iter()
                .filter_map(|h| {
                    let key = ScopedKey {
                        namespace: self.namespace,
                        key: h.to_vec(),
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
        self.epoch_index.retain(|&epoch, _| {
            if epoch > current_epoch { return true; }
            current_epoch - epoch <= k
        });
    }
}

/// Tiered Storage Backend using DualCache-FF + cdDB
#[derive(Clone)]
pub struct TieredStore {
    cache: PureCacheStore,
    db_tx: tokio::sync::mpsc::Sender<WriteCommand>,
    route: cddb::PartitionRoute,
    partition_name: String,
}

impl TieredStore {
    pub fn new(namespace: Hash32, config: Config, db: &mut CdDBDispatcher, partition: String) -> Self {
        let tx = db.register_partition(partition.clone());
        let route = db.get_route(&partition).unwrap().clone();
        Self {
            cache: PureCacheStore::new(namespace, config),
            db_tx: tx,
            route,
            partition_name: partition,
        }
    }
}

impl StateStore for TieredStore {
    fn get_record(&self, hash: &Hash32) -> Option<SignedRecord> {
        // 1. Try cache first
        if let Some(record) = self.cache.get_record(hash) {
            return Some(record);
        }

        // 2. Cache miss, try cdDB
        let entity_id = {
            let mut h = 0usize;
            for i in 0..8 {
                h = h.wrapping_shl(8) | (hash[i] as usize);
            }
            h
        };

        let snapshot = self.route.get_snapshot();
        if let Some(ptr) = snapshot.get(&entity_id) {
            let col_payload = self.route.get_column_bytes("payload")?;
            let col_epoch = self.route.get_column_int("epoch")?;
            let col_type = self.route.get_column_int("type")?;

            let payload_idx = ptr.attribute_indices.get("payload")?;
            let epoch_idx = ptr.attribute_indices.get("epoch")?;
            let type_idx = ptr.attribute_indices.get("type")?;

            let payload = col_payload.data.read().get(*payload_idx)?.as_ref()?.clone();
            let epoch = *col_epoch.data.read().get(*epoch_idx)?.as_ref()? as EpochId;
            let record_type = *col_type.data.read().get(*type_idx)?.as_ref()?;

            Some(SignedRecord {
                epoch_id: epoch,
                payload,
                judge_signature: [0u8; 64],
                record_type,
            })
        } else {
            None
        }
    }

    fn apply_signed_record(&self, record: SignedRecord) {
        // 1. Write to Cache (Write-through)
        self.cache.apply_signed_record(record.clone());

        // 2. Write to cdDB (Async Persistence)
        let attrs = AHashMap::new();
        let mut attrs_bytes = AHashMap::new();
        attrs_bytes.insert("payload".to_string(), record.payload.clone());
        
        let mut attrs_int = AHashMap::new();
        attrs_int.insert("epoch".to_string(), record.epoch_id as u32);
        attrs_int.insert("type".to_string(), record.record_type);

        // Derive entity_id from the record payload hash (same as PureCacheStore)
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

        let entity_id = {
            let mut h = 0usize;
            for i in 0..8 {
                h = h.wrapping_shl(8) | (hash[i] as usize);
            }
            h
        };

        // Backpressure: if the channel is full, we drop or warn without blocking.
        match self.db_tx.try_send(WriteCommand::Insert {
            entity_id,
            attributes: attrs,
            attributes_int: attrs_int,
            attributes_bytes: attrs_bytes,
        }) {
            Ok(_) => {}
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!("cdDB queue full! Dropping persistence for hash in partition {}", self.partition_name);
            }
            Err(e) => {
                tracing::error!("Failed to persist record to cdDB: {:?}", e);
            }
        }
    }

    fn get_by_epoch(&self, epoch_id: EpochId) -> Vec<SignedRecord> {
        self.cache.get_by_epoch(epoch_id)
    }

    fn prune(&self, current_epoch: EpochId, k: u64) {
        self.cache.prune(current_epoch, k);
    }
}
