# ServerGo Specification

## Architecture Overview

ServerGo is a high-performance distributed storage system built on the `io_oi` consensus protocol, utilizing `DualCache-FF` for wait-free caching and `cdDB` for persistent tiered storage.

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
    - **`PureCacheStore`**: In-memory only mode using `DualCache-FF`. Ideal for high-throughput edge nodes.
    - **`TieredStore`**: Write-through cache with asynchronous persistence to `cdDB` columnar storage.
    - **Wait-Free SeqLock**: Concurrency model ensures linearizable reads without mutex contention.

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

## Performance Claims

- **Latency**: Sub-millisecond read/write latency in `pure-cache` mode.
- **Throughput**: Scalable to millions of operations per second using the wait-free DualCache-FF engine.
- **Consistency**: Guaranteed by `io_oi` consensus protocol.
