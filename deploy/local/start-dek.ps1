Write-Host "Starting Pollen DEK Local Control Plane..." -ForegroundColor Cyan

$DistPath = "apps\local-admin-dashboard\dist"
if (-not (Test-Path $DistPath)) {
    Write-Host "Building Local Admin Dashboard for the first time..." -ForegroundColor Yellow
    Push-Location "apps\local-admin-dashboard"
    npm install
    npm run build
    Pop-Location
}

# Kill any existing local-control-plane to free up the port and allow rebuilding
Write-Host "Cleaning up existing processes..."
Stop-Process -Name "local-control-plane" -ErrorAction SilentlyContinue

Write-Host "Compiling the Local Control Plane..." -ForegroundColor Yellow
cargo build -p local-control-plane --release

Write-Host "Starting Local Control Plane in background..." -ForegroundColor Yellow
$env:DEK_LCP_AUTH_DISABLE="1"
Start-Process -FilePath "target\release\local-control-plane.exe" -WindowStyle Hidden

Write-Host "Waiting for server to start..."
Start-Sleep -Seconds 3

Write-Host "Opening Dashboard at http://127.0.0.1:43891"
Start-Process "http://127.0.0.1:43891"

Write-Host "Done! The Local Control Plane is now running silently in the background." -ForegroundColor Cyan
Write-Host "To stop it, run: .\stop-dek.ps1" -ForegroundColor Gray
