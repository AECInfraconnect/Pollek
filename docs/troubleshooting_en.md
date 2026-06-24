# Pollek Local Enforcement Kit Troubleshooting Guide

## Common Issues

### 1. Mock-Cloud Fails to Start

**Symptom**: `cargo run -p mock-cloud` fails with "Address already in use".
**Fix**: Ensure ports 43891 and 43892 are not being used by another process. If running on a shared development machine, check for lingering mock-cloud processes.

### 2. Enrollment Fails

**Symptom**: `Pollek-dekctl enroll` hangs or returns a connection error.
**Fix**: Verify that Mock-Cloud is running and the `--cloud-url` points exactly to the HTTPS port of the Mock-Cloud (e.g., `https://127.0.0.1:43892`).

### 3. Local Enforcement Kit Core Does Not Sync Bundle

**Symptom**: Logs show `bundle_sync_failed` and Local Enforcement Kit evaluates to fallback mode.
**Fix**: Check if the device has been enrolled successfully and `bootstrap.json` exists in `~/.Pollek/Local Enforcement Kit/`. If testing poisoning scenarios, ensure you haven't intentionally triggered a chaos outage in Mock-Cloud.

### 4. Telemetry is Not Visible in Dashboard

**Symptom**: You trigger MCP actions, but the `/admin/dashboard` does not show new events.
**Fix**: Telemetry is buffered and flushed periodically (default 5s). Wait for the flush interval, or manually trigger a flush. Ensure Local Enforcement Kit has a valid network connection to the Mock-Cloud.

### 5. eBPF Guardrail Not Working (Linux Only)

**Symptom**: Network egress is not blocked despite policy.
**Fix**: Verify Local Enforcement Kit is running with root privileges (`CAP_BPF` and `CAP_NET_ADMIN`). Check `dmesg` or `journalctl -u Pollek-Local Enforcement Kit` for BPF verifier errors.

## Preflight Doctor Checks

If \pollen-dek doctor\ fails, review the check that failed. Common issues:

- **WinDivert missing**: Re-run the installer as Administrator.
- **Port 43889 in use**: Check \
etstat -ano | findstr 43889\ and kill the conflicting process.
- **eBPF verifier error**: Your Linux kernel might be too old. Minimum required is 5.15.
