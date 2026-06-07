# Runbook: Disaster Recovery

## Objective
To recover the Pollen DEK from severe states such as corrupted bundles, failed binary updates, or persistent crash loops.

## Scenario A: Failed Binary Update (Crash Loop)
If `dek-updater` failed to swap correctly or the new binary panics immediately:
1. **Stop Service**
   ```powershell
   Stop-Service PollenDEK -Force
   ```
2. **Restore Backup Binary**
   ```powershell
   Rename-Item -Path "C:\Program Files\PollenDEK\dek-core.exe.bak" -NewName "dek-core.exe"
   ```
3. **Restart Service**
   ```powershell
   Start-Service PollenDEK
   ```

## Scenario B: Bad Policy Bundle
If a corrupted bundle is causing all requests to fail:
1. **Rollback Bundle via CLI**
   ```powershell
   dekctl rollback --target previous
   ```
2. **Verify Policies**
   ```powershell
   dekctl test-policy --input test.json
   ```

## Escalation
If the system cannot be recovered, perform a clean uninstall and re-enroll using the `install_and_enroll.md` runbook.
