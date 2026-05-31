#[cfg(feature = "loom")]
mod tests {
    use loom::sync::Arc;
    use loom::thread;
    use ServerGo::storage::cddb_helper::{GlobalHotIndex, CachedRecord};

    #[test]
    fn test_concurrent_hot_index() {
        loom::model(|| {
            let index = Arc::new(GlobalHotIndex::new(4));

            let index_clone1 = Arc::clone(&index);
            let t1 = thread::spawn(move || {
                let record = Arc::new(CachedRecord {
                    epoch_id: 1,
                    record_type: 100,
                    judge_signature: [0u8; 64],
                    payload: Arc::new(vec![1, 2, 3]),
                });
                index_clone1.set(1, record);
            });

            let index_clone2 = Arc::clone(&index);
            let t2 = thread::spawn(move || {
                let _ = index_clone2.get(1);
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }
}
