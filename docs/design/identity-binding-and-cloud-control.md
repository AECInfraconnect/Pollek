# Agent Identity Binding for Local and Cloud Control

POLLEK has two deployment planes:

- Open source local plane: Local Control Plane plus Local Dashboard for a single PC or server.
- Commercial cloud plane: Pollek Cloud as the central control plane for many Local Control Plane devices and servers.

The local plane must work without Cloud. Cloud enrollment adds central policy, compliance reporting, hot reload, and workload trace correlation.

## Identity Model

Every registered agent should have a stable local identity. When the agent or device connects to Pollek Cloud, that identity should be bound to a SPIFFE ID. SPIFFE becomes the canonical workload trace identity because it is designed for heterogeneous workloads, SVID issuance, workload attestation, and mutual authentication.

OAuth/OIDC tokens are not the canonical agent identity. They are scoped credentials bound to a provider, audience, proof mechanism, and expiry. POLLEK should store only metadata:

- provider
- issuer
- subject
- audience
- scopes
- proof of possession method
- token hash or thumbprint
- expiry and rotation time

Never store OAuth access tokens, refresh tokens, private keys, or raw API keys in registry or telemetry.

## Practical Binding Rules

1. Local-only mode:
   - Agent can be registered with process path, user subject, and signing-key fingerprint.
   - Missing SPIFFE ID should show as "Local only" or "Not bound yet", not as a hard failure.

2. Cloud-connected mode:
   - Registered agents should carry a SPIFFE ID before fleet enforcement is considered ready.
   - OAuth/OIDC bindings should be short-lived, audience-scoped, and tied to proof of possession where possible.
   - Cloud policy can use SPIFFE ID plus token binding metadata to trace workload activity and enforce compliance.

3. Telemetry:
   - Identity telemetry should include `spiffe_id` when available.
   - Resource/tool/identity telemetry should share the same envelope across local and cloud.
   - Local Dashboard shows a single-device view; Cloud Dashboard aggregates across device, workload, agent, identity, resource, and tool.

## Research Basis

- SPIFFE defines workload identities and SVIDs for mutual authentication across dynamic environments.
- SPIRE provides server/agent architecture, node attestation, workload attestation, registration entries, and SVID issuance.
- OAuth 2.0 Token Exchange supports delegation and impersonation semantics for exchanging security tokens across services.
- OAuth mutual TLS and DPoP are practical proof-of-possession options that reduce bearer-token replay risk.
