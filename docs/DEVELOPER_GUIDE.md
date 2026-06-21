# Pollen DEK Developer Guide

This guide provides concrete code examples, architectural contracts, and operational guidelines for working with Pollen DEK `v1.0.0-beta`.

---

## 1. Mock-Cloud Strict mTLS Sandbox
Mock-Cloud operates as the primary reference simulator. It listens on:
- `43892`: HTTPS Enrollment
- `43891`: Strict mTLS APIs (Telemetry, Bundles, SPIFFE)

**Code Example: Starting Mock-Cloud in strict vs insecure mode:**
```bash
# Default (Strict mTLS on port 43891)
cargo run -p mock-cloud

# Insecure mode (Allow missing client certs on port 43891)
cargo run -p mock-cloud -- --dev-insecure-allow-no-client-cert
```

---

## 2. Telemetry Batch Flush Architecture
DEK Core locally spools telemetry to a SQLite database. A background flusher pulls up to 50 events at a time and POSTs them to the Mock-Cloud.

**Code Example: Emitting Telemetry (DEK side)**
```rust
let sink = telemetry_sink.clone();
sink.emit_async(
    serde_json::json!({
        "event_type": "pollen.dek.dns_observe",
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
    { "event_type": "pollen.dek.dns_observe", ... },
    ...
  ]
}
```

---

## 3. Canonical Bundle Signing (JCS)
All policy and configuration artifacts from the Pollen Cloud must be signed using Ed25519. The signature validates the JCS (JSON Canonicalization Scheme) output of the target payload.

**Code Example: Mock-Cloud Signing (Cloud side)**
```rust
use sha2::{Sha256, Digest};
use ed25519_dalek::Signer;

// 1. Serialize the payload using JCS
let signed_bytes = serde_jcs::to_vec(&payload["signed"]).unwrap();

// 2. Sign the canonical bytes
let signature = signing_key.sign(&signed_bytes);
```

**Code Example: DEK Verifying (DEK side)**
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
Pollen DEK CI automatically generates native installers (`.deb`, `.msi`, `.pkg`) on every tag.
Before tagging a release:
- `[ ]` Ensure `cargo test --workspace` passes cleanly.
- `[ ]` Verify mock-hash bypass is disabled in release builds (`cfg!(debug_assertions)`).
- `[ ]` Ensure `Mock-Cloud` passes standard soak testing.

---

## 5. Acceptance Test Skeleton
To run a full e2e acceptance test locally:
```bash
# 1. Start Mock Cloud
cargo run -p mock-cloud &
MOCK_PID=$!

# 2. Start DEK Core
sudo -E cargo run -p dek-core

# 3. Terminate
kill $MOCK_PID
```

---

## 6. AI Agent Work Orders
When assigning work to an AI Agent for DEK, format requests as follows:
```markdown
<WORK_ORDER>
Goal: Add new Telemetry Event Type for File Access
Target: crates/dek-ebpfd and crates/dek-telemetry
Context: Need to capture `openat` calls from specific PIDs and send them to Mock-Cloud.
Constraints: Must use the existing `CloudTelemetrySink` and follow batching contract.
</WORK_ORDER>
```
