# Pollen DEK - Desktop Enforcement Kit

Pollen DEK is an extensible, highly concurrent and robust edge computing proxy node written in Rust. It serves as a secure, local interception point for traffic and events routing between applications and the Pollen Cloud.

## AI Central Control and Observability Plane for Enterprise

Pollen DEK provides a centralized governance, telemetry, and metrics gathering solution tailored for AI and MCP (Model Context Protocol) interactions across the enterprise. 
- **Universal Observability**: Captures deep telemetry, performance metrics, and policy decisions locally and streams them directly to Pollen Cloud.
- **Enterprise Governance**: Enforces dynamic policies over LLM tool-calling and API invocations to ensure compliance and security boundaries.
- **Supply-Chain Security**: Distributed as a trusted artifact with full CycloneDX SBOMs, `cargo auditable` provenance, and cosign keyless (OIDC) signatures.

## Architecture

- **dek-core**: Supervisor service managing the device lifecycle, configuration bootstrapping via mTLS, telemetry emission, and bundle synchronization.
- **dek-policy-router**: Acts as an API proxy forwarding payloads to WebAssembly components based on dynamic policies.
- **dek-policy-runtime**: WASI-based runtime embedded via Wasmtime for executing isolated, dynamic WebAssembly modules (with strict CPU/Memory resource constraints).
- **dek-telemetry**: Streams telemetry and Prometheus metrics back to Pollen Cloud.
- **dek-metrics**: Shared metrics bootstrap crate offering global Prometheus recording and resilient pushing.
- **dek-keystore**: Cross-platform secure enclave abstraction leveraging OS-native keychain APIs (Windows DPAPI, macOS Keychain, Linux Secret Service).
- **dek-openfga** and **dek-cedar**: Integrations for fine-grained authorization with external stores (OpenFGA) and Cedar policy engine.
- **mock-cloud**: A deterministic, mock Pollen Cloud for development and integration testing.
- **Key Rotation & Audit Trails**: Implements robust, fail-safe key rotation with `TrustedKeySet` and cryptographic audit logs using SHA-256 hash chains for SIEM integration.

## PEP Enforcement Model

Pollen DEK shifts from a purely cooperative policy enforcement to a machine-level enforcement architecture. The model is structured into two layers, with strict "enforcement ceilings" defined for each Operating System to balance security and system stability.

### Layer 1: Application-Layer MCP (All OS - Core)
- **Mechanism:** Deep policy inspection on JSON-RPC and MCP payloads using `dek-mcp-proxy` and `dek-mcp-stdio-wrapper`.
- **Scope:** Available universally across Linux, Windows, and macOS.
- **Role:** Remains the primary and deepest point of policy enforcement for all connected applications.

### Layer 2: Network Egress Guardrails (OS-Specific)
- **Linux (via eBPF / WS-D):** 
  - Implements coarse L3/L4 network egress guardrails.
  - Enforces traffic policies at the kernel level, catching and blocking unauthorized traffic even if a rogue or misconfigured application attempts to bypass the Layer 1 proxy.
- **Windows & macOS (Phase 1):** 
  - **No transparent kernel interception** is performed (e.g., no WFP callouts on Windows or Network Extensions on Mac). This deliberate architectural decision eliminates the risk of system-wide BSODs or kernel panics.
  - **Opt-in Redirect Options:** Enforcement at this layer is limited to opt-in traffic redirection, such as configuring system-wide proxy settings or injecting per-application proxy environment variables (`HTTP_PROXY`, `HTTPS_PROXY`).

> [!WARNING]
> **Enforcement Ceiling:** It is critical to understand that Windows and macOS currently have a lower enforcement ceiling than Linux. While Linux guarantees network-level egress enforcement via eBPF regardless of application behavior, Windows and macOS rely strictly on Layer 1 App-layer MCP proxies and cooperative opt-in network redirects.

## License

This project is licensed under the Apache License 2.0. See the [LICENSE](LICENSE) file for more information.
