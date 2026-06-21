#!/usr/bin/env bash
set -euo pipefail

mkdir -p ./target/e2e
export DEK_LCP_AUTH_DISABLE=1
export DEK_LCP_DB='sqlite://./target/e2e/pollen-local.db?mode=rwc'
export DEK_LCP_DATA='./target/e2e/pollen-local-data'
export DEK_DASHBOARD_DIR="$(pwd)/apps/local-admin-dashboard/dist"

pushd apps/local-admin-dashboard
npm ci
npm run build
popd

cargo run -p local-control-plane &
LCP_PID=$!
trap 'kill $LCP_PID || true' EXIT
for i in $(seq 1 30); do curl -fsS http://127.0.0.1:3000/health && break; sleep 1; done
cargo test -p local-control-plane --test e2e_registry
cargo test -p local-control-plane --test e2e_policy_publish

pushd apps/local-admin-dashboard
npx playwright install --with-deps
npx playwright test
popd
