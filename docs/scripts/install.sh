#!/usr/bin/env bash
set -e

# Pollen DEK Installation Script (Linux / macOS)

echo "Installing Pollen DEK v1.0.0-beta..."

INSTALL_DIR="/opt/pollen-dek"
CONFIG_DIR="/etc/pollen-dek"
DATA_DIR="/var/lib/pollen-dek"

# Require root
if [ "$EUID" -ne 0 ]; then
  echo "Please run as root"
  exit 1
fi

mkdir -p "$INSTALL_DIR"
mkdir -p "$CONFIG_DIR"
mkdir -p "$DATA_DIR"

# Assume binaries are either in current directory (if downloaded from release) or provide instructions
if [ ! -f "dek-core" ] && [ ! -f "dek-mcp-proxy" ]; then
    echo "Error: Binaries not found in current directory. Please extract the release tarball before running this script."
    exit 1
fi

echo "Copying binaries..."
cp dek-core "$INSTALL_DIR/"
cp dek-mcp-proxy "$INSTALL_DIR/"
cp dek-mcp-stdio-wrapper "$INSTALL_DIR/"
cp dekctl "$INSTALL_DIR/"

chmod +x "$INSTALL_DIR/"*

# Setup systemd service for Linux
if [ -d "/etc/systemd/system" ]; then
    echo "Configuring systemd service..."
    cat <<EOF > /etc/systemd/system/pollen-dek.service
[Unit]
Description=Pollen DEK Core
After=network.target

[Service]
ExecStart=$INSTALL_DIR/dek-core
Restart=always
User=root
Environment=POLLEN_CLOUD_URL=https://127.0.0.1:43891

[Install]
WantedBy=multi-user.target
EOF
    systemctl daemon-reload
    systemctl enable pollen-dek
    echo "Service enabled. Run 'systemctl start pollen-dek' to start."
else
    echo "Systemd not detected. Please start $INSTALL_DIR/dek-core manually."
fi

echo "Pollen DEK Installation Complete."
