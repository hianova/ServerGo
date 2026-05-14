# ServerGo Deployment Tutorial

This document provides step-by-step instructions for deploying ServerGo in a production environment.

## 1. Prerequisites

- **Rust Toolchain**: Install via `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Build Tools**: `gcc`, `make`, `cmake`, `libssl-dev` (on Ubuntu)
- **Redis Tools**: `redis-tools` (for `redis-cli` and `redis-benchmark`)

## 2. Compilation Strategies

### Optimized Release Build
For the best performance, always use the `--release` flag.

```bash
# For pure in-memory wait-free performance
cargo build --release --no-default-features --features "pure-cache"

# For persistence support (tiered storage)
cargo build --release --features "tiered-storage"
```

### Cross-Compilation for Linux Targets
If you are developing on macOS or Windows and want to target Linux servers:

1.  **Install Cross**: `cargo install cross`
2.  **Build for x86_64**: `cross build --target x86_64-unknown-linux-gnu --release`
3.  **Build for ARM64**: `cross build --target aarch64-unknown-linux-gnu --release`

## 3. Production Setup (SSH & Linux)

### Deployment Script Template (`deploy.sh`)

```bash
#!/bin/bash
SERVER_IP="1.2.3.4"
TARGET_DIR="/opt/servergo"

# 1. Build locally
cargo build --release --features "pure-cache"

# 2. Upload to server
ssh root@$SERVER_IP "mkdir -p $TARGET_DIR"
scp ./target/release/ServerGo root@$SERVER_IP:$TARGET_DIR/

# 3. Restart service
ssh root@$SERVER_IP "systemctl restart servergo"
```

## 4. Running as a Service

We recommend using `systemd` to manage the ServerGo process.

### Configuration (`/etc/systemd/system/servergo.service`)
```ini
[Unit]
Description=ServerGo Storage Node
After=network.target

[Service]
ExecStart=/opt/servergo/ServerGo --port 6379 --id 1
Restart=always
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
```

### Tuning for High Performance
To achieve maximum throughput, increase the open file limit:
```bash
ulimit -n 65535
```

## 5. Clustering & Governance

To start multiple nodes and form a decentralized cluster:
1.  **Unique Identity**: Assign a unique `--id` (1-255) to each node.
2.  **P2P Connectivity**: For subsequent nodes, use `--peer <IROH_NODE_ID>` to connect to the initial node.
3.  **Governance Tuning**:
    - Use `--control-mode competitive` (default) for decentralized dynamic leader election.
    - Use `--trust-mode full` if you are in a private network and want low-latency broadcast sync.

Example Cluster Command:
```bash
# Node 1
./ServerGo --id 1 --port 6379

# Node 2
./ServerGo --id 2 --port 6380 --peer <NODE_1_IROH_ID>
```

## 6. Verification

Run the official Redis benchmark:
```bash
redis-benchmark -h <server-ip> -p 6379 -t get,set -q -n 1000000
```
