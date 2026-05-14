#!/bin/bash
OS="$(uname -s)"
ARCH="$(uname -m)"

echo "Detected OS: $OS, Arch: $ARCH"

if [ "$OS" == "Darwin" ]; then
    BINARY="./bin/macos/ServerGo"
elif [ "$OS" == "Linux" ]; then
    if [ "$ARCH" == "x86_64" ]; then
        BINARY="./bin/linux_x64/ServerGo"
    elif [ "$ARCH" == "aarch64" ] || [ "$ARCH" == "arm64" ]; then
        BINARY="./bin/linux_arm64/ServerGo"
    fi
fi

if [ -f "$BINARY" ]; then
    echo "Using binary: $BINARY"
    cp $BINARY ./ServerGo
    sudo bash ./scripts/install.sh
else
    echo "Error: No matching binary found for your platform."
    exit 1
fi
