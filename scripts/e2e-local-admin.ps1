$ErrorActionPreference = "Stop"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p dek-control-plane-api
cargo test -p local-control-plane

$env:DEK_LCP_AUTH_DISABLE="1"
$env:DEK_LCP_BIND = if ($env:DEK_LCP_BIND) { $env:DEK_LCP_BIND } else { "127.0.0.1:5174" }
$env:DEK_LCP_DB="sqlite://./target/e2e/pollen-local.db?mode=rwc"
$env:DEK_LCP_DATA="./target/e2e/pollen-local-data"
$env:DEK_DASHBOARD_DIR=(Resolve-Path "apps/local-admin-dashboard/dist").Path
$env:PLAYWRIGHT_BASE_URL="http://$env:DEK_LCP_BIND"

$proc = Start-Process cargo -ArgumentList "run -p local-control-plane" -PassThru -WindowStyle Hidden

try {
  $healthy = $false
  for ($i = 1; $i -le 30; $i++) {
    try {
      Invoke-RestMethod "$env:PLAYWRIGHT_BASE_URL/health" | Out-Null
      $healthy = $true
      break
    }
    catch {
      Start-Sleep -Seconds 1
    }
  }
  if (-not $healthy) {
    throw "local-control-plane did not become ready at $env:PLAYWRIGHT_BASE_URL"
  }

  cargo test -p local-control-plane --test e2e_registry
  cargo test -p local-control-plane --test e2e_policy_publish

  Push-Location apps/local-admin-dashboard
  npm ci
  npm run build
  $env:DEK_PLAYWRIGHT_EXTERNAL_SERVER="1"
  npx playwright test
  Pop-Location
}
finally {
  Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
}
