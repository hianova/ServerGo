use cdDB::{
    Attributes, CdDBDispatcher, PartitionRoute, UserWriter, WriteCommand,
};
use io_oi_core::{EpochId, Hash32, SignedRecord, StateStore};
use std::sync::Arc;

/// Pure Cache Backend using cdDB in memory-only mode (No-WAL)
#[derive(Clone)]
pub struct PureCacheStore {
    _db: Arc<std::sync::Mutex<CdDBDispatcher>>,
    db_writer: Arc<UserWriter>,
    route: PartitionRoute,
}

impl PureCacheStore {
    pub fn new(_namespace: Hash32, _ram_mb: usize) -> Self {
        // Create an in-memory CdDBDispatcher (base_path is None)
        let mut db = CdDBDispatcher::new_std(None);
        // Register in-memory partition (No-WAL)
        let writer = db.register_partition("pure_cache".to_string());
        let route = db.get_route("pure_cache").unwrap().clone();
        
        Self {
            _db: Arc::new(std::sync::Mutex::new(db)),
            db_writer: Arc::new(writer),
            route,
        }
    }
}

impl StateStore for PureCacheStore {
    fn get_record(&self, hash: &Hash32) -> Option<SignedRecord> {
        let mut hasher = ahash::AHasher::default();
        use std::hash::Hasher;
        hasher.write(hash);
        let entity_id = hasher.finish() as usize;

        // Use cdDB high-level Query thread interface with a single session pin
        let q = cdDB::Query::new(&self.route);
        let session = q.session();
        let payload = match session.get_blob(entity_id, "payload") {
            Some(p) => {
                crate::metrics::db_metrics::cache_hits().inc();
                p
            }
            None => {
                crate::metrics::db_metrics::cache_misses().inc();
                return None;
            }
        };
        let epoch = session.get_int(entity_id, "epoch")? as EpochId;
        let record_type = session.get_int(entity_id, "type")?;

        Some(SignedRecord {
            epoch_id: epoch,
            payload,
            judge_signature: [0u8; 64],
            record_type,
        })
    }

    fn apply_signed_record(&self, record: SignedRecord) {
        let mut attrs_blob = Attributes::new();
        attrs_blob.insert("payload".to_string(), record.payload.clone());
        
        let mut attrs_int = Attributes::new();
        attrs_int.insert("epoch".to_string(), record.epoch_id as u32);
        attrs_int.insert("type".to_string(), record.record_type);

        let mut hasher = ahash::AHasher::default();
        use std::hash::Hasher;
        hasher.write(&record.payload);
        let entity_id = hasher.finish() as usize;

        let _ = self.db_writer.send(WriteCommand::Insert {
            entity_id,
            attributes: Attributes::new(),
            attributes_int: attrs_int,
            attributes_blob: attrs_blob,
        });
    }

    fn get_by_epoch(&self, epoch_id: EpochId) -> Vec<SignedRecord> {
        let q = cdDB::Query::new(&self.route);
        let mut records = Vec::new();
        let session = q.session();
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
        // No-op for cache pruning
    }
}

/// Tiered Storage Backend using cdDB 0.2.3 (Wait-Free RCU + Native Blob)
#[derive(Clone)]
pub struct TieredStore {
    cache: PureCacheStore,
    db_writer: Arc<UserWriter>,
    route: PartitionRoute,
}

impl TieredStore {
    pub fn new(namespace: Hash32, ram_mb: usize, db: &mut CdDBDispatcher, partition: String) -> Self {
        // cdDB 0.2.3 register_partition_with_wal returns a synchronous UserWriter with persistence
        let wal_path = format!("data/{}.wal", partition);
        let writer = db.register_partition_with_wal(partition.clone(), Some(wal_path));
        let route = db.get_route(&partition).unwrap().clone();
        
        Self {
            cache: PureCacheStore::new(namespace, ram_mb),
            db_writer: Arc::new(writer),
            route,
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
        let mut hasher = ahash::AHasher::default();
        use std::hash::Hasher;
        hasher.write(hash);
        let entity_id = hasher.finish() as usize;

        // Use cdDB high-level Query thread interface with a single session pin
        let q = cdDB::Query::new(&self.route);
        let session = q.session();
        let payload = session.get_blob(entity_id, "payload")?;
        let epoch = session.get_int(entity_id, "epoch")? as EpochId;
        let record_type = session.get_int(entity_id, "type")?;

        Some(SignedRecord {
            epoch_id: epoch,
            payload,
            judge_signature: [0u8; 64],
            record_type,
        })
    }

    fn apply_signed_record(&self, record: SignedRecord) {
        // 1. Write to Cache (Write-through)
        self.cache.apply_signed_record(record.clone());

        // 2. Write to cdDB (Async Persistence)
        let mut attrs_blob = Attributes::new();
        attrs_blob.insert("payload".to_string(), record.payload.clone());
        
        let mut attrs_int = Attributes::new();
        attrs_int.insert("epoch".to_string(), record.epoch_id as u32);
        attrs_int.insert("type".to_string(), record.record_type);

        let mut hasher = ahash::AHasher::default();
        use std::hash::Hasher;
        hasher.write(&record.payload);
        let entity_id = hasher.finish() as usize;

        // cdDB 0.2.3 send is synchronous and wait-free
        let _ = self.db_writer.send(WriteCommand::Insert {
            entity_id,
            attributes: Attributes::new(),
            attributes_int: attrs_int,
            attributes_blob: attrs_blob,
        });
    }

    fn get_by_epoch(&self, epoch_id: EpochId) -> Vec<SignedRecord> {
        // Try cache first
        let cache_records = self.cache.get_by_epoch(epoch_id);
        if !cache_records.is_empty() {
            return cache_records;
        }

        let q = cdDB::Query::new(&self.route);
        let mut records = Vec::new();
        let session = q.session();
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

    fn prune(&self, current_epoch: EpochId, k: u64) {
        self.cache.prune(current_epoch, k);
    }
}

/// L2 Execution Layer - Coordinates business logic, caching, and persistence
pub struct L2Executor;

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
        let node_clone = Arc::clone(node);
        tokio::spawn(async move {
            let _ = node_clone.broadcast_record(record).await;
        });
    }
}
