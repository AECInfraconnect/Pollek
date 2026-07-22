# Evidence Export Guide

This guide explains how to extract decision logs and audit events from the local control plane (or Pollek Cloud) for compliance auditors.

## 1. Exporting Decision Logs

Decision logs are the primary evidence for access control enforcement (AC-3, SOC2 CC6.1). They record `allow` and `deny` events along with the user, resource, action, and matched policy.

Read the decision logs from the local control plane (tenant `local`):

```bash
curl -X GET http://localhost:43891/v1/tenants/local/telemetry/decision-logs -o decisions.json
```

## 2. Exporting the Audit Chain

The audit chain is used to verify the integrity of the enforcement engine's state (AU-9), such as when policies were loaded, when keys were rotated, or when signatures failed validation.

You can export this using the `dek-cli`:

```bash
dek-cli export-audit --from "2026-06-01T00:00:00Z" --to "2026-06-30T23:59:59Z" -o audit_chain.json
```

This file contains the sequential hash chain proving that no events were tampered with.

To verify the chain, see `audit-chain-verification.md`.
