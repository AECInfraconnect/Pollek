<#
.SYNOPSIS
Pollen DEK Installation Script (Windows)

.DESCRIPTION
Installs Pollen DEK Core and sets up the Windows Service.
#>

Write-Host "Installing Pollen DEK v1.0.0-beta..."

$InstallDir = "$env:ProgramFiles\PollenDEK"
$ConfigDir = "$env:ProgramData\PollenDEK\Config"
$DataDir = "$env:ProgramData\PollenDEK\Data"

# Check Admin
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "Please run this script as Administrator." -ForegroundColor Red
    exit 1
}

New-Item -Path $InstallDir -ItemType Directory -Force | Out-Null
New-Item -Path $ConfigDir -ItemType Directory -Force | Out-Null
New-Item -Path $DataDir -ItemType Directory -Force | Out-Null

$Files = @("dek-core.exe", "dek-mcp-proxy.exe", "dek-mcp-stdio-wrapper.exe", "dekctl.exe")
foreach ($File in $Files) {
    if (-not (Test-Path $File)) {
        Write-Host "Error: $File not found in current directory. Extract the release zip before running." -ForegroundColor Red
        exit 1
    }
    Copy-Item $File -Destination $InstallDir -Force
}

Write-Host "Binaries copied to $InstallDir"

# Install Windows Service using dek-core itself (since it uses service_integration)
# Normally you'd use sc.exe or New-Service, but if dek-core handles it:
$CorePath = Join-Path $InstallDir "dek-core.exe"

# Alternatively, create the service explicitly:
$ServiceName = "PollenDEKCore"
$ExistingService = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue

if ($ExistingService) {
    Stop-Service -Name $ServiceName -Force
    sc.exe delete $ServiceName
}

New-Service -Name $ServiceName -BinaryPathName $CorePath -DisplayName "Pollen DEK Core Service" -StartupType Automatic | Out-Null

Start-Service -Name $ServiceName
Write-Host "Pollen DEK Core Service Installed and Started." -ForegroundColor Green
