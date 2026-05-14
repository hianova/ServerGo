#[cfg(test)]
mod tests {
    use ServerGo::resp_server::RespServer;
    use io_oi_core::{GenesisConfig, Hash32, SignedRecord, StateStore, genesis};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::Mutex;
    use tokio::time::Duration;

    struct MockStore {
        records: Mutex<HashMap<Hash32, SignedRecord>>,
    }

    impl StateStore for MockStore {
        fn get_record(&self, hash: &Hash32) -> Option<SignedRecord> {
            self.records.lock().unwrap().get(hash).cloned()
        }
        fn apply_signed_record(&self, record: SignedRecord) {
            let mut h = [0u8; 32];
            h.copy_from_slice(&record.payload[0..32]);
            self.records.lock().unwrap().insert(h, record);
        }
        fn get_by_epoch(&self, _: u64) -> Vec<SignedRecord> {
            vec![]
        }
        fn prune(&self, _: u64, _: u64) {}
    }

    #[tokio::test]
    async fn test_resp_server() {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .try_init();

        let namespace: [u8; 32] = [0xAA; 32];
        let local_id: [u8; 32] = [1; 32];
        let storage = MockStore {
            records: Mutex::new(HashMap::new()),
        };

        let genesis_cfg = GenesisConfig {
            namespace,
            founder_id: local_id,
            initial_stake: 100,
            epoch_duration: 1000,
        };

        let node = Arc::new(genesis(genesis_cfg, storage));
        let resp_server = RespServer::new(Arc::clone(&node));

        let port = 16379; // Use a different port for testing
        let addr = format!("127.0.0.1:{}", port);

        // Run the RESP server in a separate task
        tokio::spawn(async move {
            let _ = resp_server.run(&addr).await;
        });

        // Give the server a moment to start
        tokio::time::sleep(Duration::from_millis(50)).await;

        let client = redis::Client::open(format!("redis://127.0.0.1:{}/", port)).unwrap();
        let mut con = client.get_multiplexed_async_connection().await.unwrap();

        // Test PING
        let pong: String = redis::cmd("PING").query_async(&mut con).await.unwrap();
        assert_eq!(pong, "PONG");

        // Test SET and GET
        let _: () = redis::cmd("SET")
            .arg("test_key")
            .arg("test_value")
            .query_async(&mut con)
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await; // Allow cache to sync
        let val: String = redis::cmd("GET")
            .arg("test_key")
            .query_async(&mut con)
            .await
            .unwrap();
        assert_eq!(val, "test_value");

        // Test missing GET
        let missing: Option<String> = redis::cmd("GET")
            .arg("missing_key")
            .query_async(&mut con)
            .await
            .unwrap();
        assert_eq!(missing, None);

        // Test INFO
        let info: String = redis::cmd("INFO").query_async(&mut con).await.unwrap();
        assert!(info.contains("server_go_version"));
    }
}
