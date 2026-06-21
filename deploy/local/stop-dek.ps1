Write-Host "Stopping Pollen DEK Local Control Plane..." -ForegroundColor Yellow
$existing = Get-Process -Name "local-control-plane" -ErrorAction SilentlyContinue
if ($existing) {
    Stop-Process -Name "local-control-plane" -Force
    Write-Host "Stopped successfully." -ForegroundColor Green
} else {
    Write-Host "Not running." -ForegroundColor Gray
}
