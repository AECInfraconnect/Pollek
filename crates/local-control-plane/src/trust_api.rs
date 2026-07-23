//! Trust & Provenance surface (roadmap Phase A1) — the LCP face of the single
//! **Trust Policy Gate** (`dek-trust-gate`).
//!
//! Every bundle activation should route through one choke point that proves,
//! per the SRS, that the artifact is trustworthy *by evidence, not by where it
//! came from*: signature + signer-allowlist + revocation + tenant match +
//! generation monotonicity + artifact integrity + provenance + SBOM +
//! test-attestation. This module runs that gate for real and records the verdict
//! so the dashboard can show — per bundle — exactly which checks passed.
//!
//! Endpoints:
//!   * `POST /v1/tenants/:tenant/trust/verify` — submit a signed bundle envelope
//!     (+ optional artifact bytes); the gate runs, the verdict is persisted, a
//!     tamper-evident audit entry is appended, and on `accept` the activated
//!     revision advances (so a later downgrade is rejected).
//!   * `GET  /v1/tenants/:tenant/trust` — the effective policy, key-provisioning
//!     status, and the latest verdict per bundle.
//!
//! Trust material lives under `$DEK_LCP_DATA/trust/`:
//!   `trusted-keys.json` (dek-bundle-sync TrustedKeySet), `trust-policy.json`
//!   (dek-trust-gate TrustPolicy; default if absent), `verdicts.json`,
//!   `activated.json`, `audit.log` (hash chain).

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use dek_bundle_sync::keys::TrustedKeySet;
use dek_secure_spool::audit::AuditEntry;
use dek_trust_gate::{verify, SignedBundleEnvelope, TrustPolicy, Verdict, VerifyInput};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tenants/:tenant/trust", get(get_trust))
        .route("/v1/tenants/:tenant/trust/verify", post(verify_bundle))
}

fn trust_dir() -> PathBuf {
    let base = std::env::var("DEK_LCP_DATA").unwrap_or_else(|_| "./pollek-local-data".into());
    PathBuf::from(base).join("trust")
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Load the persisted trusted-key set (empty set if none — the gate then fails
/// closed with `NoUsableKeys`, which is the correct posture, not a crash).
fn load_keys(dir: &std::path::Path) -> TrustedKeySet {
    std::fs::read(dir.join("trusted-keys.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<TrustedKeySet>(&b).ok())
        .unwrap_or_default()
}

/// Load the local trust policy, defaulting to the fail-closed baseline
/// (signature + generation-monotonicity required).
fn load_policy(dir: &std::path::Path) -> TrustPolicy {
    std::fs::read(dir.join("trust-policy.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<TrustPolicy>(&b).ok())
        .unwrap_or_default()
}

fn load_map(path: &std::path::Path) -> HashMap<String, serde_json::Value> {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_json<T: serde::Serialize>(path: &std::path::Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&tmp, path)
}

/// Append one verdict to the tamper-evident audit chain (hash-linked JSON lines).
fn append_audit(dir: &std::path::Path, verdict: &Verdict) {
    let path = dir.join("audit.log");
    let (seq, prev_hash) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| {
            s.lines()
                .last()
                .and_then(|l| serde_json::from_str::<AuditEntry>(l).ok())
        })
        .map(|e| (e.seq + 1, e.entry_hash))
        .unwrap_or((0, "GENESIS".to_string()));
    let ts = chrono::Utc::now().to_rfc3339();
    let entry = AuditEntry::new(seq, ts, verdict.audit_payload(), &prev_hash);
    if let Ok(line) = serde_json::to_string(&entry) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

#[derive(Debug, Deserialize)]
struct VerifyRequest {
    envelope: SignedBundleEnvelope,
    /// Optional artifact bytes, base64 by `BundleArtifact.path`. When present, the
    /// gate verifies each declared artifact against its authenticated sha256.
    #[serde(default)]
    artifacts: HashMap<String, String>,
}

async fn verify_bundle(
    State(_state): State<AppState>,
    Path(tenant): Path<String>,
    Json(req): Json<VerifyRequest>,
) -> impl IntoResponse {
    let dir = trust_dir();
    let keys = load_keys(&dir);
    // The URL tenant is authoritative: pin the gate's expected tenant to it so a
    // bundle minted for another tenant cannot be activated here.
    let mut policy = load_policy(&dir);
    policy.expected_tenant = Some(tenant.clone());

    // Decode any provided artifact bytes (skip malformed entries — integrity will
    // then flag the missing artifact rather than trusting it).
    let mut artifact_bytes: HashMap<String, Vec<u8>> = HashMap::new();
    for (p, b64) in &req.artifacts {
        if let Ok(bytes) = base64::prelude::BASE64_STANDARD.decode(b64) {
            artifact_bytes.insert(p.clone(), bytes);
        }
    }

    let activated = load_map(&dir.join("activated.json"));
    let bundle_id = req.envelope.signed.bundle.metadata.bundle_id.clone();
    let last_rev = activated.get(&bundle_id).and_then(|v| v.as_str());

    let verdict = verify(VerifyInput {
        envelope: &req.envelope,
        policy: &policy,
        trusted_keys: &keys,
        now_unix: now_unix(),
        last_activated_revision: last_rev,
        artifact_bytes: &artifact_bytes,
    });

    // Persist the latest verdict per bundle.
    let verdicts_path = dir.join("verdicts.json");
    let mut verdicts = load_map(&verdicts_path);
    if let Ok(v) = serde_json::to_value(&verdict) {
        verdicts.insert(bundle_id.clone(), v);
        let _ = write_json(&verdicts_path, &verdicts);
    }

    // On accept, advance the activated revision (keeps the downgrade guard honest).
    if verdict.accepted() {
        let mut activated = activated;
        activated.insert(
            bundle_id,
            serde_json::Value::String(verdict.bundle_revision.clone()),
        );
        let _ = write_json(&dir.join("activated.json"), &activated);
    }

    append_audit(&dir, &verdict);

    let code = if verdict.accepted() {
        StatusCode::OK
    } else {
        // 422: the request was well-formed but failed the trust gate (quarantined).
        StatusCode::UNPROCESSABLE_ENTITY
    };
    (code, Json(json!({ "tenant": tenant, "verdict": verdict })))
}

async fn get_trust(
    State(_state): State<AppState>,
    Path(tenant): Path<String>,
) -> impl IntoResponse {
    let dir = trust_dir();
    let policy = load_policy(&dir);
    let keys = load_keys(&dir);
    let now = now_unix();
    let usable_keys = keys.usable_keys(now).count();

    let verdicts = load_map(&dir.join("verdicts.json"));
    let mut list: Vec<serde_json::Value> = verdicts.into_values().collect();
    // Newest evaluation first.
    list.sort_by(|a, b| {
        let ta = a
            .get("evaluated_at_unix")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let tb = b
            .get("evaluated_at_unix")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        tb.cmp(&ta)
    });

    (
        StatusCode::OK,
        Json(json!({
            "schema_version": "trust-provenance.v1",
            "tenant": tenant,
            "policy": policy,
            "keys": {
                "provisioned": !keys.keys.is_empty(),
                "usable_now": usable_keys,
            },
            "verdicts": list,
        })),
    )
}
