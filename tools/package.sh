#!/bin/bash

# ServerGo Packaging Script
# This script builds all targets including Windows and organizes them into ./install_packages

set -e

echo "Starting packaging process..."

# 1. Clean and Create Folders
mkdir -p install_packages/bin/linux_x64
mkdir -p install_packages/bin/linux_arm64
mkdir -p install_packages/bin/macos
mkdir -p install_packages/bin/windows
mkdir -p install_packages/scripts

# 2. Build and Archive (Mocked if cross is missing, but commands are provided)
echo "Building Linux x86_64..."
if command -v cross &> /dev/null; then
    cross build --target x86_64-unknown-linux-gnu --release --no-default-features --features "pure-cache"
    cp target/x86_64-unknown-linux-gnu/release/ServerGo install_packages/bin/linux_x64/
else
    echo "Warning: 'cross' not found. Skipping Linux x86_64 build."
fi

echo "Building Linux ARM64..."
if command -v cross &> /dev/null; then
    cross build --target aarch64-unknown-linux-gnu --release --no-default-features --features "pure-cache"
    cp target/aarch64-unknown-linux-gnu/release/ServerGo install_packages/bin/linux_arm64/
else
    echo "Warning: 'cross' not found. Skipping Linux ARM64 build."
fi

echo "Building Windows x86_64..."
if command -v cross &> /dev/null; then
    cross build --target x86_64-pc-windows-gnu --release --no-default-features --features "pure-cache"
    cp target/x86_64-pc-windows-gnu/release/ServerGo.exe install_packages/bin/windows/
else
    echo "Warning: 'cross' not found. Skipping Windows x86_64 build."
fi

echo "Building macOS Native..."
cargo build --release --no-default-features --features "pure-cache"
cp target/release/ServerGo install_packages/bin/macos/

# 3. Create a Master Installer in root of install_packages
cat <<EOF > install_packages/quick_install.sh
#!/bin/bash
OS="\$(uname -s)"
ARCH="\$(uname -m)"

echo "Detected OS: \$OS, Arch: \$ARCH"

if [ "\$OS" == "Darwin" ]; then
    BINARY="./bin/macos/ServerGo"
elif [ "\$OS" == "Linux" ]; then
    if [ "\$ARCH" == "x86_64" ]; then
        BINARY="./bin/linux_x64/ServerGo"
    elif [ "\$ARCH" == "aarch64" ] || [ "\$ARCH" == "arm64" ]; then
        BINARY="./bin/linux_arm64/ServerGo"
    fi
elif [[ "\$OS" == *"MINGW"* ]] || [[ "\$OS" == *"CYGWIN"* ]] || [[ "\$OS" == *"MSYS"* ]]; then
    BINARY="./bin/windows/ServerGo.exe"
fi

if [ -f "\$BINARY" ]; then
    echo "Using binary: \$BINARY"
    cp \$BINARY ./ServerGo
    if [ "\$OS" == "Linux" ]; then
        sudo bash ./scripts/install.sh
    else
        echo "Installation script is only supported on Linux. For other platforms, simply run ./ServerGo"
    fi
else
    echo "Error: No matching binary found for your platform."
    exit 1
fi
EOF

chmod +x install_packages/quick_install.sh

echo "Packaging complete! All files are in ./install_packages"
