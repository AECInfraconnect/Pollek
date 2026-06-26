# Pollek Local Enforcement Kit Incident Response Runbook

## Identifying an Incident

1. **Cloud Telemetry Alerts**: Spikes in `decision_log` Deny verdicts with `reason: unknown_error` or `reason: policy_evaluation_error`.
2. **Heartbeat Failures**: Devices stop sending heartbeats for > 5 minutes.
3. **Queue Build-up**: Telemetry queue depth > 5000 indicates the Cloud sink is unreachable or rate-limiting.

## Debugging Workflow

### 1. Check Local Logs

Access the host and tail the logs:

- **Windows**: `Get-EventLog -LogName Application -Source PollekDEK` or check `C:\ProgramData\PollekDEK\logs\`
- **Linux**: `journalctl -u Pollek-dek-core -f`

### 2. Verify mTLS Connection

Ensure the device's mTLS certificate is valid and not expired.
Run `dek-cli status` to verify certificate expiration and Spire agent health.

### 3. Check Active Bundle

Inspect the `active_bundle.json` at `/etc/Pollek-Local Enforcement Kit/` (Linux) or `C:\ProgramData\PollekDEK\` (Windows).
Validate the bundle schema using:
`dek-cli validate-bundle --path /etc/Pollek-Local Enforcement Kit/active_bundle.json`

## Escalation

If an active threat actor is detected bypassing Local Enforcement Kit controls:

1. Issue an **Emergency Deny** via the Cloud Management API:
   `POST /v1/tenants/{tenant_id}/devices/{device_id}/emergency_deny`
2. This pushes an immediate zero-trust block-all bundle down to the device over the active WebSocket/gRPC stream.
3. Review `mcp_decision` logs for anomalous tool usage to identify the compromised agent.
