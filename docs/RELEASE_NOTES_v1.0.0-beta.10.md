# Pollek Local Enforcement Kit v1.0.0-beta.10 Release Notes

We are excited to announce Pollek Local Enforcement Kit `v1.0.0-beta.10` — the largest feature release since public beta. This release introduces the full **AI Agent Observability stack**, **Shadow AI Detection**, and the **Observe → Suggest → Enforce Governance Loop**.

## Highlights

### AI Agent Observability (New)

- **Shadow AI Auto-Discovery**: Background OS process scanning with heuristic fingerprinting detects unmanaged AI agents — Ollama, vLLM, Claude Desktop, GitHub Copilot, Cursor, and more — running on the local machine.
- **Token & Cost Ledger**: Tracks estimated token costs across all observed AI APIs via a configurable price catalog, with per-agent and per-model breakdowns.
- **Policy Suggestion Engine**: Automatically generates Rego and Cedar policies based on observed cost thresholds ($25/day default), Shadow AI detections, and agent behavior anomalies.
- **Agent Trust Scoring**: Real-time trust scores via `AgentBaseline` analysis, enabling dynamic `KillSwitch` or `RequireApproval` obligations when anomalous behavior is detected.

### Security Hardening

- **Content Guard**: The MCP proxy now inspects payloads for prompt injection patterns, PII leakage, and malicious content _before_ policy evaluation triggers.
- **Rate Limiting**: Token-bucket rate limiters per agent protect downstream endpoints from overuse and abuse.
- **Tamper-Evident Audit**: Decisions are securely queued locally with a SHA-256 hash chain (`AuditEntry`), cryptographically proving audit log integrity.

### Platform Previews

- **A2A Mediator** (`dek-a2a-mediator`): Inter-Agent Trust Protocol (IATP) mediator for Google A2A protocol communication between trusted agents.
- **Execution Sandbox** (`dek-execution-sandbox`): Isolated, short-lived tool execution environments for untrusted agent code.
- **Policy Presets** (`dek-policy-presets`): Pre-built Rego/Cedar/OpenFGA policy templates for zero-config quickstart — deploy common guardrails in one click.

### Dashboard (7 New Pages)

- **Auto Discovery** — trigger and view process/config scans
- **Shadow AI Inbox** — alerts for unrecognized AI activity
- **Policy Suggestions** — review and apply auto-generated policies
- **Cost Ledger** — monitor AI spend across agents and models
- **Policy Presets** — browse and deploy pre-built policy templates
- **Blackbox AI Providers** — manage registered external AI providers
- **Alerts** — system-wide security and compliance notifications

### CI/CD Improvements

- Fixed gitleaks OS incompatibility (Linux binary on macOS runner)
- Fixed `sha256sum` failing on directories in release asset checksums
- Migrated `rcgen` certificate generation API to v0.13
- Fixed Sigstore cosign signing for nested artifact directories

## Upgrading

To upgrade your Local Enforcement Kit CLI to the latest version via the beta channel:

```bash
dek-cli update --channel beta
```

## Full Changelog

See [CHANGELOG.md](CHANGELOG.md) for the complete history of changes from beta.7 through beta.10.
