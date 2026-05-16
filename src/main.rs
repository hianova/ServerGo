use sha2::{Sha256, Digest};
use ServerGo::storage::{PureCacheStore, TieredStore};
use cdDB::CdDBDispatcher;
use clap::Parser;
use io_oi_core::{ControlMode, GenesisConfig, NodeId, TrustMode, StateStore};
use io_oi_node::{DefaultRespCommandParser, GatewayCommand, RespCommandParser, RespGateway, genesis};
use rkyv::Deserialize;
use std::sync::Arc;
use tracing::{Level, info};
use bytes::Bytes;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to bind the RESP server to
    #[arg(short, long, default_value_t = 6379)]
    port: u16,

    /// IP address to bind the RESP server to
    #[arg(short, long, default_value = "127.0.0.1")]
    bind: String,

    /// Node ID (1-255)
    #[arg(short, long, default_value_t = 1)]
    id: u8,

    /// Memory budget for the cache in MB
    #[arg(long, default_value_t = 512)]
    budget: usize,

    /// Trust Mode: full (Broadcast) or localized (Gossip)
    #[arg(long, default_value = "localized")]
    trust_mode: String,

    /// Control Mode: strict (Leader-only) or competitive (Decentralized)
    #[arg(long, default_value = "competitive")]
    control_mode: String,

    /// Peer Iroh Ticket or Node ID to connect to
    #[arg(short = 'E', long)]
    peer: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let args = Args::parse();

    info!("--- ServerGo: io_oi v2 Powered Distributed Database ---");
    info!("Node ID: {}, Bind: {}:{}", args.id, args.bind, args.port);

    let namespace: [u8; 32] = [0xAA; 32];
    let mut local_id_bytes: NodeId = [0x00; 32];
    local_id_bytes[0] = args.id;
    let cache_config = dualcache_ff::Config::with_memory_budget(args.budget, 60);

    // 1. Initialize Storage based on features
    #[cfg(feature = "tiered-storage")]
    let storage = {
        info!("Mode: Tiered Storage (Cache + Columnar DB)");
        let mut db = CdDBDispatcher::new_std(Some("data".to_string()));
        TieredStore::new(
            namespace,
            cache_config,
            &mut db,
            format!("server_go.node_{}", args.id),
        )
    };

    #[cfg(all(feature = "pure-cache", not(feature = "tiered-storage")))]
    let storage = {
        info!("Mode: Pure Cache (In-memory Wait-Free)");
        PureCacheStore::new(namespace, cache_config)
    };

    #[cfg(all(not(feature = "tiered-storage"), not(feature = "pure-cache")))]
    let storage = {
        info!("Mode: Default (Mock Store)");
        struct MockStore;
        impl io_oi_core::StateStore for MockStore {
            fn get_record(&self, _: &io_oi_core::Hash32) -> Option<io_oi_core::SignedRecord> {
                None
            }
            fn apply_signed_record(&self, _: io_oi_core::SignedRecord) {}
            fn get_by_epoch(&self, _: io_oi_core::EpochId) -> Vec<io_oi_core::SignedRecord> {
                vec![]
            }
            fn prune(&self, _: io_oi_core::EpochId, _: u64) {}
        }
        MockStore
    };

    // 2. Initialize P2P Endpoint
    let secret_key = iroh::SecretKey::generate();
    let node_id = secret_key.public();
    let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::Minimal)
        .secret_key(secret_key)
        .bind()
        .await?;
    info!("P2P Endpoint initialized. PeerId: {}", node_id);

    // 3. Initialize io_oi Node with Governance Modes
    let trust_mode = match args.trust_mode.to_lowercase().as_str() {
        "full" => TrustMode::Full,
        _ => TrustMode::Localized,
    };
    let control_mode = match args.control_mode.to_lowercase().as_str() {
        "strict" => ControlMode::Strict,
        _ => ControlMode::Competitive,
    };

    let genesis_cfg = GenesisConfig {
        namespace,
        founder_id: local_id_bytes,
        initial_stake: 100,
        epoch_duration: 1000,
        trust_mode,
        control_mode,
        genesis_url: None,
    };

    let node = Arc::new(genesis(genesis_cfg, endpoint, storage));
    info!(
        "io_oi Node initialized with TrustMode::{:?} and ControlMode::{:?}",
        trust_mode, control_mode
    );

    if let Some(peer_str) = args.peer {
        if let Ok(peer_id) = peer_str.parse::<iroh::PublicKey>() {
            node.add_peer(peer_id);
            info!("Added peer: {}", peer_id);
        }
    }

    // 4. Start RESP Gateway (Internal to io_oi_node)
    let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
    let gateway_addr = format!("{}:{}", args.bind, args.port);
    let gateway = RespGateway::new(&gateway_addr, tx, DefaultRespCommandParser);

    let node_clone = Arc::clone(&node);
    tokio::spawn(async move {
        while let Some(req) = rx.recv().await {
            use resp_rs::resp2::Frame;
            use io_oi_node::GatewayCommand;

            match req.cmd {
                GatewayCommand::Get(key) => {
                    let mut hasher = Sha256::new();
                    hasher.update(key.as_bytes());
                    let key_hash: [u8; 32] = hasher.finalize().into();

                    if let Some(record) = node_clone.storage.get_record(&key_hash) {
                        // Strip key_hash if it's KV type
                        let val = if record.record_type == 100 && record.payload.len() >= 32 {
                            &record.payload[32..]
                        } else {
                            &record.payload
                        };
                        let _ = req.response_tx.send(Frame::BulkString(Some(Bytes::copy_from_slice(val))));
                    } else {
                        let _ = req.response_tx.send(Frame::BulkString(None));
                    }
                }
                GatewayCommand::Put(key, val) => {
                    let mut hasher = Sha256::new();
                    hasher.update(key.as_bytes());
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
                    
                    // 1. Apply locally (Self-correction/Immediate consistency for local write)
                    node_clone.storage.apply_signed_record(record.clone());
                    
                    // 2. Broadcast to peers
                    let _ = node_clone.broadcast_record(record).await;
                    
                    let _ = req.response_tx.send(Frame::SimpleString("OK".into()));
                }
                GatewayCommand::Info => {
                    let stats = format!(
                        "node_id:{}\r\nversion:0.1.0\r\ntrust_mode:{:?}\r\ncontrol_mode:{:?}\r\n",
                        args.id, args.trust_mode, args.control_mode
                    );
                    let _ = req.response_tx.send(Frame::BulkString(Some(Bytes::from(stats))));
                }
                _ => {
                    let _ = req.response_tx.send(Frame::Error("Unknown command".into()));
                }
            }
        }
    });

    info!(
        "Wire Protocol Compatibility Layer active (RESP/Redis) on {}.",
        gateway_addr
    );
    gateway.run().await?;

    Ok(())
}
