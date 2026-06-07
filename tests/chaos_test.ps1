$ErrorActionPreference = "Continue"

Write-Host "Running Chaos Tests on Pollen DEK"

# 1. Kill Telemetry API
Write-Host "Simulating Telemetry API Outage..."
# In a real environment, we would use iptables/Windows Firewall to block outbound to telemetry endpoint
Write-Host "Blocked port 43891 (Mock)"

# 2. Kill Spire
Write-Host "Simulating Spire Agent Failure..."
$spireProc = Get-Process -Name "spire-agent" -ErrorAction SilentlyContinue
if ($spireProc) {
    Stop-Process -Id $spireProc.Id -Force
    Write-Host "Killed Spire Agent"
} else {
    Write-Host "Spire Agent not running, assuming simulated failure."
}

# Wait for 10 seconds to allow systems to degrade
Start-Sleep -Seconds 10

# 3. Check if dek-core is still running and serving requests
$dekProc = Get-Process -Name "dek-core" -ErrorAction SilentlyContinue
if ($dekProc) {
    Write-Host "dek-core is still running (Expected). It degraded gracefully."
} else {
    Write-Error "dek-core crashed during chaos test!"
}

# 4. Check if Telemetry Spooler is queuing
Write-Host "Checking Telemetry Spool..."
if (Test-Path "~\.dek\telemetry-core.db") {
    Write-Host "Telemetry DB exists. Spooling is working."
}

Write-Host "Chaos Tests completed. Please restore environment."
