# 🚀 ServerGo: High-Performance Cache & State Node

ServerGo is a high-performance database node built on top of **io_oi v2** and **DualCache-FF**. It is designed for ultra-low latency data access and reliable async persistence using **cdDB**.

> [!NOTE]
> While ServerGo is designed to be part of a distributed consensus network via the `io_oi` protocol, current development and benchmarking focus on **single-node performance** and **local-first consistency**. True multi-node distributed consensus is currently in the experimental stage.

## 🌟 Core Architecture
- **⚡ DualCache-FF L1/L2**: A two-tier cache (Wait-Free RAM + Disk) for microsecond-level access.
- **🛡️ io_oi v2 Integration**: Uses the io_oi protocol for record signing, epoch management, and P2P gossip sync.
- **💾 cdDB Persistent Engine**: A columnar storage backend for long-term data durability with backpressure support.
- **🔌 RESP Protocol Compatibility**: Talk to ServerGo using any standard Redis client.
- 💾 **Tiered Storage**: High-speed memory cache + Persistent columnar database (`cdDB`).
- 🌐 **Modern P2P Connectivity**: Integrated with `iroh 0.98` for robust distributed synchronization.
- 🔌 **Redis Compatibility**: Wire-level RESP protocol support with request-response semantics.

## Performance

Based on Criterion micro-benchmarks on Apple M1:
- **Reads**: **45 ns** (~22 Million ops/s)
- **Writes**: **268 ns** (~3.7 Million ops/s)

## Documentation & Deployment

- 📘 [安裝教學 (Traditional Chinese)](./install_packages/安裝教學.md)
- 📖 [Deployment Tutorial (English)](./install_packages/deploy_tutorial.md)
- 📊 [Performance Report](./perf_report.md)
- 📦 [Installation Packages](./install_packages/) - Pre-built binaries and automation scripts.

## Quick Start

### Build and Run

To start a node with tiered storage and default governance (Localized + Competitive):

```bash
cargo run --features "tiered-storage" -- --id 1 --port 6379
```

To join an existing cluster as Node 2:

```bash
# Provide the iroh node ID of Node 1 to connect
cargo run --features "tiered-storage" -- --id 2 --port 6380 --peer <NODE_1_IROH_ID>
```

### Advanced Governance

```bash
# Strict mode: Only initial leader can propose records
cargo run -- --control-mode strict

# Full trust mode: Broadcast every record to all known peers
cargo run -- --trust-mode full
```

### Accessing via Redis Client

Once the server is running, you can connect using any Redis client:

```bash
# Connect using redis-cli
redis-cli -p 6379 SET mykey "Hello ServerGo"
redis-cli -p 6379 GET mykey
```

## Internal Architecture

ServerGo acts as a high-performance wrapper around:
- **io_oi v2**: Decentralized consensus protocol with iroh P2P.
- **DualCache-FF**: High-performance, wait-free cache engine.
- **cdDB**: Tiered storage and columnar database.

See [SPEC.md](SPEC.md) for detailed technical specifications.

## License
[PolyForm-Noncommercial-1.0.0](PolyForm-Noncommercial-1.0.0.txt)