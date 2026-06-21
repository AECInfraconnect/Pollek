#!/usr/bin/env bash
set -e

echo "Enrolling local device..."
cargo run --bin dek-cli -- enroll --cloud-url https://localhost:8443 --token dev-token-123
