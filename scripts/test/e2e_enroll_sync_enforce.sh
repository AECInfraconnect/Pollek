#!/usr/bin/env bash
set -e

echo "Running E2E: Enroll -> Sync -> Enforce"
echo "Starting mock cloud..."
cargo run --bin mock-cloud -- --port 8443 &
MOCK_PID=$!
sleep 2

echo "Enrolling..."
cargo run --bin dek-cli -- enroll --cloud-url https://localhost:8443 --token test-token

echo "Starting core..."
cargo run --bin dek-core &
CORE_PID=$!
sleep 2

echo "Testing enforcement..."
cargo run --bin dek-cli -- status

echo "Tearing down..."
kill $CORE_PID
kill $MOCK_PID
echo "E2E passed!"
