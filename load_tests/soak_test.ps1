# Soak Test Script for Pollek DEK (24 hours)
$ErrorActionPreference = "Stop"

$durationHours = 24
$durationSeconds = $durationHours * 3600
$startTime = Get-Date

Write-Host "Starting 24-hour soak test for Pollek DEK at $startTime"

# Start dek-load-test in a loop, running 1-minute bursts every 5 minutes
$burstDuration = 60
$sleepDuration = 240

while ((Get-Date) -lt $startTime.AddSeconds($durationSeconds)) {
    $now = Get-Date
    Write-Host "Running 1-minute load burst at $now..."
    
    # Run dek-load-test
    cargo run -p dek-load-test -- --duration $burstDuration --concurrency 50
    
    Write-Host "Sleeping for 4 minutes..."
    Start-Sleep -Seconds $sleepDuration
}

Write-Host "Soak test completed successfully at $(Get-Date)."
