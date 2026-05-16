use ahash::AHashMap;
use cdDB::{
    Attributes, CdDBDispatcher, PartitionRoute, UserWriter, WorkerState,
    WriteCommand,
};
use dashmap::DashMap;
use dualcache_ff::{Config, DualCacheFF as RawCache};
use io_oi_core::{EpochId, ScopedKey, SignedRecord, StateStore};
use std::sync::Arc;
use std::sync::atomic::Ordering;

pub type Hash32 = [u8; 32];

/// Pure Cache Backend using the high-performance DualCache-FF
#[derive(Clone)]
pub struct PureCacheStore {
    cache: Arc<RawCache<ScopedKey, SignedRecord>>,
    namespace: Hash32,
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
        self.epoch_index.retain(|&epoch, _| {
            if epoch > current_epoch { return true; }
            current_epoch - epoch <= k
        });
    }
}

/// Tiered Storage Backend using cdDB 0.2.0 (Wait-Free RCU + Native Blob)
#[derive(Clone)]
pub struct TieredStore {
    cache: PureCacheStore,
    db_writer: Arc<UserWriter>,
    route: PartitionRoute,
    worker: Arc<WorkerState>,
    partition_name: String,
}

impl TieredStore {
    pub fn new(namespace: Hash32, config: Config, db: &mut CdDBDispatcher, partition: String) -> Self {
        // cdDB 0.2.0 register_partition returns a synchronous UserWriter
        let writer = db.register_partition(partition.clone());
        let route = db.get_route(&partition).unwrap().clone();
        let worker = route.register_worker();
        
        Self {
            cache: PureCacheStore::new(namespace, config),
            db_writer: Arc::new(writer),
            route,
            worker,
            partition_name: partition,
        }
    }
}

// 內部讀取工具，處理 cdDB 0.2.0 的 AtomicPtr (Wait-Free RCU)
mod cddb_internal {
    use super::*;
    pub fn load_ref<'a, T>(ptr: &std::sync::atomic::AtomicPtr<T>) -> &'a T {
        let p = ptr.load(Ordering::Acquire);
        unsafe { &*p }
    }
}

impl StateStore for TieredStore {
    fn get_record(&self, hash: &Hash32) -> Option<SignedRecord> {
        // 1. Try cache first
        if let Some(record) = self.cache.get_record(hash) {
            return Some(record);
        }

        // 2. Cache miss, try cdDB
        let mut hasher = ahash::AHasher::default();
        use std::hash::Hasher;
        hasher.write(hash);
        let entity_id = hasher.finish() as usize;

        // Use QSBR worker for safe wait-free reading
        self.worker.enter();
        let pointers = cddb_internal::load_ref(&self.route.shared_pointers);
        let record = if let Some(ptr) = pointers.get(&entity_id) {
            // cdDB 0.2.0 supports native get_column_blob
            let col_payload = self.route.get_column_blob("payload", &self.worker)?;
            let col_epoch = self.route.get_column_int("epoch", &self.worker)?;
            let col_type = self.route.get_column_int("type", &self.worker)?;

            let payload_idx = ptr.attribute_indices.get("payload")?;
            let epoch_idx = ptr.attribute_indices.get("epoch")?;
            let type_idx = ptr.attribute_indices.get("type")?;

            // Zero-copy access (clone is still needed for SignedRecord return, but no Hex decoding)
            let payload = col_payload.get_element(*payload_idx, &self.worker)?;
            let epoch = col_epoch.get_element(*epoch_idx, &self.worker)? as EpochId;
            let record_type = col_type.get_element(*type_idx, &self.worker)?;

            Some(SignedRecord {
                epoch_id: epoch,
                payload,
                judge_signature: [0u8; 64],
                record_type,
            })
        } else {
            None
        };
        self.worker.leave();
        record
    }

    fn apply_signed_record(&self, record: SignedRecord) {
        // 1. Write to Cache (Write-through)
        self.cache.apply_signed_record(record.clone());

        // 2. Write to cdDB (Async Persistence)
        // cdDB 0.2.0 supports native Blob attributes
        let mut attrs_blob = Attributes::new();
        attrs_blob.insert("payload".to_string(), record.payload.clone());
        
        let mut attrs_int = Attributes::new();
        attrs_int.insert("epoch".to_string(), record.epoch_id as u32);
        attrs_int.insert("type".to_string(), record.record_type);

        let mut hasher = ahash::AHasher::default();
        use std::hash::Hasher;
        hasher.write(&record.payload);
        let entity_id = hasher.finish() as usize;

        // cdDB 0.2.0 send is synchronous and wait-free
        let _ = self.db_writer.send(WriteCommand::Insert {
            entity_id,
            attributes: Attributes::new(), // String attributes
            attributes_int: attrs_int,
            attributes_blob: attrs_blob,
        });
    }

    fn get_by_epoch(&self, epoch_id: EpochId) -> Vec<SignedRecord> {
        self.cache.get_by_epoch(epoch_id)
    }

    fn prune(&self, current_epoch: EpochId, k: u64) {
        self.cache.prune(current_epoch, k);
    }
}
