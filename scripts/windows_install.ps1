param (
    [string]$InstallPath = "C:\Program Files\PollekDEK",
    [string]$Version = "latest",
    [string]$Repo = "AECInfraconnect/Pollek"
)

$ErrorActionPreference = "Stop"

# 1. Elevate if not admin
if (!([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host "Please run as Administrator."
    exit 1
}

Write-Host "Installing Pollek DEK to $InstallPath..."

# 2. Resolve version and download
if ($Version -eq "latest") {
    $ReleaseData = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Version = $ReleaseData.tag_name
}

$Base = "https://github.com/$Repo/releases/download/$Version"
$Bin = "dek-windows-x86_64.exe"

Write-Host "▶ 1/5 ดาวน์โหลด $Bin ($Version)"
Invoke-WebRequest -Uri "$Base/$Bin" -OutFile "$env:TEMP\$Bin"
Invoke-WebRequest -Uri "$Base/$Bin.sha256" -OutFile "$env:TEMP\$Bin.sha256"
Invoke-WebRequest -Uri "$Base/$Bin.sig" -OutFile "$env:TEMP\$Bin.sig"

Write-Host "▶ 2/5 ตรวจ checksum + ลายเซ็น (supply-chain)"
$ExpectedHash = (Get-Content "$env:TEMP\$Bin.sha256").Split(" ")[0]
$ActualHash = (Get-FileHash "$env:TEMP\$Bin" -Algorithm SHA256).Hash.ToLower()

if ($ActualHash -ne $ExpectedHash) {
    throw "Checksum verification failed! Expected: $ExpectedHash, Got: $ActualHash"
}

if (Get-Command cosign -ErrorAction SilentlyContinue) {
    cosign verify-blob --certificate-identity-regexp "https://github.com/$Repo/.github/workflows/.*" --certificate-oidc-issuer "https://token.actions.githubusercontent.com" --signature "$env:TEMP\$Bin.sig" "$env:TEMP\$Bin"
} else {
    Write-Host "⚠️  cosign ไม่ได้ติดตั้งข้ามการตรวจลายเซ็น (signature check skipped)"
}

Write-Host "▶ 3/5 ตรวจ dependency ของเครื่อง (preflight)"
& "$env:TEMP\$Bin" doctor --json > "$env:TEMP\dek-doctor.json"
if ($LASTEXITCODE -ne 0) {
    Write-Host "พบ dependency ที่ขาด — สรุปและวิธีแก้:"
    & "$env:TEMP\$Bin" doctor
    $a = Read-Host "ให้ลองติดตั้ง dependency ที่ติดตั้งได้อัตโนมัติเลยไหม? [y/N] "
    if ($a -eq "y" -or $a -eq "Y") {
        & "$env:TEMP\$Bin" doctor --fix
    }
}

Write-Host "▶ 4/5 ยอมรับข้อตกลง (Agreements)"
& "$env:TEMP\$Bin" agree
if ($LASTEXITCODE -ne 0) {
    throw "❌ ไม่สามารถติดตั้งได้หากไม่ยอมรับข้อตกลง"
}

Write-Host "▶ 5/5 ติดตั้ง + เริ่มบริการ"
if (!(Test-Path $InstallPath)) {
    New-Item -ItemType Directory -Force -Path $InstallPath | Out-Null
}
$DataDir = "C:\ProgramData\PollekDEK"
if (!(Test-Path $DataDir)) {
    New-Item -ItemType Directory -Force -Path $DataDir | Out-Null
}

Write-Host "Locking down permissions on $DataDir..."
icacls "$DataDir" /inheritance:r /grant "SYSTEM:(OI)(CI)F" /grant "Administrators:(OI)(CI)F" /T /C /Q

$WebView2RegPath = "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
$WebView2UserRegPath = "HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
if (!(Test-Path $WebView2RegPath) -and !(Test-Path $WebView2UserRegPath)) {
    Write-Host "Warning: Edge WebView2 runtime is not installed."
}

Copy-Item "$env:TEMP\$Bin" "$InstallPath\pollek-dek.exe" -Force
& "$InstallPath\pollek-dek.exe" service install
& "$InstallPath\pollek-dek.exe" service start

Write-Host "✅ เสร็จ — เปิด dashboard ที่ http://127.0.0.1:43891 หรือรัน: pollek-dek wizard"
