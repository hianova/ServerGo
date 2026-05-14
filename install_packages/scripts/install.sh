#!/bin/bash

# ServerGo Automated Installation Script
# This script installs ServerGo as a systemd service on Linux.

set -e

echo "--- ServerGo Installer ---"

# 1. Check for Linux
if [[ "$OSTYPE" != "linux-gnu"* ]]; then
    echo "Error: This installation script is designed for Linux."
    exit 1
fi

# 2. Check for Root
if [ "$EUID" -ne 0 ]; then
    echo "Please run as root (sudo)."
    exit 1
fi

# 3. Setup Directories
INSTALL_DIR="/opt/servergo"
mkdir -p $INSTALL_DIR

# 4. Copy Binary (assuming it exists in current directory or target/release)
if [ -f "./ServerGo" ]; then
    cp ./ServerGo $INSTALL_DIR/
elif [ -f "./target/release/ServerGo" ]; then
    cp ./target/release/ServerGo $INSTALL_DIR/
else
    echo "Error: ServerGo binary not found. Please build it first using 'cargo build --release'."
    exit 1
fi

# 5. Create Service File
cat <<EOF > /etc/systemd/system/servergo.service
[Unit]
Description=ServerGo High-Performance Storage Node
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=$INSTALL_DIR
ExecStart=$INSTALL_DIR/ServerGo --port 6379 --id 1
Restart=always
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

# 6. Enable and Start
systemctl daemon-reload
systemctl enable servergo
systemctl start servergo

echo "--- Installation Complete ---"
echo "ServerGo is now running on port 6379."
echo "Use 'systemctl status servergo' to check status."
EOF
