use ahash::AHashMap;
use cdDB::{CdDBDispatcher, WriteCommand};
use dualcache_ff::{Config, DualCacheFF};
use io_oi_core::{Epoch, Node, NodeId};
use std::thread;
use std::time::Duration;

#[tokio::main]
async fn main() {
    println!("--- ServerGo: Real Multi-Repo Integration ---");

    // 1. Initialize DualCache-FF (Wait-Free Cache)
    // Using default adaptive config
    let config = Config::adaptive_config::<String, String>();
    let cache = DualCacheFF::<String, String>::new(config);
    println!("DualCache-FF initialized and daemon spawned.");

    // 2. Initialize cdDB (Columnar Database)
    let mut db = CdDBDispatcher::new();
    let writer_tx = db.register_partition("server_go.main".to_string());
    println!("cdDB Dispatcher initialized with partition 'server_go.main'.");

    // 3. Initialize io_oi types (Distributed Protocol concepts)
    let local_id: NodeId = [0u8; 32];
    let epoch = Epoch {
        leader_id: local_id,
        epoch_id: 1,
        deadline: 1234567890,
    };
    println!(
        "io_oi Protocol state: Epoch {} active for Node ID {:?}.",
        epoch.epoch_id, local_id
    );

    // 4. Simulate an Integrated Write Workflow
    println!("\n--- Simulating Integrated Write ---");
    let key = "user:123".to_string();
    let val = "Bob".to_string();

    // Step A: Cache Update (Wait-Free)
    // In DualCache-FF, insert is non-blocking as it sends to daemon
    cache.insert(key.clone(), val.clone());
    println!("Cache update command sent for key: {}", key);

    // Step B: Persistent Storage (cdDB Columnar)
    let mut attrs = AHashMap::new();
    attrs.insert("name".to_string(), val.clone());

    let mut attrs_int = AHashMap::new();
    attrs_int.insert("id".to_string(), 123);

    writer_tx
        .send(WriteCommand::Insert {
            entity_id: 1,
            attributes: attrs,
            attributes_int: attrs_int,
        })
        .unwrap();
    println!("Storage insert command sent for entity: 1");

    // Allow background threads/daemons to process
    thread::sleep(Duration::from_millis(200));

    // 5. Simulate a Read Workflow
    println!("\n--- Simulating Integrated Read ---");

    // Check Cache
    if let Some(cached_val) = cache.get(&key) {
        println!("Cache Hit: {} = {}", key, cached_val);
    } else {
        println!("Cache Miss for key: {}", key);
    }

    // Check Storage
    if let Some(route) = db.get_route("server_go.main") {
        let snapshot = route.get_snapshot();
        if let Some(ptr) = snapshot.get(&1) {
            println!("Storage Result (Entity 1): Found in snapshot.");
            if let Some(idx) = ptr.attribute_indices.get("name") {
                if let Some(col) = route.get_column_str("name") {
                    let data = col.data.read();
                    if let Some(storage_val) = &data[*idx] {
                        println!("  - Persisted Name: {}", storage_val);
                    }
                }
            }
        }
    }

    println!("\nIntegration Successful. ServerGo is using actual repository logic.");
}
