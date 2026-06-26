# Runbook: Install and Enroll Local Enforcement Kit

## Objective

To successfully deploy the Pollek Local Enforcement Kit on a new Windows host and enroll it with Pollek Cloud to fetch initial certificates and configuration.

## Steps

1. **Install MSI Package**

   ```powershell
   msiexec /i PollekDEK.msi /quiet
   ```

2. **Verify Installation**

   ```powershell
   Get-Service PollekDEK
   ```

3. **Execute Enrollment**

   ```powershell
   dekctl enroll --token <enrollment-token> --cloud-url https://api.Pollek-cloud.internal
   ```

4. **Verify Configuration**

   ```powershell
   dekctl doctor
   ```

## Escalation

If `dekctl doctor` reports MTLS errors, verify that `client.crt` and `client.key` exist in `C:\ProgramData\PollekDEK\certs`. If missing, verify the Pollek Cloud URL and ensure port 43891 is open outbound.
