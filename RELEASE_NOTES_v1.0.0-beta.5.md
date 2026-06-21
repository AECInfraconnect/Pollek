# Pollen DEK v1.0.0-beta.5 Release Notes

We are excited to announce Pollen DEK `v1.0.0-beta.5`. This release focuses on addressing edge deployment gaps and bringing critical control plane capabilities to the local environment.

## Highlights

- **Dry-run Simulation Engine:** Easily simulate and test draft policies with multiple what-if scenarios directly from the Local Admin Dashboard without affecting live traffic.
- **Audit Logging Export:** Export decision logs directly from the dashboard in CSV and JSON formats for external reporting and SIEM integrations.
- **Connector Health Checks:** Test connectivity to external PDP backends (OPA, OpenFGA, Cedar) via the Local Admin Dashboard settings.
- **Failover Enhancements:** Control PDP pool selections with `ManualOverride` capabilities and fine-grained `auto_recovery_delay` settings on circuit breakers.
- **Internationalization Readiness:** Native support for English and Thai language localizations across the dashboard UI.
- **Contract Schema Fixes:** Re-aligned `DecisionResult` and `PollenError` with `adapter_results`, `obligations` properties. `latency_ms` has been safely cast to `i64`.
- **MSRV Enforced:** Minimum Supported Rust Version (MSRV) raised to `1.85`.

## Upgrading

To upgrade your DEK CLI to the latest version via the beta channel:

```bash
dek-cli update --channel beta
```
