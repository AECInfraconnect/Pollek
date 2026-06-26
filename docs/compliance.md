# Pollek DEK Compliance Mappings

Pollek DEK helps organizations satisfy emerging AI governance frameworks by enforcing strict agent boundaries and maintaining tamper-evident audit trails.

## 1. EU AI Act

- **Article 14 (Human Oversight):** DEK enforces 'human-in-the-loop' requirements by requiring approval for high-risk MCP tool calls.
- **Article 15 (Accuracy, Robustness, Cybersecurity):** DEK uses eBPF and MAC to ensure agents cannot perform arbitrary out-of-bounds execution.
- **Article 12 (Record-Keeping):** DEK’s telemetry provides immutable, signed logs of every action an AI agent takes on the system.

## 2. NIST AI RMF

- **MAP 1.5 (Risk Tolerances):** The DEK Agent Risk Score helps quantify over-permissions and shadow AI.
- **MEASURE 2.1 (Traceability):** DEK logs each tool access with the exact prompt, user intent, and resulting decision.
- **MANAGE 3.1 (Risk Treatment):** Policy presets provide guardrails to contain agent actions.

## 3. ISO/IEC 42001 (AI Management System)

- **Annex A.6.3 (Audit trails):** All access to local resources via MCP is recorded and can be exported using the `dekctl export-compliance` command.
- **Annex A.7 (Data for AI systems):** PII redactors prevent the exfiltration of sensitive organizational data into untrusted models.

## Exporting the Evidence Pack

To generate a zip file of compliance evidence suitable for auditors:

```bash
pollek-dek export-compliance
```
