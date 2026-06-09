# Pollen DEK - First Run Quickstart

Welcome to Pollen DEK! This guide will take you from cloning the repository to successfully evaluating your first policy decision via the DEK sidecar.

## 1. Build the Workspaces

Pollen DEK uses a robust Rust-based workspace. Compile the components:

```bash
cargo build --release --workspace
```

## 2. Boot the Mock Cloud Control Plane

The Mock Cloud provides the necessary CA, keys, MTLS provisioning, and policy bundles required for the DEK agent to function.

In a new terminal window:
```bash
# Generate the necessary development certificates
cargo run --bin cert-gen

# Start the mock control plane
cargo run --bin mock-cloud
```
The Mock Cloud should now be running on `https://127.0.0.1:43892`.

## 3. Enroll the Device

Before starting the DEK sidecar, you must perform an enrollment to provision your device's initial mTLS SVID and configuration.

```bash
cargo run --bin dek-cli -- enroll --cloud-url https://127.0.0.1:43892
```
This generates the certificates and initial config in `$HOME/.pollen_dek/`.

## 4. Start the DEK Core Agent

Now start the Core supervisor and sidecar:

```bash
cargo run --bin dek-core
```
You will see logs indicating that `dek-core` has started its sidecar API (default `127.0.0.1:43890`) and synced its first policy bundle from the mock cloud.

## 5. Evaluate Your First Policy

Now that the system is running, simulate an application making an authorization request via the sidecar API.

In another terminal, run:
```bash
curl -X POST http://127.0.0.1:43890/v1/decision/check \
  -H "Content-Type: application/json" \
  -d '{
    "request_id": "req-1",
    "tenant_id": "tenant-1",
    "device_id": "dev-1",
    "principal": {"id": "user_1", "roles": ["admin"]},
    "action": "read",
    "resource": {"kind": "document", "id": "doc_1"},
    "context": {}
  }'
```

You should receive a `DecisionResponse` with `allow: true` (or `false` depending on the active mock bundle policy).

## 6. Simulate Fail-Closed (Chaos)

You can verify the fail-closed network resilience mechanics by inducing a cloud outage:

```bash
curl -X POST https://127.0.0.1:43892/mock/admin/chaos/outage \
  -H "Content-Type: application/json" -k \
  -d '{"enabled": true}'
```

Watch the `dek-core` logs transition to `fail-closed` if the TUF-Lite sync fails repeatedly. Requests to `/v1/decision/check` will safely return a `StrictDeny`. To restore:

```bash
curl -X POST https://127.0.0.1:43892/mock/admin/chaos/outage \
  -H "Content-Type: application/json" -k \
  -d '{"enabled": false}'
```

Welcome to secure, resilient edge-first authorization!
