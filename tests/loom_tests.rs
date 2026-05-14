#[cfg(loom)]
mod tests {
    use loom::sync::Arc;
    use loom::thread;
    use io_oi_core::{SignedRecord, StateStore};
    use ServerGo::storage::PureCacheStore;
    use dualcache_ff::Config;

    type Hash32 = [u8; 32];

    #[test]
    fn test_concurrent_apply_and_get() {
        loom::model(|| {
            let namespace: Hash32 = [0xAA; 32];
            let config = Config::with_memory_budget(1, 60); // Small config for loom
            let store = Arc::new(PureCacheStore::new(namespace, config));

            let store_clone: Arc<PureCacheStore> = Arc::clone(&store);
            let t1 = thread::spawn(move || {
                let record = SignedRecord {
                    epoch_id: 1,
                    payload: vec![1, 2, 3],
                    judge_signature: [0u8; 64],
                    record_type: 0,
                };
                store_clone.apply_signed_record(record);
            });

            let store_clone2: Arc<PureCacheStore> = Arc::clone(&store);
            let t2 = thread::spawn(move || {
                let hash = [0u8; 32];
                let _ = store_clone2.get_record(&hash);
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }
}
