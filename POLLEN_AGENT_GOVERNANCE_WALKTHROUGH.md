# Pollen Agent Governance Lifecycle Walkthrough

This document demonstrates how an AI agent is discovered, validated, bound to policies, and enforced in the Pollen DEK ecosystem.

## 1. Discovery and Fingerprinting

When an agent starts (e.g., `claude-desktop`), `dek-agent-observer` scans for running processes or configuration files. It uses `dek-fingerprint-defs` to match the agent against an offline baseline or dynamically downloaded fingerprint list.

- The fingerprint provides the binary hashes, configuration locations, and a definition of what the agent *should* be capable of.
- The `FingerprintService` ensures we never use definitions that are older than our current engine allows, preventing rollback attacks on governance schemas.

## 2. Binding Generation

Upon match, `AgentBinding::from_discovery` is invoked. The static definition is expanded into a dynamic **Binding**:

- **Capabilities**: Translates `mcp_stdio_wrapper` into `InteractionSurface::McpStdio`.
- **Control**: Maps how the wrapper handles Stdio or how HTTP proxies intercept egress.
- **Enforcement**: Dictates if the agent's specific tools (e.g., file system writes) should trigger a `RequireApproval` guard.
- **Telemetry**: Instructs `dek-agent-observer` to redact sensitive fields (like API keys) from the telemetry data stream before it leaves the machine.

## 3. Enforcement Integration

- **dek-agent-connector**: Hooks into the configuration to point the agent's executable path to our `dek-mcp-stdio-wrapper`.
- **dek-policy-router**: Looks up the Agent Binding before letting any outbound OpenAI/Anthropic network calls pass. If the policy says `NetworkEgressInterception` and it's not authorized, the Circuit Breaker opens.

## 4. Drift Detection & Lifecycle

At runtime, if an agent uses a tool not present in its declared capabilities:

- The `reevaluate()` method catches the mismatch.
- A `capability_drift:new_tool` telemetry event is generated.
- The binding transitions from `Provisioned` to `Suspended` or `Enforced` depending on the configured strictness level.
- This feeds back into the `dek-policy-syncer` which ships the state up to the Cloud for audit.

## API Integration

The Control Plane provides REST endpoints to inspect this lifecycle:

- `GET /v1/agents/fingerprints`
- `GET /v1/agents/bindings`
