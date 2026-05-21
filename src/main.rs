use ServerGo::storage::L2Executor;
#[cfg(feature = "pure-cache")]
use ServerGo::storage::PureCacheStore;
#[cfg(feature = "tiered-storage")]
use ServerGo::storage::TieredStore;
use cdDB::CdDBDispatcher;
use clap::Parser;
use io_oi_core::{ControlMode, GenesisConfig, NodeId, TrustMode};
use io_oi_node::{DefaultRespCommandParser, RespGateway, genesis};
use std::sync::Arc;
use tracing::{Level, info};
use bytes::Bytes;
use socket2::{Socket, Domain, Type, Protocol};
use std::net::TcpListener;

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

    // 1. Initialize Storage based on features
    #[cfg(feature = "tiered-storage")]
    let storage = {
        info!("Mode: Tiered Storage (Cache + Columnar DB)");
        let mut db = CdDBDispatcher::new_std(Some("data".to_string()));
        TieredStore::new(
            namespace,
            args.budget,
            &mut db,
            format!("server_go.node_{}", args.id),
        )
    };

    #[cfg(all(feature = "pure-cache", not(feature = "tiered-storage")))]
    let storage = {
        info!("Mode: Pure Cache (In-memory Wait-Free)");
        PureCacheStore::new(namespace, args.budget)
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

    let cores = num_cpus::get();
    info!("Spawning {} worker threads (Thread-per-core architecture)", cores);
    
    let mut handles = vec![];
    for i in 0..cores {
        let node_clone = Arc::clone(&node);
        let bind_addr = args.bind.clone();
        let port = args.port;
        
        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime");
                
            rt.block_on(async move {
                let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).unwrap();
                socket.set_reuse_address(true).unwrap();
                #[cfg(unix)]
                socket.set_reuse_port(true).unwrap();
                
                let addr = format!("{}:{}", bind_addr, port).parse::<std::net::SocketAddr>().unwrap();
                socket.bind(&addr.into()).unwrap();
                socket.listen(1024).unwrap();
                
                let listener = TcpListener::from(socket);
                let tokio_listener = tokio::net::TcpListener::from_std(listener).unwrap();
                
                let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
                let gateway = RespGateway::new_with_listener(tokio_listener, tx, DefaultRespCommandParser);
                
                info!("Worker {} listening on {}", i, addr);

                tokio::spawn(async move {
                    while let Some(req) = rx.recv().await {
                        use resp_rs::resp2::Frame;
                        use io_oi_node::GatewayCommand;

                        match req.cmd {
                            GatewayCommand::Get(key) => {
                                if let Some(val) = L2Executor::get(&node_clone, key.as_bytes()) {
                                    let _ = req.response_tx.send(Frame::BulkString(Some(Bytes::copy_from_slice(&val))));
                                } else {
                                    let _ = req.response_tx.send(Frame::BulkString(None));
                                }
                            }
                            GatewayCommand::Put(key, val) => {
                                L2Executor::put(&node_clone, key.into_bytes(), val);
                                let _ = req.response_tx.send(Frame::SimpleString("OK".into()));
                            }
                            GatewayCommand::Info => {
                                let stats = format!(
                                    "worker_id:{}\r\nversion:0.1.0\r\n",
                                    i
                                );
                                let _ = req.response_tx.send(Frame::BulkString(Some(Bytes::from(stats))));
                            }
                            GatewayCommand::Ping => {
                                let _ = req.response_tx.send(Frame::SimpleString("PONG".into()));
                            }
                            _ => {
                                let _ = req.response_tx.send(Frame::Error("Unknown command".into()));
                            }
                        }
                    }
                });

                gateway.run().await.unwrap();
            });
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }

    Ok(())
}
