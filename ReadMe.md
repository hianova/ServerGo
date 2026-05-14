# ServerGo

**ServerGo** is an advanced, high-performance distributed database server that integrates the `io_oi` consensus protocol with tiered storage engines (`DualCache-FF` and `cdDB`).

## Features

- 🚀 **Wait-Free Performance**: Powered by DualCache-FF, reaching **100M+ QPS** for reads.
- 🔗 **Decoupled Architecture**: Storage-agnostic consensus logic via `StateStore` trait.
- 🛡️ **Distributed Governance**: Customizable `TrustMode` (Full/Localized) and `ControlMode` (Strict/Competitive).
- 💾 **Tiered Storage**: High-speed memory cache + Persistent columnar database (`cdDB`).
- 🌐 **Modern P2P Connectivity**: Integrated with `iroh 0.98` for robust distributed synchronization.
- 🔌 **Redis Compatibility**: Wire-level RESP protocol support with request-response semantics.

## Performance

Based on Criterion micro-benchmarks on Apple M1:
- **Reads**: **9.43 ns** (~106 Million ops/s)
- **Writes**: **113.37 ns** (~8.8 Million ops/s)

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
Project created for high-performance distributed systems research.
