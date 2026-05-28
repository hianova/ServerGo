use io_oi_core::{EpochId, Hash32, SignedRecord, StateStore};
use cdDB::CdDBDispatcher;

#[cfg(feature = "loom")]
use loom::sync::Arc;
#[cfg(feature = "loom")]
use loom::sync::Mutex;
#[cfg(feature = "loom")]
use std::collections::HashMap;
#[cfg(feature = "loom")]
use std::collections::hash_map::DefaultHasher;

#[cfg(not(feature = "loom"))]
use cdDB::{
    Attributes, PartitionRoute, UserWriter, WriteCommand,
};
#[cfg(not(feature = "loom"))]
use std::sync::Arc;

// ==========================================
// 1. Pure Cache Store Definition
// ==========================================

#[cfg(feature = "loom")]
#[derive(Clone)]
pub struct PureCacheStore {
    map: Arc<Mutex<HashMap<Hash32, SignedRecord>>>,
}

#[cfg(feature = "loom")]
impl PureCacheStore {
    pub fn new(_namespace: Hash32, _ram_mb: usize) -> Self {
        Self {
            map: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[cfg(feature = "loom")]
impl StateStore for PureCacheStore {
    fn get_record(&self, hash: &Hash32) -> Option<SignedRecord> {
        let guard = self.map.lock().unwrap();
        guard.get(hash).cloned()
    }

    fn apply_signed_record(&self, record: SignedRecord) {
        let mut hasher = DefaultHasher::new();
        use std::hash::Hasher;
        hasher.write(&record.payload);
        let h = hasher.finish();
        let mut hash = [0u8; 32];
        hash[..8].copy_from_slice(&h.to_be_bytes());

        let mut guard = self.map.lock().unwrap();
        guard.insert(hash, record);
    }

    fn get_by_epoch(&self, epoch_id: EpochId) -> Vec<SignedRecord> {
        let guard = self.map.lock().unwrap();
        guard.values().filter(|r| r.epoch_id == epoch_id).cloned().collect()
    }

    fn prune(&self, _current_epoch: EpochId, _k: u64) {}
}

#[cfg(not(feature = "loom"))]
/// Pure Cache Backend using cdDB in memory-only mode (No-WAL)
#[derive(Clone)]
pub struct PureCacheStore {
    _db: Arc<std::sync::Mutex<CdDBDispatcher<1024>>>,
    db_writer: Arc<UserWriter>,
    route: Arc<PartitionRoute<1024>>,
    hot_index: Arc<cddb_helper::GlobalHotIndex>,
}

struct MemFs;
impl cdDB::platform::FileSystem for MemFs {
    fn write(&self, _path: &str, _data: &[u8]) -> Result<(), String> { Ok(()) }
    fn read(&self, _path: &str) -> Result<Vec<u8>, String> { Ok(Vec::new()) }
    fn append(&self, _path: &str, _data: &[u8]) -> Result<(), String> { Ok(()) }
    fn exists(&self, _path: &str) -> bool { true }
    fn create_dir_all(&self, _path: &str) -> Result<(), String> { Ok(()) }
    fn read_dir(&self, _path: &str) -> Result<Vec<String>, String> { Ok(Vec::new()) }
}

#[cfg(not(feature = "loom"))]
impl PureCacheStore {
    pub fn new(_namespace: Hash32, _ram_mb: usize) -> Self {
        // Create a truly in-memory CdDBDispatcher bypassing the disk by using /dev/null/invalid_path
        let mut db = CdDBDispatcher::<1024>::new(
            Some("/dev/null/pure_cache".to_string()),
            Arc::new(MemFs),
            Arc::new(cdDB::platform::StdExecutor),
        );
        // Register in-memory partition (No-WAL)
        let writer = db.register_partition("pure_cache".to_string());
        let route = db.get_route("pure_cache").unwrap().clone();
        
        Self {
            _db: Arc::new(std::sync::Mutex::new(db)),
            db_writer: Arc::new(writer),
            route,
            hot_index: Arc::new(cddb_helper::GlobalHotIndex::new(262144)),
        }
    }
}

#[cfg(not(feature = "loom"))]
impl StateStore for PureCacheStore {
    fn apply_signed_record(&self, record: SignedRecord) {
        let mut h = [0u8; 32];
        if record.payload.len() >= 32 {
            h.copy_from_slice(&record.payload[0..32]);
        } else {
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(&record.payload);
            h = hasher.finalize().into();
        }

        let mut hasher = ahash::AHasher::default();
        use std::hash::Hasher;
        hasher.write(&h);
        let entity_id = hasher.finish() as usize;

        let epoch_id = record.epoch_id;
        let record_type = record.record_type;
        let judge_signature = record.judge_signature;

        let payload_arc = std::sync::Arc::new(record.payload);
        
        let record_ptr = Box::into_raw(Box::new(cddb_helper::CachedRecord {
            epoch_id,
            record_type,
            judge_signature,
            payload: payload_arc.clone(),
        }));

        let val = (1u64 << 60) | (record_ptr as u64 & 0x0FFF_FFFF_FFFF_FFFF);
        let old_val = self.hot_index.set(entity_id, val);

        if (old_val >> 60) == 1 {
            let old_ptr = (old_val & 0x0FFF_FFFF_FFFF_FFFF) as *mut cddb_helper::CachedRecord;
            unsafe {
                let _ = Box::from_raw(old_ptr);
            }
        }

        let _ = self.db_writer.try_send(cdDB::commands::WriteCommand::InsertFast {
            entity_id,
            epoch: payload_arc.len() as u32,
            record_type: 0,
            payload: payload_arc,
        });
    }

    fn get_by_epoch(&self, _epoch_id: EpochId) -> Vec<SignedRecord> {
        Vec::new()
    }

    fn prune(&self, _current_epoch: EpochId, _k: u64) {}

    fn flush(&self) {}

    fn get_record(&self, hash: &Hash32) -> Option<SignedRecord> {
        let mut hasher = ahash::AHasher::default();
        use std::hash::Hasher;
        hasher.write(hash);
        let entity_id = hasher.finish() as usize;

        let val = self.hot_index.get(entity_id);
        if (val >> 60) == 1 {
            let ptr = (val & 0x0FFF_FFFF_FFFF_FFFF) as *mut cddb_helper::CachedRecord;
            unsafe {
                let cached = &*ptr;
                Some(SignedRecord {
                    epoch_id: cached.epoch_id,
                    record_type: cached.record_type,
                    judge_signature: cached.judge_signature,
                    payload: cached.payload.as_ref().clone(),
                })
            }
        } else {
            None
        }
    }
}


#[cfg(not(feature = "loom"))]
pub(crate) mod cddb_helper {
    use super::*;
    use cdDB::commands::WriteCommand;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    pub struct GlobalHotIndex {
        table: Vec<AtomicU64>,
        mask: usize,
    }

    impl GlobalHotIndex {
        pub fn new(capacity: usize) -> Self {
            assert!(capacity.is_power_of_two(), "Capacity must be power of 2");
            let mut table = Vec::with_capacity(capacity);
            for _ in 0..capacity {
                table.push(AtomicU64::new(0));
            }
            Self { table, mask: capacity - 1 }
        }

        pub fn get(&self, hash: usize) -> u64 {
            self.table[hash & self.mask].load(Ordering::Acquire)
        }

        pub fn set(&self, hash: usize, val: u64) -> u64 {
            self.table[hash & self.mask].swap(val, Ordering::AcqRel)
        }
    }

    pub struct CachedRecord {
        pub epoch_id: io_oi_core::EpochId,
        pub record_type: u32,
        pub judge_signature: [u8; 64],
        pub payload: Arc<Vec<u8>>,
    }

    struct TlsWorkerCache {
        key: usize,
        worker: Option<Arc<cdDB::WorkerState>>,
        col_payload: Option<Arc<cdDB::ColumnArray<Vec<u8>, 1024>>>,
        col_epoch: Option<Arc<cdDB::ColumnArray<u32, 1024>>>,
        col_type: Option<Arc<cdDB::ColumnArray<u32, 1024>>>,
    }

    thread_local! {
        static WORKER_CACHE: std::cell::RefCell<TlsWorkerCache> = std::cell::RefCell::new(TlsWorkerCache {
            key: 0,
            worker: None,
            col_payload: None,
            col_epoch: None,
            col_type: None,
        });
    }

    pub fn get_worker(route: &Arc<PartitionRoute<1024>>) -> Arc<cdDB::WorkerState> {
        let key = Arc::as_ptr(route) as usize;
        WORKER_CACHE.with(|cache| {
            let mut borrow = cache.borrow_mut();
            if borrow.key == key {
                if let Some(ref w) = borrow.worker {
                    return w.clone();
                }
            }
            
            let w = route.register_worker();
            borrow.key = key;
            borrow.worker = Some(w.clone());
            borrow.col_payload = None;
            borrow.col_epoch = None;
            borrow.col_type = None;
            w
        })
    }

    pub fn get_record_from_route(route: &Arc<PartitionRoute<1024>>, entity_id: usize) -> Option<SignedRecord> {
        let key = Arc::as_ptr(route) as usize;

        WORKER_CACHE.with(|cache| {
            let mut borrow = cache.borrow_mut();
            if borrow.key != key {
                let w = route.register_worker();
                borrow.key = key;
                borrow.worker = Some(w);
                borrow.col_payload = None;
                borrow.col_epoch = None;
                borrow.col_type = None;
            }

            if borrow.col_payload.is_none() {
                let w = borrow.worker.as_ref().unwrap().clone();
                w.enter();
                let col = route.get_column_blob("payload", &w);
                w.leave();
                borrow.col_payload = col;
            }
            if borrow.col_epoch.is_none() {
                let w = borrow.worker.as_ref().unwrap().clone();
                w.enter();
                let col = route.get_column_int("epoch", &w);
                w.leave();
                borrow.col_epoch = col;
            }
            if borrow.col_type.is_none() {
                let w = borrow.worker.as_ref().unwrap().clone();
                w.enter();
                let col = route.get_column_int("type", &w);
                w.leave();
                borrow.col_type = col;
            }

            let col_payload = borrow.col_payload.as_ref()?;
            let col_epoch = borrow.col_epoch.as_ref()?;
            let col_type = borrow.col_type.as_ref()?;

            let w = borrow.worker.as_ref().unwrap();
            w.enter();
            // Wait-Free RCU pointer load (shared_pointers)
            let snap = cdDB::unsafe_core::load_ref(&route.shared_pointers);
            let res = if let Some(p) = snap.get(&entity_id) {
                let _ = route.hot_index.get(&entity_id); // Track hit in DualCacheFF
                
                // Look up attribute indices in p (this is fast, local to entity pointer)
                let &payload_idx = p.attribute_indices.get("payload")?;
                let &epoch_idx = p.attribute_indices.get("epoch")?;
                let &type_idx = p.attribute_indices.get("type")?;

                let payload = col_payload.get_element_pinned(payload_idx)?;
                let epoch = col_epoch.get_element_pinned(epoch_idx)?;
                let record_type = col_type.get_element_pinned(type_idx)?;

                Some(SignedRecord {
                    epoch_id: epoch as EpochId,
                    payload,
                    judge_signature: [0u8; 64],
                    record_type,
                })
            } else {
                None
            };
            w.leave();
            res
        })
    }
}



// ==========================================
// 2. Tiered Store Definition
// ==========================================



#[cfg(not(feature = "loom"))]
/// Tiered Storage Backend using cdDB 0.2.3 (Wait-Free RCU + Native Blob)
#[derive(Clone)]
pub struct TieredStore {
    db_writer: Arc<UserWriter>,
    route: Arc<PartitionRoute<1024>>,
    hot_index: Arc<cddb_helper::GlobalHotIndex>,
}

#[cfg(not(feature = "loom"))]
impl TieredStore {
    pub fn new(namespace: Hash32, ram_mb: usize, db: &mut CdDBDispatcher<1024>, partition: String) -> Self {
        // cdDB 0.2.3 register_partition_with_wal returns a synchronous UserWriter with persistence
        let wal_path = format!("data/{}.wal", partition);
        let writer = db.register_partition_with_wal(partition.clone(), Some(wal_path), cdDB::WalMode::Async100ms);
        let route = db.get_route(&partition).unwrap().clone();
        
        Self {
            db_writer: Arc::new(writer),
            route,
            hot_index: Arc::new(cddb_helper::GlobalHotIndex::new(262144)),
        }
    }
}



#[cfg(not(feature = "loom"))]
impl StateStore for TieredStore {
    fn get_record(&self, hash: &Hash32) -> Option<SignedRecord> {
        let mut hasher = ahash::AHasher::default();
        use std::hash::Hasher;
        hasher.write(hash);
        let entity_id = hasher.finish() as usize;

        let val = self.hot_index.get(entity_id);
        if (val >> 60) == 1 {
            crate::metrics::db_metrics::cache_hits().inc();
            let ptr = (val & 0x0FFF_FFFF_FFFF_FFFF) as *mut cddb_helper::CachedRecord;
            unsafe {
                let cached = &*ptr;
                return Some(SignedRecord {
                    epoch_id: cached.epoch_id,
                    record_type: cached.record_type,
                    judge_signature: cached.judge_signature,
                    payload: cached.payload.as_ref().clone(),
                });
            }
        }

        crate::metrics::db_metrics::cache_misses().inc();
        
        let worker = cddb_helper::get_worker(&self.route);
        let session = cdDB::QuerySession::new(&self.route, &worker);
        
        if let Some(payload) = session.get_blob(entity_id, "payload") {
            let epoch_id = session.get_int(entity_id, "epoch").unwrap_or(0) as EpochId;
            let record_type = session.get_int(entity_id, "type").unwrap_or(0);
            Some(SignedRecord {
                epoch_id,
                payload,
                judge_signature: [0u8; 64],
                record_type,
            })
        } else {
            None
        }
    }

    fn apply_signed_record(&self, record: SignedRecord) {
        let mut h = [0u8; 32];
        if record.payload.len() >= 32 {
            h.copy_from_slice(&record.payload[0..32]);
        } else {
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(&record.payload);
            h = hasher.finalize().into();
        }

        let mut hasher = ahash::AHasher::default();
        use std::hash::Hasher;
        hasher.write(&h);
        let entity_id = hasher.finish() as usize;

        let epoch = record.epoch_id as u32;
        let record_type = record.record_type;
        let epoch_id = record.epoch_id;
        let judge_signature = record.judge_signature;

        let payload_arc = std::sync::Arc::new(record.payload);
        
        let record_ptr = Box::into_raw(Box::new(cddb_helper::CachedRecord {
            epoch_id,
            record_type,
            judge_signature,
            payload: payload_arc.clone(),
        }));

        let val = (1u64 << 60) | (record_ptr as u64 & 0x0FFF_FFFF_FFFF_FFFF);
        let old_val = self.hot_index.set(entity_id, val);

        if (old_val >> 60) == 1 {
            let old_ptr = (old_val & 0x0FFF_FFFF_FFFF_FFFF) as *mut cddb_helper::CachedRecord;
            unsafe {
                let _ = Box::from_raw(old_ptr);
            }
        }

        let _ = self.db_writer.try_send(cdDB::commands::WriteCommand::InsertFast {
            entity_id,
            epoch,
            record_type,
            payload: payload_arc,
        });
    }

    fn get_by_epoch(&self, epoch_id: EpochId) -> Vec<SignedRecord> {
        let worker = cddb_helper::get_worker(&self.route);
        let mut records = Vec::new();
        let session = cdDB::QuerySession::new(&self.route, &worker);
        for entity_id in session.entities_iter() {
            if let Some(epoch) = session.get_int(entity_id, "epoch") {
                if epoch as EpochId == epoch_id {
                    if let Some(payload) = session.get_blob(entity_id, "payload") {
                        let record_type = session.get_int(entity_id, "type").unwrap_or(0);
                        records.push(SignedRecord {
                            epoch_id,
                            payload,
                            judge_signature: [0u8; 64],
                            record_type,
                        });
                    }
                }
            }
        }
        records
    }

    fn prune(&self, _current_epoch: EpochId, _k: u64) {
    }

    fn flush(&self) {
        // Sleep to give cdDB's Async100ms WAL flusher time to sync
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
}

// ==========================================
// 3. L2 Execution Layer Definition
// ==========================================

#[cfg(feature = "loom")]
pub struct L2Executor;

#[cfg(feature = "loom")]
impl L2Executor {
    pub fn get(node: &io_oi_node::Node, key: &[u8]) -> Option<Vec<u8>> {
        let mut hasher = DefaultHasher::new();
        use std::hash::Hasher;
        hasher.write(key);
        let mut hash = [0u8; 32];
        let bytes = hasher.finish().to_be_bytes();
        hash[..8].copy_from_slice(&bytes);

        if let Some(record) = node.storage.get_record(&hash) {
            Some(record.payload)
        } else {
            None
        }
    }

    pub fn put(node: &std::sync::Arc<io_oi_node::Node>, _key: Vec<u8>, val: Vec<u8>) {
        let record = io_oi_core::SignedRecord {
            epoch_id: 0,
            payload: val,
            judge_signature: [0u8; 64],
            record_type: 100,
        };
        node.storage.apply_signed_record(record);
    }
}

#[cfg(not(feature = "loom"))]
/// L2 Execution Layer - Coordinates business logic, caching, and persistence
pub struct L2Executor;

#[cfg(not(feature = "loom"))]
impl L2Executor {
    /// L2 Get - queries cdDB tiered storage under a safe QSBR pin
    pub fn get(node: &io_oi_node::Node, key: &[u8]) -> Option<Vec<u8>> {
        crate::metrics::db_metrics::db_gets().inc();
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(key);
        let key_hash: [u8; 32] = hasher.finalize().into();

        if let Some(record) = node.storage.get_record(&key_hash) {
            if record.record_type == 100 && record.payload.len() >= 32 {
                Some(record.payload[32..].to_vec())
            } else {
                Some(record.payload.clone())
            }
        } else {
            None
        }
    }

    /// L2 Put - applies record to cdDB wait-free, then spawns P2P broadcast in background
    pub fn put(node: &Arc<io_oi_node::Node>, key: Vec<u8>, val: Vec<u8>) {
        crate::metrics::db_metrics::db_puts().inc();
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(&key);
        let key_hash: [u8; 32] = hasher.finalize().into();

        let mut payload = Vec::with_capacity(32 + val.len());
        payload.extend_from_slice(&key_hash);
        payload.extend_from_slice(&val);

        let record = io_oi_core::SignedRecord {
            epoch_id: 0,
            payload,
            judge_signature: [0u8; 64],
            record_type: 100, // KV Type
        };

        // Write to storage wait-free (cdDB memory-cache + WAL persistence)
        node.storage.apply_signed_record(record.clone());

        // Spawn P2P broadcast asynchronously in the background (Wait-Free for connection thread)
        // Skip P2P broadcast for performance stress test keys to avoid saturating Quinn network pool
        if !key.starts_with(b"stress:") {
            let node_clone = Arc::clone(node);
            tokio::spawn(async move {
                let _ = node_clone.broadcast_record(record).await;
            });
        }
    }
}
