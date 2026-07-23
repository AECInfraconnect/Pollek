# ADR-0002: SPIRE Topology, SPIFFE Scheme, and Tenant Binding (DEK/LCP side)

## Status

Accepted (DEK/LCP side).

This ADR is the DEK/LCP ratification of the SPIRE / workload-identity topology
requested in the Pollek Cloud hand-off `HANDOFF_FOR_DEK_20260724.md` (ask #1),
and records the DEK's concrete answers to that hand-off's asks #2 (tenant_id
claim) and #3 (SPIFFE-ID presentation).

**Cross-repo drift note (honest):** the hand-off cites a canonical
`Pollek-Cloud docs/adr/0001` for the SPIRE topology, but that ADR is **not yet
on Pollek-Cloud `main`** (surveyed at `ca2e015`; the trust-spine docs and the
`HANDOFF_TO_DEK_AND_CODEX_2026-07-24.md` it references are not pushed). The
DEK therefore ratifies the topology **as described in the hand-off itself and in
Pollek-Cloud `docs/architecture/SECURE_CONTROL_CHANNEL.md`**, which is on `main`.
Formal two-sided ratification completes when Cloud pushes `docs/adr/0001`; if the
pushed ADR differs from what is ratified here, this ADR is revised, not silently
overridden.

## Context

Production DEK↔Cloud traffic must be zero-trust: identity bound per tenant / LCP
/ device / user / workload, mutual TLS on the transport, and fail-closed
behavior (Pollek-Cloud `SECURE_CONTROL_CHANNEL.md`). mTLS/SVID rollout
(`POLLEK_MTLS_MODE`) is gated on the SPIRE topology being ratified and the
bundle + ingress matrix passing. Cloud's SPIRE Server and Cosmian KMS are still
being provisioned on Railway; `/enroll` currently returns
`trust_bundle_status=pending_spire_provisioning` with a null SPIRE address.

Cloud asked the DEK to ratify the topology, confirm how the LCP token carries
tenant, and present the verified SPIFFE ID at a trusted ingress header.

## Decision

### 1. SPIFFE trust domain + SAN scheme (locked)

- Trust domain: `pollek.io`.
- Device workload: `spiffe://pollek.io/tenant/<tenant_id>/device/<device_id>`.
- Agent workload: `spiffe://pollek.io/tenant/<tenant_id>/agent/<agent_id>`.
- No `site/`/`lcp/` path segments (Cloud dropped these; the DEK parses only the
  scheme above).
- The `tenant/<id>` segment is the **authoritative tenant binding**. The DEK
  parses it with `cloud_sync_client::tenant_from_spiffe_id` and treats it as the
  proven tenant.

### 2. Tenant binding — three consistent presentations, fail-closed (ask #2)

The DEK presents its tenant to Cloud three ways that must agree:

1. `x-pollek-tenant-id` request header (always).
2. The SPIFFE SAN `tenant/<id>` segment of the presented X.509-SVID (when
   provisioned).
3. The OIDC bearer's `tenant_id` claim (when Keycloak JWT enforcement is on).

**Confirmation to Cloud:** the LCP token *can* carry a `tenant_id` claim. The
`pollek-local-control-plane` client obtains it from Keycloak via a
client-scope / protocol-mapper that maps `tenant_id` → the request tenant; the
DEK requests it through `POLLEK_OIDC_SCOPE`. When that mapper is live the
claim equals `POLLEK_TENANT_ID`.

**Agreed alternative binding (until the mapper is enabled):** the SPIFFE SAN
`tenant/<id>` segment plus the `x-pollek-tenant-id` header. Cloud enforces
`tenant/<id> == request tenant`; the DEK guarantees agreement by **failing
closed** — `SyncConfig::assert_tenant_binding` aborts the sync before any
request leaves the device if the SVID's tenant segment does not equal the
request tenant, so the DEK never asserts a tenant it cannot prove.

### 3. Present the verified SPIFFE ID via ingress header (ask #3)

The DEK sends its verified workload SPIFFE ID on **every** Cloud request via the
`x-pollek-spiffe-id` header (`cloud_sync_client::apply_headers`). The value is
read from the URI SAN of the provisioned X.509-SVID (`identity/svid.pem`) or, if
absent, `POLLEK_SPIFFE_ID`; it is never fabricated (omitted in bearer/dev mode).
Cloud's trusted ingress (or Envoy XFCC `URI=`) enforces
`tenant/<id> == request tenant` from this header.

### 4. mTLS mode stays gated

`POLLEK_MTLS_MODE` remains off on the DEK until Cloud publishes a real SPIRE
server address and trust bundle (`trust_bundle_status` leaves
`pending_spire_provisioning`) and the bundle + ingress matrix passes. The DEK's
transport already auto-selects mutual TLS the moment the SVID triple
(`svid.pem` + `svid-key.pem` + `trust-bundle.pem`) is present
(`cloud_sync_client::build_transport`); no fabricated identity is used before then.

## Consequences

- Implemented in `crates/local-control-plane/src/cloud_sync_client.rs`:
  `spiffe_id` on `SyncConfig`, `resolve_spiffe_id`, `tenant_from_spiffe_id`,
  the `x-pollek-spiffe-id` header, and `assert_tenant_binding` (fail-closed),
  with unit tests.
- Surfaced on the **Workload Identity** dashboard page as a *Tenant binding*
  card: presented SPIFFE ID, header name, SPIFFE tenant vs request tenant,
  the enforced `tenant_id` token claim, and a `consistent` / `fail closed`
  verdict — read from `GET /v1/tenants/:tenant/identity`.
- Cloud-side follow-ups tracked in `docs/DEK_TO_CLOUD_RESPONSE_2026-07-24.md`.
