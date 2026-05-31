#[cfg(test)]
mod tests {
    use io_oi_node::{RespGateway, RespCommandParser, GatewayCommand, GatewayRequest, genesis};
    use io_oi_core::{GenesisConfig, Hash32, SignedRecord, StateStore, TrustMode, ControlMode};
        use std::sync::Arc;
        use tokio::time::Duration;
    use bytes::Bytes;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use ServerGo::storage::TieredStore;
    use cdDB::CdDBDispatcher;

    struct SimpleParser;
    impl RespCommandParser for SimpleParser {
        fn parse(&self, frame: resp_rs::resp2::Frame) -> Result<GatewayCommand, String> {
            use resp_rs::resp2::Frame;
            match frame {
                Frame::Array(Some(arr)) => {
                    let cmd_bytes = match &arr[0] {
                        Frame::BulkString(Some(s)) => s.as_ref(),
                        _ => return Err("Err".into()),
                    };
                    if cmd_bytes.eq_ignore_ascii_case(b"GET") {
                        let key = match &arr[1] {
                            Frame::BulkString(Some(s)) => String::from_utf8_lossy(s).into_owned(),
                            _ => return Err("Err".into()),
                        };
                        Ok(GatewayCommand::Get(key))
                    } else if cmd_bytes.eq_ignore_ascii_case(b"PING") {
                        Ok(GatewayCommand::Ping)
                    } else if cmd_bytes.eq_ignore_ascii_case(b"SET") {
                        let key = match &arr[1] {
                            Frame::BulkString(Some(s)) => String::from_utf8_lossy(s).into_owned(),
                            _ => return Err("Err".into()),
                        };
                        let val = match &arr[2] {
                            Frame::BulkString(Some(s)) => s.to_vec(),
                            _ => return Err("Err".into()),
                        };
                        Ok(GatewayCommand::Put(key, val))
                    } else {
                        Ok(GatewayCommand::Info)
                    }
                }
                _ => Err("Err".into()),
            }
        }
    }

    #[tokio::test]
    async fn test_resp_server() {
        let temp_dir = tempfile::tempdir().unwrap();
        let data_path = temp_dir.path().join("data_test").to_str().unwrap().to_string();
        
        let namespace: [u8; 32] = [0xAA; 32];
        let local_id: [u8; 32] = [1; 32];
        let mut db = CdDBDispatcher::<1024>::new_std(Some(data_path));
        let storage = TieredStore::new(namespace, 512, &mut db, "test_partition".to_string(), None);

        let genesis_cfg = GenesisConfig {
            namespace,
            founder_id: local_id,
            initial_stake: 100,
            epoch_duration: 1000,
            trust_mode: TrustMode::Full,
            control_mode: ControlMode::Competitive,
            genesis_url: None,
            wal_path: None,
        };

        let secret_key = iroh::SecretKey::generate();
        let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::Minimal)
            .secret_key(secret_key)
            .bind()
            .await
            .unwrap();

        let node = Arc::new(genesis(genesis_cfg, endpoint, storage));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        drop(listener);
        let (tx, mut rx) = tokio::sync::mpsc::channel::<GatewayRequest>(100);
        let node_clone = Arc::clone(&node);

        tokio::spawn(async move {
            while let Some(req) = rx.recv().await {
                use resp_rs::resp2::Frame;
                match req.cmd {
                    GatewayCommand::Get(key) => {
                        use sha2::{Sha256, Digest};
                        let mut hasher = Sha256::new();
                        hasher.update(&key);
                        let key_hash: [u8; 32] = hasher.finalize().into();
                        if let Some(record) = node_clone.storage.get_record(&key_hash) {
                            let val = if record.payload.len() >= 32 { &record.payload[32..] } else { &record.payload };
                            let _ = req.response_tx.send(Frame::BulkString(Some(Bytes::copy_from_slice(val))));
                        } else {
                            let _ = req.response_tx.send(Frame::BulkString(None));
                        }
                    }
                    GatewayCommand::Put(key, val) => {
                        use sha2::{Sha256, Digest};
                        let mut hasher = Sha256::new();
                        hasher.update(&key);
                        let key_hash: [u8; 32] = hasher.finalize().into();
                        let mut payload = Vec::with_capacity(32 + val.len());
                        payload.extend_from_slice(&key_hash);
                        payload.extend_from_slice(&val);
                        node_clone.storage.apply_signed_record(SignedRecord {
                            epoch_id: 0,
                            payload,
                            judge_signature: [0u8; 64],
                            record_type: 100,
                        });
                        let _ = req.response_tx.send(Frame::SimpleString("OK".into()));
                    }
                    GatewayCommand::Ping => {
                        let _ = req.response_tx.send(Frame::SimpleString("PONG".into()));
                    }
                    GatewayCommand::Info => {
                        let _ = req.response_tx.send(Frame::SimpleString("OK".into()));
                    }
                    _ => {
                        let _ = req.response_tx.send(Frame::SimpleString("OK".into()));
                    }
                }
            }
        });

        let gateway = RespGateway::new(&addr, tx, SimpleParser);
        tokio::spawn(async move { let _ = gateway.run().await; });
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut stream = tokio::net::TcpStream::connect(&addr).await.unwrap();
        
        // Test SET
        stream.write_all(b"*3\r\n$3\r\nSET\r\n$1\r\nk\r\n$1\r\nv\r\n").await.unwrap();
        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        assert!(String::from_utf8_lossy(&buf[..n]).contains("OK"));

        // Test GET
        tokio::time::sleep(Duration::from_millis(200)).await;
        stream.write_all(b"*2\r\n$3\r\nGET\r\n$1\r\nk\r\n").await.unwrap();
        let n = stream.read(&mut buf).await.unwrap();
        assert!(String::from_utf8_lossy(&buf[..n]).contains("$1\r\nv\r\n"));

        
        // Test PING
        stream.write_all(b"*1\r\n$4\r\nPING\r\n").await.unwrap();
        let mut buf2 = [0u8; 1024];
        let n = stream.read(&mut buf2).await.unwrap();
        assert!(String::from_utf8_lossy(&buf2[..n]).contains("+PONG"));

        // Test INFO
        stream.write_all(b"*1\r\n$4\r\nINFO\r\n").await.unwrap();
        let n = stream.read(&mut buf2).await.unwrap();
        assert!(String::from_utf8_lossy(&buf2[..n]).contains("+OK"));

        // Clean up handled automatically by temp_dir going out of scope

        drop(stream);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
