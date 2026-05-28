#[cfg(test)]
mod tests {
    use ServerGo::storage::TieredStore;
    use cdDB::CdDBDispatcher;
    use io_oi_core::{Hash32, SignedRecord, StateStore};

    #[test]
    fn test_store_drop_leak() {
        let namespace: Hash32 = [0xAA; 32];
        
        for i in 0..10 {
            let path = format!("data_leak_test_{}", i);
            let mut db = CdDBDispatcher::<1024>::new_std(Some(path.clone()));
            let store = TieredStore::new(namespace, 64, &mut db, format!("test_part_{}", i));
            
            let record = SignedRecord {
                epoch_id: 1,
                payload: vec![1, 2, 3],
                judge_signature: [0u8; 64],
                record_type: 0,
            };
            store.apply_signed_record(record);
            
            // Allow time for async workers
            std::thread::sleep(std::time::Duration::from_millis(10));
            // Explicit drop to test thread/leak cleanup.
            drop(store);
            drop(db);
            let _ = std::fs::remove_dir_all(path);
        }
        let _ = std::fs::remove_dir_all("data");
    }
}
