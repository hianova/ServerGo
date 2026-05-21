# ServerGo Specification

## Architecture Overview

ServerGo is a high-performance distributed storage system built on the `io_oi` consensus protocol, utilizing `cdDB`'s high-performance columnar engine for both memory caching and persistent tiered storage.

### The 3-Tier Architecture

To achieve clean separation of concerns and maximum throughput, ServerGo is structured into three distinct layers:

1. **L1 - Network Layer (TCP/RESP)**: Handles parsing of the raw Redis protocol. Agnostic to underlying database internals, parsing bytes into command payloads and formatting responses.
2. **L2 - Execution Layer (`L2Executor` / `cdDB`)**: Core engine. Receives parsed commands and executes them against L3 via the high-level `cdDB` Query thread interface. It also offloads P2P consensus propagation (`broadcast_record`) to asynchronous tokio tasks, providing wait-free `SET` operations.
3. **L3 - Storage & Concurrency Layer (`cdDB`)**: The ultimate physical and memory boundary. Powered purely by `cdDB`'s columnar storage. Both `PureCacheStore` (in-memory only, No-WAL) and `TieredStore` (persistent with WAL) are built on top of `cdDB`'s high-level `Query` and `QuerySession` interfaces, which automatically coordinate wait-free RCU reads and worker QSBR epoch pinning without exposing unsafe pointer manipulation or raw worker states to L2.

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
    - **Wait-Free RCU via cdDB Thread Interface**: Uses `cdDB::Query` and `QuerySession` to process queries under high-performance thread-safe, lock-free RCU, completely removing lock bottlenecks (such as `dashmap` or `parking_lot` mutexes) from the hot path.
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

## Performance Claims

- **Latency**: Sub-millisecond read/write latency in `pure-cache` mode.
- **Throughput**: Scalable to millions of operations per second using the wait-free DualCache-FF engine.
- **Consistency**: Guaranteed by `io_oi` consensus protocol.
