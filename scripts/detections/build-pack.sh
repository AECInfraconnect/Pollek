#!/usr/bin/env bash
# Build and sign a Pollek detection pack for release.
#
# The checked-in local-dev pack is verified by CI through manifest SHA-256
# hashes. Release packs can use this script to issue a detached cosign bundle
# when OIDC/cosign credentials are available.
#
# Usage: scripts/detections/build-pack.sh contracts/detections/packs/core-v1
set -euo pipefail

PACK_DIR="${1:?usage: build-pack.sh <pack-dir>}"
cd "$PACK_DIR"

echo "==> validating rules against JSON schema"
# Requires: npx ajv-cli or any JSON-Schema validator. YAML is converted first.
for f in POLLEK-DET-*.yaml; do
  python3 -c "import sys,yaml,json; json.dump(yaml.safe_load(open('$f')), open('/tmp/$f.json','w'))"
  npx --yes ajv-cli@5 validate \
    -s ../../schema/detection-rule.schema.json \
    -d "/tmp/$f.json" --spec=draft2020 >/dev/null
done

echo "==> regenerating manifest.json with sha256 hashes"
python3 - "$PWD" <<'PY'
import datetime
import glob
import hashlib
import json
import os

rules = []
for path in sorted(glob.glob("POLLEK-DET-*.yaml")):
    with open(path, "r", encoding="utf-8") as handle:
        canonical = handle.read().replace("\r\n", "\n")
    digest = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
    with open(path, "r", encoding="utf-8") as handle:
        rule_id = handle.readline().split(":", 1)[1].strip()
    rules.append({"id": rule_id, "file": path, "sha256": digest})

manifest = {
    "schema_version": "1.0",
    "pack_id": os.path.basename(os.getcwd()),
    "version": os.environ.get("PACK_VERSION", "1.0.0"),
    "created": datetime.datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ"),
    "min_engine": "0.1.0",
    "rules": rules,
    "signature": {
        "method": "sha256-manifest-ci",
        "bundle": "not-issued-for-local-dev-pack",
    },
}
with open("manifest.json", "w", encoding="utf-8") as handle:
    json.dump(manifest, handle, indent=2)
    handle.write("\n")
PY

echo "==> signing manifest with cosign (keyless / OIDC)"
COSIGN_EXPERIMENTAL=1 cosign sign-blob --yes \
  --bundle manifest.json.cosign.bundle \
  manifest.json

python3 - <<'PY'
import json

with open("manifest.json", "r", encoding="utf-8") as handle:
    manifest = json.load(handle)
manifest["signature"] = {
    "method": "sigstore-cosign",
    "bundle": "manifest.json.cosign.bundle",
}
with open("manifest.json", "w", encoding="utf-8") as handle:
    json.dump(manifest, handle, indent=2)
    handle.write("\n")
PY

echo "==> done. Pack signed: $PACK_DIR/manifest.json and manifest.json.cosign.bundle"
