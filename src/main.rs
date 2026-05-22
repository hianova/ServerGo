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

    /// Serial port to communicate with the Gateway ESP32
    #[arg(short, long)]
    serial_port: Option<String>,
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

    let mut _db_holder = None;

    // 1. Initialize Storage based on features
    #[cfg(feature = "tiered-storage")]
    let storage = {
        info!("Mode: Tiered Storage (Cache + Columnar DB)");
        let mut db = CdDBDispatcher::new_std(Some("data".to_string()));
        let store = TieredStore::new(
            namespace,
            args.budget,
            &mut db,
            format!("server_go.node_{}", args.id),
        );
        _db_holder = Some(db);
        store
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

    // Start serial driver if serial_port is provided
    let serial_tx = if let Some(ref port) = args.serial_port {
        let tx = node.start_serial_driver(port, 115200);
        info!("Serial driver started on port: {}", port);
        Some(tx)
    } else {
        None
    };

    #[cfg(target_os = "macos")]
    {
        let addr = format!("{}:{}", args.bind, args.port).parse::<std::net::SocketAddr>().unwrap();
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
        let gateway = RespGateway::new_with_listener(listener, tx, DefaultRespCommandParser);
        
        info!("Worker 0 listening directly on main runtime at {}", addr);

        let node_clone = Arc::clone(&node);
        let serial_tx_clone = serial_tx.clone();
        
        tokio::spawn(async move {
            while let Some(req) = rx.recv().await {
                use resp_rs::resp2::Frame;
                use io_oi_node::GatewayCommand;

                info!("[ServerGo Debug] Received request: {:?}", req.cmd);

                match req.cmd {
                    GatewayCommand::Get(key) => {
                        info!("[ServerGo Debug] L2Executor::get for key: {}", key);
                        if let Some(val) = L2Executor::get(&node_clone, key.as_bytes()) {
                            info!("[ServerGo Debug] Found key {} val len {}", key, val.len());
                            let send_res = req.response_tx.send(Frame::BulkString(Some(Bytes::copy_from_slice(&val))));
                            info!("[ServerGo Debug] Response send result: {:?}", send_res);
                        } else {
                            info!("[ServerGo Debug] Key {} not found", key);
                            let send_res = req.response_tx.send(Frame::BulkString(None));
                            info!("[ServerGo Debug] Response send result: {:?}", send_res);
                        }
                    }
                    GatewayCommand::Put(key, val) => {
                        info!("[ServerGo Debug] L2Executor::put for key: {}, val len: {}", key, val.len());
                        L2Executor::put(&node_clone, key.clone().into_bytes(), val.clone());
                        info!("[ServerGo Debug] L2Executor::put finished");
                        
                        // Forward VM scripts to Gateway Bridge over USB Serial
                        if key.starts_with("vm:") {
                            if let Some(ref s_tx) = serial_tx_clone {
                                let mac = {
                                    let part = key.strip_prefix("vm:").unwrap_or(&key);
                                    if part.eq_ignore_ascii_case("broadcast") || part.eq_ignore_ascii_case("all") {
                                        [0xFF; 6]
                                    } else {
                                        let clean: String = part.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                                        if clean.len() == 12 {
                                            let mut m = [0u8; 6];
                                            for idx in 0..6 {
                                                if let Ok(b) = u8::from_str_radix(&clean[idx*2..idx*2+2], 16) {
                                                    m[idx] = b;
                                                }
                                            }
                                            m
                                        } else {
                                            [0xFF; 6]
                                        }
                                    }
                                };

                                // 0x40 is OpCode::VmScriptDispatch
                                let mut payload = Vec::with_capacity(1 + val.len());
                                payload.push(0x40);
                                payload.extend_from_slice(&val);

                                let frame = io_oi_core::GatewayFrame {
                                    mac_addr: mac,
                                    payload,
                                };

                                if let Err(e) = s_tx.send(frame).await {
                                    eprintln!("[ServerGo] Failed to forward VM script frame to serial: {:?}", e);
                                } else {
                                    info!("[ServerGo] Forwarded VM script frame to MAC: {:02X?}", mac);
                                }
                            }
                        }
                        
                        let send_res = req.response_tx.send(Frame::SimpleString("OK".into()));
                        info!("[ServerGo Debug] Response send result for PUT: {:?}", send_res);
                    }
                    GatewayCommand::Info => {
                        let stats = format!(
                            "worker_id:{}\r\nversion:0.1.0\r\n",
                            0
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
    }

    #[cfg(not(target_os = "macos"))]
    {
        let cores = num_cpus::get();
        info!("Spawning {} worker threads (Thread-per-core architecture)", cores);
        
        let mut handles = vec![];
        for i in 0..cores {
            let node_clone = Arc::clone(&node);
            let bind_addr = args.bind.clone();
            let port = args.port;
            let serial_tx_clone = serial_tx.clone();
            
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

                            println!("[ServerGo Debug] Received request: {:?}", req.cmd);

                            match req.cmd {
                                GatewayCommand::Get(key) => {
                                    println!("[ServerGo Debug] L2Executor::get for key: {}", key);
                                    if let Some(val) = L2Executor::get(&node_clone, key.as_bytes()) {
                                        println!("[ServerGo Debug] Found key {} val len {}", key, val.len());
                                        let send_res = req.response_tx.send(Frame::BulkString(Some(Bytes::copy_from_slice(&val))));
                                        println!("[ServerGo Debug] Response send result: {:?}", send_res);
                                    } else {
                                        println!("[ServerGo Debug] Key {} not found", key);
                                        let send_res = req.response_tx.send(Frame::BulkString(None));
                                        println!("[ServerGo Debug] Response send result: {:?}", send_res);
                                    }
                                }
                                GatewayCommand::Put(key, val) => {
                                    println!("[ServerGo Debug] L2Executor::put for key: {}, val len: {}", key, val.len());
                                    L2Executor::put(&node_clone, key.clone().into_bytes(), val.clone());
                                    println!("[ServerGo Debug] L2Executor::put finished");
                                    
                                    // Forward VM scripts to Gateway Bridge over USB Serial
                                    if key.starts_with("vm:") {
                                        if let Some(ref s_tx) = serial_tx_clone {
                                            let mac = {
                                                let part = key.strip_prefix("vm:").unwrap_or(&key);
                                                if part.eq_ignore_ascii_case("broadcast") || part.eq_ignore_ascii_case("all") {
                                                    [0xFF; 6]
                                                } else {
                                                    let clean: String = part.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                                                    if clean.len() == 12 {
                                                        let mut m = [0u8; 6];
                                                        for idx in 0..6 {
                                                            if let Ok(b) = u8::from_str_radix(&clean[idx*2..idx*2+2], 16) {
                                                                m[idx] = b;
                                                            }
                                                        }
                                                        m
                                                    } else {
                                                        [0xFF; 6]
                                                    }
                                                }
                                            };

                                            // 0x40 is OpCode::VmScriptDispatch
                                            let mut payload = Vec::with_capacity(1 + val.len());
                                            payload.push(0x40);
                                            payload.extend_from_slice(&val);

                                            let frame = io_oi_core::GatewayFrame {
                                                mac_addr: mac,
                                                payload,
                                            };

                                            if let Err(e) = s_tx.send(frame).await {
                                                eprintln!("[ServerGo] Failed to forward VM script frame to serial: {:?}", e);
                                            } else {
                                                println!("[ServerGo] Forwarded VM script frame to MAC: {:02X?}", mac);
                                            }
                                        }
                                    }
                                    
                                    let send_res = req.response_tx.send(Frame::SimpleString("OK".into()));
                                    println!("[ServerGo Debug] Response send result for PUT: {:?}", send_res);
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
    }

    Ok(())
}
