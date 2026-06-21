#!/usr/bin/env bash
set -e

echo "Starting DEK Core Supervisor..."
cargo run --bin dek-core -- --config config.toml
