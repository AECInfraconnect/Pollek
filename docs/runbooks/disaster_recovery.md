# Pollek Local Enforcement Kit Disaster Recovery Runbook

## Overview

This runbook provides the procedure for recovering a Pollek Local Enforcement Kit node in the event of severe failure, cryptographic compromise, or persistent crash loop.

## Scenario 1: Cryptographic Compromise (Spire or Core Keys Leaked)

1. In the Pollek Cloud Management console, locate the affected Device ID.
2. Click **Revoke Device Identity**. This will blacklist the device's mTLS certificate and SVID.
3. On the compromised endpoint, stop the Pollek Local Enforcement Kit Core service:
   - **Linux**: `systemctl stop Pollek-dek-core`
   - **Windows**: `Stop-Service PollekDEKCore`
4. Wipe the local Keystore and Data Directory:
   - Delete all contents of `/etc/Pollek-Local Enforcement Kit/` or `C:\ProgramData\PollekDEK\`.
5. Generate a new enrollment token from the Cloud Management Console.
6. Run `dek-enroll` with the new token to establish a fresh cryptographic identity.

## Scenario 2: Persistent Crash Loop (Poison Pill Bundle)

If a bad policy bundle causes `dek-core` or `dek-mcp-proxy` to panic repeatedly:

1. Local Enforcement Kit includes a fallback to the `shadow_bundle.json` if `active_bundle.json` fails probation.
2. If probation failed to catch it, manually trigger a rollback:
   `dek-cli rollback --device <device_id>` (This sends an emergency override via Cloud).
3. If the device cannot reach the Cloud:
   - On the local device, execute: `dek-cli local-rollback` (Requires LocalAdmin role or physical access).
   - This copies `lkg_bundle.json` over `active_bundle.json` and restarts the service.

## Scenario 3: Loss of Connectivity to Control Plane

Local Enforcement Kit is designed to operate seamlessly without the control plane (offline-first).

- Policies will continue evaluating using the cached `active_bundle.json`.
- Telemetry will spool to the local SQLite database (`telemetry.db`).
- **Action**: Ensure telemetry database size doesn't exhaust disk space. By default, it truncates old events when exceeding 1GB.
