# 🚀 ServerGo

ServerGo is a high-performance database node built on top of **io_oi v2** and **DualCache-FF**. It is designed for ultra-low latency data access and reliable async persistence.

> [!NOTE]
> While ServerGo is designed to be part of a distributed consensus network via the `io_oi` protocol, current development and benchmarking focus on **single-node performance** and **local-first consistency**.

## 🛠️ Technologies Used

- **Rust**: The core language providing memory safety and extreme performance.
- **cdDB**: A highly optimized, custom columnar database engine for high-speed Wait-Free RAM caching and disk persistence.
- **io_oi**: A robust decentralized consensus and P2P gossip sync protocol.
- **iroh (0.98)**: State-of-the-art peer-to-peer networking stack for zero-config discovery and QUIC connections.
- **Tokio & io_uring**: The async runtime, optionally compiled with Linux's `io_uring` for maximum I/O throughput.
- **Foundations**: Cloudflare's telemetry framework providing Prometheus metrics, HTTP servers, structured logging, and distributed tracing.
- **Seccomp**: Automatic kernel-level syscall sandboxing on Linux targets to secure the runtime against arbitrary execution.

## 📦 Installation & Quick Start

ServerGo requires Rust to be installed. Once installed, you can build and run the node easily.

### 1. Build from Source
```bash
# Clone the repository
git clone https://github.com/yourusername/ServerGo.git
cd ServerGo

# Build the release binary
cargo build --release
```

### 2. Generate Configuration
ServerGo uses a production-grade YAML configuration file managed by `foundations`. Start by generating a default template:
```bash
./target/release/ServerGo --generate config.yaml
```

### 3. Run the Node
To start a node with tiered storage enabled:
```bash
./target/release/ServerGo --features "tiered-storage" --config config.yaml
```

*(Alternatively, use Docker Compose for multi-node clusters: `cd deploy && make build && make up`)*

### 4. Connect via Redis Client
ServerGo is RESP-compatible. You can query it like any Redis server:
```bash
redis-cli -p 6379 SET mykey "Hello ServerGo"
redis-cli -p 6379 GET mykey
```

## ✨ Core Features

- **DualCache-FF L1/L2**: A two-tier cache delivering extreme microsecond-level access. Wait-Free `GlobalHotIndex` (`arc-swap`) ensures zero-lock reads.
- **Zero-Copy Architecture**: TLS Wait-Free RCU bypasses dynamic heap allocations and Mutex locks, achieving reads as low as ~127 ns on Apple Silicon.
- **Tiered Columnar Storage**: Safely persists data to disk via `cdDB`'s async Write-Ahead Log (WAL) without blocking hot-path cache reads.
- **RESP Protocol Compatibility**: Seamlessly drops into existing architectures using standard Redis clients for `GET`, `SET`, `PING`, and `INFO`.
- **Automatic OS Sandboxing**: Transparently leverages Linux `seccomp` to restrict unapproved system calls for defense-in-depth security.
- **Built-in Observability**: Includes an HTTP metrics server (`/metrics`), structured logs, and application-level cache hit/miss tracking out of the box.

## 📘 Documentation
For more detailed technical insights, please refer to [SPEC.md](SPEC.md). 

Historical performance reports and audits are archived in the [docs/](docs/) directory:
- [PERF.md](docs/PERF.md)
- [test_bench_audit.md](docs/test_bench_audit.md)
- [servergo_improvements.md](docs/servergo_improvements.md)
- [test_case.md](docs/test_case.md)

## License
[PolyForm-Noncommercial-1.0.0](PolyForm-Noncommercial-1.0.0.txt)
