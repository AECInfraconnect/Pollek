# Runbook: Install and Enroll DEK

## Objective
To successfully deploy the Pollen DEK on a new Windows host and enroll it with Pollen Cloud to fetch initial certificates and configuration.

## Steps

1. **Install MSI Package**
   ```powershell
   msiexec /i PollenDEK.msi /quiet
   ```
2. **Verify Installation**
   ```powershell
   Get-Service PollenDEK
   ```
3. **Execute Enrollment**
   ```powershell
   dekctl enroll --token <enrollment-token> --cloud-url https://api.pollen-cloud.internal
   ```
4. **Verify Configuration**
   ```powershell
   dekctl doctor
   ```

## Escalation
If `dekctl doctor` reports MTLS errors, verify that `client.crt` and `client.key` exist in `C:\ProgramData\PollenDEK\certs`. If missing, verify the Pollen Cloud URL and ensure port 43891 is open outbound.
