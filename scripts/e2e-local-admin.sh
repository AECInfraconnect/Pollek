#!/usr/bin/env bash
set -euo pipefail

mkdir -p ./target/e2e
export DEK_LCP_AUTH_DISABLE=1
export DEK_LCP_BIND="${DEK_LCP_BIND:-127.0.0.1:5174}"
export DEK_LCP_DB='sqlite://./target/e2e/pollen-local.db?mode=rwc'
export DEK_LCP_DATA='./target/e2e/pollen-local-data'
export DEK_DASHBOARD_DIR="$(pwd)/apps/local-admin-dashboard/dist"
export PLAYWRIGHT_BASE_URL="http://${DEK_LCP_BIND}"

pushd apps/local-admin-dashboard
npm ci
npm run build
popd

cargo run -p local-control-plane &
LCP_PID=$!
trap 'kill $LCP_PID || true' EXIT
for i in $(seq 1 300); do
  if curl -fsS "${PLAYWRIGHT_BASE_URL}/health"; then
    break
  fi
  if [ "$i" -eq 300 ]; then
    echo "local-control-plane did not become ready at ${PLAYWRIGHT_BASE_URL}" >&2
    exit 1
  fi
  sleep 1
done
cargo test -p local-control-plane --test e2e_registry
cargo test -p local-control-plane --test e2e_policy_publish

pushd apps/local-admin-dashboard
npx playwright install --with-deps
DEK_PLAYWRIGHT_EXTERNAL_SERVER=1 PLAYWRIGHT_BASE_URL="${PLAYWRIGHT_BASE_URL}" npx playwright test
popd
