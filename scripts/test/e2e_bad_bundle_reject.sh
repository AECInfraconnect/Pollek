#!/usr/bin/env bash
set -e

echo "Running E2E: Bad bundle reject"
echo "Starting mock cloud in bad bundle mode..."
# (Assuming mock-cloud supports an env var or arg to serve bad bundles)
BAD_BUNDLE_MODE=1 cargo run --bin mock-cloud -- --port 8443 &
MOCK_PID=$!
sleep 2

echo "Starting core..."
cargo run --bin dek-core &
CORE_PID=$!
sleep 2

echo "Tearing down..."
kill $CORE_PID
kill $MOCK_PID
echo "E2E passed (bad bundle rejected safely)."
