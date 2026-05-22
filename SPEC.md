# ServerGo Specification

## Architecture Overview

ServerGo is a high-performance distributed storage system built on the `io_oi` consensus protocol, utilizing `cdDB`'s high-performance columnar engine for both memory caching and persistent tiered storage.

### The 3-Tier Architecture

To achieve clean separation of concerns and maximum throughput, ServerGo is structured into three distinct layers:

1. **L1 - Network Layer (TCP/RESP)**: Handles parsing of the raw Redis protocol. Agnostic to underlying database internals, parsing bytes into command payloads and formatting responses.
2. **L2 - Execution Layer (`L2Executor` / `cdDB`)**: Core engine. Receives parsed commands and executes them against L3 via the high-level `cdDB` Query thread interface. It also offloads P2P consensus propagation (`broadcast_record`) to asynchronous tokio tasks, providing wait-free `SET` operations.
3. **L3 - Storage & Concurrency Layer (`cdDB`)**: The ultimate physical and memory boundary. Powered purely by `cdDB`'s columnar storage. Both `PureCacheStore` (in-memory only, No-WAL) and `TieredStore` (persistent with WAL) are built on top of `cdDB`'s high-level `QuerySession` interface. To completely eliminate dynamic worker registration heap allocations and global `Mutex` locks on the read hot path, a thread-local worker cache (`WORKER_CACHE`) is used. Each thread registers with `cdDB`'s QSBR manager exactly once per partition route, achieving a 100% wait-free query session pinning with zero lock overhead.

### Core Components

1.  **Consensus Layer (`io_oi_core v2`)**:
    - Implements a decentralized leader-based consensus protocol.
    - Decoupled from storage via the `StateStore` trait.
    - **Governance Modes**:
        - `TrustMode`: Full (Broadcast) or Localized (Gossip-based).
        - `ControlMode`: Strict (Fixed Leader) or Competitive (WASM-based dynamic election).
        - Manages `SignedRecord` lifecycle and epoch finalization.

2.  **P2P Network Layer (`iroh 0.98`)**:
    - High-performance QUIC-based peer-to-peer communication.
    - Zero-config discovery and NAT traversal.
    - Supports `TrustMode::Localized` for large-scale decentralized clusters.

3.  **Storage Layer**:
    - **`L2Executor`**: The decoupled L2 execution boundary that coordinates business logic, caching, and persistence.
    - **`PureCacheStore`**: In-memory only mode using `cdDB` with WAL disabled (`NoopWal`), delivering wait-free in-memory query processing.
    - **`TieredStore`**: Write-through memory-cache utilizing a persistent `cdDB` partition with WAL enabled for transaction-safe disk durability.
    - **Wait-Free RCU via Thread-Local QSBR Caching**: Uses a thread-local worker state cache combined with `cdDB::QuerySession` to process queries under high-performance thread-safe, lock-free RCU. This bypasses the global dispatcher lock and dynamic heap allocations, completely removing lock and allocation bottlenecks from the read hot path.
    - **Zero-Copy Persistence**: Data is passed to `cdDB` as raw bytes, eliminating hex-string allocation overhead.

4.  **Wire Protocol Layer**:
    - **RESP Compatibility**: Built-in support for Redis Serialization Protocol (RESP) via `io_oi_node::RespGateway`.
    - **Request-Response Semantics**: Supports synchronous `GET` operations across the distributed network.
    - Implements a KV convention (Type 100) to map Redis commands to consensus records.

## Storage Trait: `StateStore`

```rust
pub trait StateStore: Send + Sync {
    fn get_record(&self, hash: &Hash32) -> Option<SignedRecord>;
    fn apply_signed_record(&self, record: SignedRecord);
    fn get_by_epoch(&self, epoch_id: EpochId) -> Vec<SignedRecord>;
    fn prune(&self, current_epoch: EpochId, k: u64);
}
```

## Protocol Integration

### Redis (RESP) Mapping

- **`SET key val`**:
    1.  Gateway parses `SET` and constructs a `SignedRecord`.
    2.  Record is applied to local `StateStore`.
    3.  Record is broadcast to peers via `iroh` (according to `TrustMode`).
- **`GET key`**:
    1.  Gateway queries local `StateStore` (Read-aside).
    2.  If data exists, returns value via RESP Bulk String.
    - Payload: `[hash(key) (32B)] + [val]`
    - Record Type: 100
    - Triggers consensus submission (direct apply in demo).
- **`GET key`**:
    - Computes `hash(key)`.
    - Queries storage for the record identified by this hash.
    - Extracts value from payload (skipping the 32B key hash header).
- **`PING`**: Returns `+PONG`.

## Platform-Specific Optimizations (Linux io_uring)

To maximize networking and disk I/O performance on modern Linux hosts, ServerGo implements platform-specific `io_uring` support:

1. **Target-Specific Cargo Selection**: Tokio's `io-uring` driver is compiled selectively on Linux targets to maintain full macOS/development compatibility:
   - Linux: Uses `tokio` with `["full", "io-uring"]` features.
   - Non-Linux: Uses `tokio` with `["full"]` features.
2. **Tokio Unstable Feature Activation**: Enabling the `io_uring` driver in standard Tokio is an unstable feature and requires the `--cfg tokio_unstable` compiler flag (`RUSTFLAGS` environment variable) during Linux target compilation. This enables the runtime to utilize the highly optimized Linux asynchronous system call ring interface for network packet handling.


## Security & Sandboxing (Linux seccomp)

To minimize the security attack surface in untrusted or multi-tenant production environments, ServerGo integrates kernel-level system call sandboxing:

1. **Linux seccomp-based Filtering**: On Linux `x86_64` and `aarch64` targets, ServerGo leverages Linux `seccomp` (Secure Computing Mode) to restrict allowed system calls to an absolute minimum necessary for database operation.
2. **Predefined Syscall Allow Lists**: Configures a strict allow-list composed of three core profiles from the `foundations::security` module:
   - `SERVICE_BASICS`: Minimum standard library and runtime bootstrap syscalls.
   - `ASYNC`: Necessary calls for asynchronous scheduler operations (epoll, timers).
   - `NET_SOCKET_API`: Sockets and network connection interface calls.
3. **Defense-in-Depth Hardening**: Any unapproved syscall attempts immediately trigger `ViolationAction::KillProcess`, mitigating arbitrary code execution exploits.
4. **Cross-Platform Compatibility**: Syscall sandboxing is compiled target-specifically using Rust conditional attributes (`#[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "aarch64")))]`), serving as a zero-overhead safe no-op on non-Linux development environments (e.g. macOS).


## Configuration & CLI Interface

ServerGo utilizes the robust `foundations` settings and CLI subsystem to manage configurations rather than relying on loose command-line flags:

### 1. Command-Line Arguments
The `foundations::cli::Cli` wrapper defines a standardized, clean set of system management options:
- `-c, --config <file>`: Specifies one or more YAML configuration files to boot the database server with.
- `-g, --generate <file>`: Generates a self-documenting YAML configuration file containing default settings, custom descriptions, and full telemetry schemas, then exits immediately.
- `-v, --version`: Prints the service version extracted directly from `Cargo.toml`.
- `-h, --help`: Prints standardized usage information.

### 2. YAML Configuration Schema (`config.yaml`)
A typical generated configuration file looks like this:
```yaml
# Port to bind the RESP server to
port: 6379
# IP address to bind the RESP server to
bind: 127.0.0.1
# Node ID (1-255)
id: 1
# Memory budget for the cache in MB
budget: 512
# Trust Mode: full (Broadcast) or localized (Gossip)
trust_mode: localized
# Control Mode: strict (Leader-only) or competitive (Decentralized)
control_mode: competitive
# Peer Iroh Ticket or Node ID to connect to
peer: ~
# Serial port to communicate with the Gateway ESP32
serial_port: ~
# Telemetry settings (logging, metrics, tracing)
telemetry:
  server:
    enabled: true
    addr: "127.0.0.1:8080"
  logging:
    output: terminal
    format: text
    verbosity: INFO
```

### 3. Environment Variables Override
In addition to configuration files, `foundations` allows overriding fields dynamically at runtime via environment variables using the `SERVERGO_` prefix:
- `SERVERGO_PORT`: Overrides the database binding port.
- `SERVERGO_BIND`: Overrides the database binding address.
- `SERVERGO_ID`: Overrides the Node ID.
- `SERVERGO_TELEMETRY_LOGGING_VERBOSITY`: Overrides the logging output verbosity (e.g. `DEBUG`, `INFO`, `WARN`).

## Observability & Telemetry

To ensure reliable production deployments and comprehensive cluster health monitoring, ServerGo integrates Cloudflare's `foundations` crate as its centralized telemetry framework:

1. **Service Metadata Initialization**: Uses the `foundations::service_info!()` macro to dynamically inspect `Cargo.toml` and extract service details (name, version, etc.) at startup, establishing a unified identity for logs, traces, and metrics.
2. **Unified HTTP Telemetry Server**: Configures and starts the built-in `foundations` telemetry server on `127.0.0.1:8080` (enabled via the `telemetry-server` feature flag), which automatically exposes:
   - **Prometheus Metrics Endpoint (`/metrics`)**: Exposes runtime metrics and our custom application-level metrics.
   - **Health Checks (`/health`)**: Exposes built-in server status and health monitoring.
   - **Memory Profiling**: Support for heap profiling via jemalloc integration.
3. **Custom Application-Level Metrics**: Implemented a dedicated declarative `#[metrics]` module in `src/metrics.rs` exposing:
   - `db_gets`: Monotonic counter tracking cumulative GET requests.
   - `db_puts`: Monotonic counter tracking cumulative PUT requests.
   - `cache_hits`: Monotonic counter tracking cache lookup hits in the memory layer.
   - `cache_misses`: Monotonic counter tracking cache lookup misses requiring backend storage lookups.
4. **Wait-Free Background Telemetry Driver**: The telemetry driver future (`TelemetryDriver`) is spawned asynchronously on the `tokio` event loop, ensuring that log/metric ingestion and HTTP scraping do not interfere with hot-path RCU query execution or P2P broadcast latencies.

## Performance Claims

- **Latency**:
  - `pure_get` (Read): **~38.62 ns** (Wait-Free memory column lookup)
  - `pure_apply` (Write): **~631.18 ns** (Memory column append)
  - `tiered_get` (Read): **~79.31 ns** (Wait-Free layered memory read)
  - `tiered_apply` (Write): **~6.22 µs** (Layered write with WAL persistence)
- **Throughput**: Single-core read throughput scales to **~25.8 M QPS** in pure cache mode, outperforming pure `DualCache-FF` and historical baselines.
- **Consistency**: Guaranteed by the `io_oi` consensus protocol.
