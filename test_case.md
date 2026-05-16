# ServerGo Test & Benchmark Cases

This document lists all available tests and benchmarks for verifying the performance, reliability, and correctness of ServerGo and the io_oi consensus engine.

## 1. Rust Unit & Integration Tests (`tests/`)

These tests verify the core logic and networking layers using standard Rust testing infrastructure.

| Test Case | Description | Command |
| :--- | :--- | :--- |
| `test_resp_server` | Verifies the RESP gateway (PING, SET, GET, INFO) and basic storage interaction. | `cargo test --test integration_tests` |
| `test_concurrent_apply_and_get` | Loom-based concurrency test to check for race conditions in storage engines. | `cargo test --test loom_tests --features loom` |

## 2. Rust Performance Benchmarks (`benches/`)

Criterion-based benchmarks for fine-grained performance measurement of internal components.

| Benchmark Case | Component | Description | Command |
| :--- | :--- | :--- | :--- |
| `apply_record` | `PureCacheStore` | Measures throughput and latency of record insertion. | `cargo bench --bench storage_bench` |
| `get_record` | `PureCacheStore` | Measures retrieval speed from the high-performance cache. | `cargo bench --bench storage_bench` |

## 3. System-Level Stress & Chaos Tests (`tools/`)

High-level scripts designed to validate the system under extreme conditions or simulate real-world disasters.

| Test Script | Strategy | Description | Command |
| :--- | :--- | :--- | :--- |
| `ram_crusher.py` | OOM Survival | Compares Redis vs ServerGo under strict memory limits (4GB) with 6GB+ data. | `make oom-test` |
| `chaos_test.py` | Chaos Compose | Injects network latency (500ms) and packet loss (30%) into a 5-node cluster. | `make chaos` |
| `bench.sh` | QPS Benchmark | Standardized `redis-benchmark` wrapper for SET/GET/MSET performance. | `./tools/bench.sh` |
| `verify_sync.py` | Consistency | Verifies that all nodes in the cluster have synchronized state. | `python3 tools/verify_sync.py` |

## 4. Operational & Monitoring Tools

| Tool | Description | Command |
| :--- | :--- | :--- |
| `dashboard.py` | TUI for real-time monitoring of cluster node status, epochs, and modes. | `make dashboard` |
| `entrypoint.sh` | Docker orchestration script for controlled node startup and discovery. | (Used by Docker) |

---
*Last Updated: 2026-05-16*
