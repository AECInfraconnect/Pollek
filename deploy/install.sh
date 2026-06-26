#!/usr/bin/env bash
set -euo pipefail

# Pollek DEK Installation Script

echo "--- Installing Pollek DEK ---"

if [ "$EUID" -ne 0 ]; then
  echo "Please run as root"
  exit 1
fi

DEK_VERSION=${1:-"latest"}
BIN_URL="https://github.com/pollek-cloud/pollek-dek/releases/download/${DEK_VERSION}/dek-linux-x86_64"

echo "1. Downloading DEK version ${DEK_VERSION}..."
curl -sL "${BIN_URL}" -o /usr/local/bin/pollek-dek
chmod +x /usr/local/bin/pollek-dek

echo "2. Setting up configuration directories..."
mkdir -p /etc/pollek
mkdir -p /var/lib/pollek

# If configuring via environment variables during install
if [ -n "${POLLEK_ENROLLMENT_TOKEN:-}" ]; then
  echo "Found enrollment token, generating initial config..."
  cat <<EOF > /etc/pollek/dek.yml
control_plane:
  endpoint: "https://cloud.pollek.internal:8443"
  tenant_id: "default"
enrollment:
  token: "${POLLEK_ENROLLMENT_TOKEN}"
EOF
fi

echo "3. Installing systemd service..."
cp dek.service /etc/systemd/system/dek.service
systemctl daemon-reload
systemctl enable dek.service
systemctl start dek.service

echo "--- Installation Complete ---"
echo "Check status with: systemctl status dek.service"
