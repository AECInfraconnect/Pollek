# Runbook: Identity Rotation

## Objective

To manually rotate the SPIFFE SVID or MTLS certificates if the automated renewal process fails or if a compromise is suspected.

## Steps

1. **Force Rotation via dekctl**

   ```powershell
   dekctl rotate-identity --force
   ```

2. **Verify New Certificate**
   Check the `NotBefore` and `NotAfter` timestamps of the `client.crt` file.

   ```powershell
   openssl x509 -in C:\ProgramData\PollekDEK\certs\client.crt -text -noout | Select-String "Not After"
   ```

3. **Restart Local Enforcement Kit Core (Optional)**
   While Local Enforcement Kit handles rotation dynamically, if telemetry components hang:

   ```powershell
   Restart-Service PollekDEK
   ```

## Escalation

If rotation fails with `HTTP 401 Unauthorized`, the device may have been revoked by the admin in Pollek Cloud. Check the Pollek Cloud Dashboard.
