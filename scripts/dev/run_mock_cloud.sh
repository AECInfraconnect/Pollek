#!/usr/bin/env bash
set -e

echo "Starting Mock Cloud..."
cargo run --bin mock-cloud -- --port 8443
