$ErrorActionPreference = "Stop"

$workspaceDir = Split-Path -Parent $MyInvocation.MyCommand.Definition | Split-Path -Parent

Write-Host "Building release binaries..."
Set-Location $workspaceDir
cargo build --release

Write-Host "Generating SBOM..."
cargo cyclonedx --all --format xml
if (Test-Path "bom.xml") {
    Write-Host "SBOM generated successfully at bom.xml"
} else {
    Write-Error "SBOM generation failed."
}

Write-Host "Packaging dek-core MSI..."
# cargo wix will automatically use wix/main.wxs if configured, or default
cargo wix -p dek-core --no-build

Write-Host "Release process completed."
