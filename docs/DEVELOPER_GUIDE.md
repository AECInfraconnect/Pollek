# Pollek Local Enforcement Kit Developer Guide

This guide provides concrete code examples, architectural contracts, and operational guidelines for working with Pollek Local Enforcement Kit `v1.0.0-beta`.

---

## 1. Local Control Plane

For local development, run the local control plane. It listens on:

- `43892`: HTTPS Enrollment
- `43891`: Strict mTLS APIs (Telemetry, Bundles, SPIFFE)

**Code Example: Starting the local control plane:**

```bash
cargo run -p local-control-plane
```

The kit talks to the local control plane by default; syncing to Pollek Cloud
is configured on the local service via `DEK_CLOUD_URL` (see `cloud_sync.rs`).

---

## 2. Telemetry Batch Flush Architecture

Local Enforcement Kit Core locally spools telemetry to a SQLite database. A background flusher pulls up to 50 events at a time and POSTs them to Pollek Cloud.

**Code Example: Emitting Telemetry (Local Enforcement Kit side)**

```rust
let sink = telemetry_sink.clone();
sink.emit_async(
    serde_json::json!({
        "event_type": "Pollek.Local Enforcement Kit.dns_observe",
        "cgroup_id": obs.cgroup_id,
        "qname": obs.qname,
        "answers": obs.answers,
        "is_response": obs.is_response,
    }),
    dek_telemetry::Priority::Low,
);
```

**Network Contract:**

```http
POST /v1/tenants/:tenant_id/telemetry/events
Content-Type: application/json

{
  "events": [
    { "event_type": "Pollek.Local Enforcement Kit.dns_observe", ... },
    ...
  ]
}
```

---

## 3. Canonical Bundle Signing (JCS)

All policy and configuration artifacts from the Pollek Cloud must be signed using Ed25519. The signature validates the JCS (JSON Canonicalization Scheme) output of the target payload.

**Code Example: Mock-Cloud Signing (Cloud side)**

```rust
use sha2::{Sha256, Digest};
use ed25519_dalek::Signer;

// 1. Serialize the payload using JCS
let signed_bytes = serde_jcs::to_vec(&payload["signed"]).unwrap();

// 2. Sign the canonical bytes
let signature = signing_key.sign(&signed_bytes);
```

**Code Example: Local Enforcement Kit Verifying (Local Enforcement Kit side)**

```rust
// 1. Serialize incoming payload to JCS
let signed_bytes = serde_jcs::to_vec(&metadata["signed"])
    .context("serialize signed payload using JCS")?;

// 2. Verify against pinned key
let key_set = self.key_set.load();
match key_set.verify(now, &signed_bytes, &sigs) {
    VerifyOutcome::Valid { .. } => { /* OK */ },
    outcome => return Err(anyhow!("Invalid signature")),
}
```

---

## 4. Release Checklist & CI Workflow

Pollek Local Enforcement Kit CI automatically generates native installers (`.deb`, `.msi`, `.pkg`) on every tag.
Before tagging a release:

- `[ ]` Ensure `cargo test --workspace` passes cleanly.
- `[ ]` Verify mock-hash bypass is disabled in release builds (`cfg!(debug_assertions)`).

---

## 5. Acceptance Test Skeleton

To run the local end-to-end acceptance test:

```bash
cargo test -p acceptance-tests --test local_e2e -- --ignored --nocapture
```

---

## 6. AI Agent Work Orders

When assigning work to an AI Agent for Local Enforcement Kit, format requests as follows:

```markdown
<WORK_ORDER>
Goal: Add new Telemetry Event Type for File Access
Target: crates/dek-ebpfd and crates/dek-telemetry
Context: Need to capture `openat` calls from specific PIDs and send them to Mock-Cloud.
Constraints: Must use the existing `CloudTelemetrySink` and follow batching contract.
</WORK_ORDER>
```
