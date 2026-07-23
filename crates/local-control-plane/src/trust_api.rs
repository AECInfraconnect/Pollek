//! Trust & Provenance surface (roadmap Phase A1, aligned to Cloud
//! `bundle-manifest.v2`) — the LCP face of the single **Trust Policy Gate**
//! (`dek-trust-gate`).
//!
//! Every bundle activation routes through one choke point that verifies a policy
//! bundle manifest exactly as Pollek Cloud signs it (Ed25519 base64url over the
//! canonical unsigned manifest) and enforces tenant match, generation
//! monotonicity, revocation status, artifact integrity, and — when present —
//! provenance/SBOM/attestation. This module runs that gate for real and records
//! the verdict so the dashboard shows, per bundle, which checks passed.
//!
//! Endpoints:
//!   * `POST /v1/tenants/:tenant/trust/verify` — submit a `bundle-manifest.v2`
//!     (+ optional artifact bytes by name); the gate runs, the verdict is
//!     persisted, a tamper-evident audit entry is appended, and on `accept` the
//!     activated revision advances (so a later downgrade is rejected).
//!   * `GET  /v1/tenants/:tenant/trust` — the effective policy, trusted-signer
//!     status, and the latest verdict per bundle.
//!
//! Trust anchor: the DEK's pinned bundle-signing keys — the local control-plane
//! signer (`state.signer`, for Local-mode bundles) plus any Cloud signer public
//! keys pinned at `$DEK_LCP_DATA/trust/cloud-signers.json`. No key is fabricated;
//! with no trusted signer the gate fails closed.

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use dek_secure_spool::audit::AuditEntry;
use dek_trust_gate::{verify, TrustPolicy, TrustedSigner, Verdict, VerifyInput};
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

/// Serializable pin of a Cloud bundle-signing key (SPKI PEM), stored in
/// `$DEK_LCP_DATA/trust/cloud-signers.json`.
#[derive(Debug, Deserialize)]
struct CloudSignerPin {
    key_id: String,
    public_key_pem: String,
}

/// The DEK's trusted bundle-signing keys = single source of truth:
/// the local control-plane signer (Local mode) + any pinned Cloud signers.
fn trusted_signers(state: &AppState, dir: &std::path::Path) -> Vec<TrustedSigner> {
    let mut signers = Vec::new();
    if let Some(s) =
        TrustedSigner::from_base64(state.signer.key_id.clone(), &state.signer.public_key_b64())
    {
        signers.push(s);
    }
    if let Ok(bytes) = std::fs::read(dir.join("cloud-signers.json")) {
        if let Ok(pins) = serde_json::from_slice::<Vec<CloudSignerPin>>(&bytes) {
            for p in pins {
                if let Some(s) = TrustedSigner::from_pem(p.key_id, &p.public_key_pem) {
                    signers.push(s);
                }
            }
        }
    }
    signers
}

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
    /// The Cloud `bundle-manifest.v2` (raw), including its `signatures[]`.
    manifest: serde_json::Value,
    /// Optional artifact bytes, base64 by `artifact.name`.
    #[serde(default)]
    artifacts: HashMap<String, String>,
}

async fn verify_bundle(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Json(req): Json<VerifyRequest>,
) -> impl IntoResponse {
    let dir = trust_dir();
    let signers = trusted_signers(&state, &dir);
    // The URL tenant is authoritative.
    let mut policy = load_policy(&dir);
    policy.expected_tenant = Some(tenant.clone());

    let mut artifact_bytes: HashMap<String, Vec<u8>> = HashMap::new();
    for (name, b64) in &req.artifacts {
        if let Ok(bytes) = base64::prelude::BASE64_STANDARD.decode(b64) {
            artifact_bytes.insert(name.clone(), bytes);
        }
    }

    let activated = load_map(&dir.join("activated.json"));
    let bundle_id = req
        .manifest
        .get("bundle_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let last_rev = activated.get(&bundle_id).and_then(|v| v.as_str());

    let verdict = verify(VerifyInput {
        manifest: &req.manifest,
        policy: &policy,
        trusted_signers: &signers,
        now_unix: now_unix(),
        last_activated_revision: last_rev,
        artifact_bytes: &artifact_bytes,
    });

    let verdicts_path = dir.join("verdicts.json");
    let mut verdicts = load_map(&verdicts_path);
    if let Ok(v) = serde_json::to_value(&verdict) {
        verdicts.insert(bundle_id.clone(), v);
        let _ = write_json(&verdicts_path, &verdicts);
    }

    if verdict.accepted() {
        let mut activated = activated;
        activated.insert(
            bundle_id,
            serde_json::Value::String(verdict.revision.clone()),
        );
        let _ = write_json(&dir.join("activated.json"), &activated);
    }

    append_audit(&dir, &verdict);

    let code = if verdict.accepted() {
        StatusCode::OK
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    (code, Json(json!({ "tenant": tenant, "verdict": verdict })))
}

async fn get_trust(State(state): State<AppState>, Path(tenant): Path<String>) -> impl IntoResponse {
    let dir = trust_dir();
    let policy = load_policy(&dir);
    let signers = trusted_signers(&state, &dir);

    let verdicts = load_map(&dir.join("verdicts.json"));
    let mut list: Vec<serde_json::Value> = verdicts.into_values().collect();
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
            "schema_version": "trust-provenance.v2",
            "tenant": tenant,
            "manifest_contract": "bundle-manifest.v2",
            "policy": policy,
            "keys": {
                "provisioned": !signers.is_empty(),
                "usable_now": signers.len(),
            },
            "verdicts": list,
        })),
    )
}
