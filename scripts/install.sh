#!/usr/bin/env bash
# scripts/install.sh
set -euo pipefail

REPO="AECInfraconnect/Pollek"
VERSION="${1:-latest}"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
if [ "$ARCH" = "x86_64" ]; then
    ARCH="x86_64"
elif [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    ARCH="aarch64"
fi

if [ "$VERSION" = "latest" ]; then
    VERSION=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep -Po '"tag_name": "\K.*?(=")')
fi

BASE="https://github.com/${REPO}/releases/download/${VERSION}"
BIN="dek-${OS}-${ARCH}"

echo "▶ 1/5 ดาวน์โหลด ${BIN} (${VERSION})"
curl -fsSL "${BASE}/${BIN}"        -o /tmp/${BIN}
curl -fsSL "${BASE}/${BIN}.sha256" -o /tmp/${BIN}.sha256
curl -fsSL "${BASE}/${BIN}.sig"    -o /tmp/${BIN}.sig

echo "▶ 2/5 ตรวจ checksum + ลายเซ็น (supply-chain)"
( cd /tmp && sha256sum -c "${BIN}.sha256" )
if command -v cosign >/dev/null 2>&1; then
    cosign verify-blob \
      --certificate-identity-regexp "https://github.com/${REPO}/.github/workflows/.*" \
      --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
      --signature "/tmp/${BIN}.sig" "/tmp/${BIN}"
else
    echo "⚠️  cosign ไม่ได้ติดตั้งข้ามการตรวจลายเซ็น (signature check skipped)"
fi
chmod +x /tmp/${BIN}

echo "▶ 3/5 ตรวจ dependency ของเครื่อง (preflight)"
if ! /tmp/${BIN} doctor --json > /tmp/dek-doctor.json; then
  echo "พบ dependency ที่ขาด — สรุปและวิธีแก้:"
  /tmp/${BIN} doctor
  read -rp "ให้ลองติดตั้ง dependency ที่ติดตั้งได้อัตโนมัติเลยไหม? [y/N] " a
  if [[ "$a" == "y" || "$a" == "Y" ]]; then
      /tmp/${BIN} doctor --fix
  fi
fi

echo "▶ 4/5 ยอมรับข้อตกลง (Agreements)"
if ! /tmp/${BIN} agree; then
    echo "❌ ไม่สามารถติดตั้งได้หากไม่ยอมรับข้อตกลง"
    exit 1
fi

echo "▶ 5/5 ติดตั้ง + เริ่มบริการ"
install -m 0755 /tmp/${BIN} /usr/local/bin/pollek-dek
pollek-dek service install || true
pollek-dek service start || true

echo "✅ เสร็จ — เปิด dashboard ที่ http://127.0.0.1:43891 หรือรัน: pollek-dek wizard"
