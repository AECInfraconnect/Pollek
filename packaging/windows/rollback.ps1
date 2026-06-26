# ============================================================================
# packaging/windows/rollback.ps1
# ----------------------------------------------------------------------------
# Triggered by the Windows SCM "run program" failure action after PollekDEK
# exhausts its restart attempts. The service is STOPPED at this point, so the
# dek-core.exe file is unlocked and a plain copy works.
#
# It simply delegates to `dekctl rollback` (single source of truth, same logic
# as Linux), then starts the service back up.
# ============================================================================

$ErrorActionPreference = "Stop"
$ServiceName = "PollekDEK"
$InstallDir  = Join-Path $env:ProgramFiles "PollekDEK"
$Dekctl      = Join-Path $InstallDir "dekctl.exe"
$LogDir      = Join-Path $env:ProgramData "PollekDEK\logs"
$LogFile     = Join-Path $LogDir "rollback.log"

New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
function Log($m) { "$(Get-Date -Format o)  $m" | Tee-Object -FilePath $LogFile -Append }

Log "SCM failure action fired: attempting A/B rollback for $ServiceName"

# Make sure the service is fully stopped so the .exe is not locked.
try {
    $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($svc -and $svc.Status -ne 'Stopped') {
        Log "Stopping $ServiceName before rollback..."
        Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
        $svc.WaitForStatus('Stopped', (New-TimeSpan -Seconds 20))
    }
} catch { Log "WARN: could not confirm stopped state: $_" }

# Delegate to dekctl (restores .bak over dek-core.exe, clears the marker).
try {
    Log "Running: $Dekctl rollback"
    & $Dekctl rollback 2>&1 | Tee-Object -FilePath $LogFile -Append
    Log "dekctl rollback completed with exit code $LASTEXITCODE"
} catch {
    Log "ERROR: dekctl rollback failed: $_"
}

# Bring the (restored) service back.
try {
    Log "Starting $ServiceName..."
    Start-Service -Name $ServiceName
    Log "Service started."
} catch {
    Log "ERROR: failed to start service: $_"
    exit 1
}

exit 0


# ============================================================================
# INSTALL — register the failure actions (run once during MSI/post-install)
# ----------------------------------------------------------------------------
# reset= 60   : reset the failure counter after 60s of healthy running
# actions     : 1st fail -> restart after 2s
#               2nd fail -> restart after 2s
#               3rd fail -> run our rollback program (delay 0)
# Note: sc.exe needs the literal "actions=" with a trailing space, and the
# command must be wrapped so the SCM passes it as the failure "command".
#
#   sc.exe failure PollekDEK reset= 60 actions= restart/2000/restart/2000/run/0
#   sc.exe failureflag PollekDEK 1
#   sc.exe failure PollekDEK command= "powershell -ExecutionPolicy Bypass -NoProfile -File \"C:\Program Files\PollekDEK\rollback.ps1\""
#
# (failureflag 1 makes SCM apply failure actions even on "clean" non-zero exits,
#  which matches probation's std::process::exit(1) on abort.)
