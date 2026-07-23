//! Workload identity surface (roadmap 2D) — the DEK's **dual identity plane**
//! for talking to Pollek Cloud:
//!
//!   - **Device / workload identity**: an X.509-SVID (SPIFFE) that backs mutual
//!     TLS on the transport, so Cloud can cryptographically tell *which DEK*
//!     sent telemetry. Provisioned by `dek-spire-node` (join-token → SVID),
//!     renewed before expiry, roots kept fresh by the trust-bundle poller.
//!   - **User / tenant identity**: an OAuth/OIDC bearer (Keycloak) that carries
//!     *who* the telemetry is attributed to.
//!
//! This endpoint reports what identity material is provisioned on this device
//! and its live status (SPIFFE ID, SVID expiry, mTLS readiness, OAuth binding)
//! so the dashboard can show the identity plane without exposing any secrets.

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::path::PathBuf;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/tenants/:tenant/identity", get(get_identity))
}

/// Directory the DEK keeps its identity material in.
fn identity_dir() -> PathBuf {
    let base = std::env::var("DEK_LCP_DATA").unwrap_or_else(|_| "./pollek-local-data".into());
    PathBuf::from(base).join("identity")
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn get_identity(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
) -> impl IntoResponse {
    let dir = identity_dir();
    let svid_path = dir.join("svid.pem");
    let key_path = dir.join("svid-key.pem");
    let root_path = dir.join("trust-bundle.pem");

    // ---- Device / workload identity plane (X.509-SVID + mTLS) -------------
    let svid_present = svid_path.exists();
    let key_present = key_path.exists();
    let trust_bundle_present = root_path.exists();
    // mTLS is only usable when the full triple is present.
    let mtls_ready = svid_present && key_present && trust_bundle_present;

    let workload = match std::fs::read_to_string(&svid_path) {
        Ok(pem) => match dek_spire_node::describe_svid(&pem, now_unix()) {
            Ok(info) => json!({
                "provisioned": true,
                "spiffe_id": info.spiffe_id,
                "subject": info.subject,
                "issuer": info.issuer,
                "serial": info.serial,
                "not_before_unix": info.not_before_unix,
                "not_after_unix": info.not_after_unix,
                "seconds_until_expiry": info.seconds_until_expiry,
                "expired": info.expired,
            }),
            Err(e) => json!({ "provisioned": true, "error": format!("unparsable SVID: {e}") }),
        },
        Err(_) => json!({ "provisioned": false }),
    };

    // ---- User / tenant identity plane (OAuth/OIDC) ------------------------
    let oidc_issuer = std::env::var("POLLEK_OIDC_ISSUER").ok();
    let oidc_client_id = std::env::var("POLLEK_OIDC_CLIENT_ID").ok();
    let oauth_configured = oidc_issuer.is_some() || oidc_client_id.is_some();

    (
        StatusCode::OK,
        Json(json!({
            "schema_version": "workload-identity.v1",
            "tenant_id": tenant,
            "device": {
                "actor_id": state.identity.actor_id,
                "workspace_id": state.identity.workspace_id,
                "environment_id": state.identity.environment_id,
            },
            "transport": {
                "mtls_ready": mtls_ready,
                "svid_present": svid_present,
                "private_key_present": key_present,
                "trust_bundle_present": trust_bundle_present,
            },
            "workload_identity": workload,
            "user_identity": {
                "oauth_configured": oauth_configured,
                "oidc_issuer": oidc_issuer,
                "oidc_client_id": oidc_client_id,
                "auth_subject": state.identity.auth_subject,
            },
        })),
    )
}
