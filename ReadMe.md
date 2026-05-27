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

Based on Criterion micro-benchmarks on Apple M1 (Pure Engine Mode):
- **Reads**: **~62 ns** (~16.03 Million ops/s via Zero-Copy TLS Cache)
- **Writes**: **~1.07 µs** (~934K ops/s via Adaptive Group Commit on Tiered WAL)

## 📦 Deployment & Orchestration

All deployment, orchestration, and benchmarking configurations are consolidated under the [deploy/](./deploy/) directory to keep the project root clean and organized.

### Multi-Node Cluster (Docker Compose)
To compile the system and spin up a local 5-node distributed cluster (along with a comparative Redis instance):

```bash
cd deploy
make build
make up
```

This starts:
- 5 ServerGo nodes on ports `6379`, `6380`, `6381`, `6382`, and `6383` respectively.
- A standard Redis baseline on port `6389` for benchmark comparisons.

Orchestration commands available in `deploy/Makefile`:
- `make build`: Rebuild the ServerGo Docker image.
- `make up` / `make down`: Start / stop the distributed cluster.
- `make release`: Build and package a release binary for macOS.
- `make release-linux`: Build and package a release binary for Linux (via Docker cross-compilation).
- `make chaos` / `make oom-test`: Run cluster resilience/OOM endurance suites.
- `make clean`: Bring down the containers and clean database/WAL assets.

### 🐧 Linux `io_uring` Acceleration
ServerGo compiles with platform-specific `io_uring` support enabled automatically when built for Linux environments.
- **How it works**: The `Cargo.toml` targets `tokio`'s unstable `io-uring` driver selectively on `cfg(target_os = "linux")` environments.
- **Docker builds**: The [deploy/Dockerfile](./deploy/Dockerfile) injects `ENV RUSTFLAGS="--cfg tokio_unstable"` during the build stage. This compiles and binds the high-performance async ring buffer system to the networking runtime.
- **macOS/Dev Compatibility**: Building locally on macOS uses the standard thread-per-core `epoll` equivalent without any additional requirements.

### 📊 Production-Grade Observability
ServerGo integrates Cloudflare's `foundations` telemetry framework to provide a robust, production-grade observability stack:
- **HTTP Telemetry Server**: Spawns an independent telemetry background server on `127.0.0.1:8080` (enabled via the `telemetry-server` feature flag), exposing Prometheus metrics at `/metrics` and built-in health checks at `/health`.
- **Custom Application Metrics**: Defines a declarative `#[metrics]` module to record `db_gets`, `db_puts`, `cache_hits`, and `cache_misses` with zero hot-path overhead.
- **Structured Logging**: Automatic structured logs routed through asynchronous buffers to ensure zero event-loop bottlenecks.
- **Jaeger Distributed Tracing**: Provides end-to-end tracing support for cluster request synchronization.
- **Service Metadata**: Leverages the `service_info!` macro to load service names, versions, and build details directly from `Cargo.toml`.


## 📘 Documentation & References

- 📘 [安裝教學 (Traditional Chinese)](./install_packages/安裝教學.md)
- 📖 [Deployment Tutorial (English)](./install_packages/deploy_tutorial.md)
- 📊 [Performance Report](./perf_report.md)
- 📦 [Installation Packages](./install_packages/) - Pre-built binaries and automation scripts.


## Quick Start

### 1. Generate the Configuration Template

ServerGo utilizes a production-grade, self-documenting YAML configuration file managed by `foundations`. Start by generating a default template:

```bash
cargo run --features "tiered-storage" -- --generate config.yaml
```

This creates a complete `config.yaml` file in the current directory. You can edit this file to configure the port, bind address, node ID, and custom telemetry options.

### 2. Build and Run

To start a node with tiered storage using the configuration file:

```bash
cargo run --features "tiered-storage" -- --config config.yaml
```

To join an existing cluster as Node 2:
1. Generate or copy another configuration file (e.g. `node2.yaml`).
2. Edit `node2.yaml` to specify different settings (such as `id: 2`, `port: 6380`, and add the P2P peer ticket to the `peer` property):
   ```yaml
   id: 2
   port: 6380
   peer: "<NODE_1_IROH_ID>"
   ```
3. Boot Node 2:
   ```bash
   cargo run --features "tiered-storage" -- --config node2.yaml
   ```

### 3. Advanced Governance

To alter governance modes, simply edit the corresponding fields inside the YAML configuration file under the core options:
- **Strict Control Mode** (only the initial leader can propose records): Set `control_mode: strict`
- **Full Trust Mode** (broadcast every record to all known peers): Set `trust_mode: full`

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