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

    let mut svid_spiffe_id: Option<String> = None;
    let workload = match std::fs::read_to_string(&svid_path) {
        Ok(pem) => match dek_spire_node::describe_svid(&pem, now_unix()) {
            Ok(info) => {
                svid_spiffe_id = info.spiffe_id.clone();
                json!({
                    "provisioned": true,
                    "spiffe_id": info.spiffe_id,
                    "subject": info.subject,
                    "issuer": info.issuer,
                    "serial": info.serial,
                    "not_before_unix": info.not_before_unix,
                    "not_after_unix": info.not_after_unix,
                    "seconds_until_expiry": info.seconds_until_expiry,
                    "expired": info.expired,
                })
            }
            Err(e) => json!({ "provisioned": true, "error": format!("unparsable SVID: {e}") }),
        },
        Err(_) => json!({ "provisioned": false }),
    };

    // ---- Tenant binding (Cloud hand-off asks #2 + #3) ---------------------
    // The DEK presents its verified SPIFFE ID via `x-pollek-spiffe-id`; Cloud's
    // trusted ingress enforces `tenant/<id> == request tenant`, and (when JWT
    // enforcement is on) the bearer's `tenant_id` claim must equal it too. When
    // an SVID is present its tenant segment MUST match the request tenant or the
    // sync client fails closed rather than assert an unprovable tenant.
    let presented_spiffe_id = svid_spiffe_id.clone().or_else(|| {
        std::env::var("POLLEK_SPIFFE_ID")
            .ok()
            .filter(|v| !v.is_empty())
    });
    let spiffe_tenant = presented_spiffe_id
        .as_deref()
        .and_then(crate::cloud_sync_client::tenant_from_spiffe_id);
    let binding_consistent = match &spiffe_tenant {
        Some(t) => *t == tenant,
        None => true, // no SVID ⇒ nothing to contradict (bearer/dev)
    };
    let tenant_binding = json!({
        "request_tenant": tenant,
        "presented_via": "x-pollek-spiffe-id",
        "presented_spiffe_id": presented_spiffe_id,
        "spiffe_tenant": spiffe_tenant,
        "token_claim_enforced": "tenant_id",
        "consistent": binding_consistent,
        "fail_closed": !binding_consistent,
    });

    // ---- User / tenant identity plane (OAuth/OIDC) ------------------------
    let oidc_issuer = std::env::var("POLLEK_OIDC_ISSUER").ok();
    let oidc_client_id = std::env::var("POLLEK_OIDC_CLIENT_ID").ok();
    let oauth_configured = oidc_issuer.is_some() || oidc_client_id.is_some();

    // Which credential the DEK↔Cloud token exchange will use, most→least
    // preferred. private_key_jwt (JWT-SVID) proves workload identity with no
    // shared secret; client_credentials is the shared-secret fallback.
    let auth_mechanism = if std::env::var("POLLEK_OIDC_CLIENT_ASSERTION")
        .ok()
        .filter(|v| !v.is_empty())
        .is_some()
    {
        "private_key_jwt"
    } else if std::env::var("POLLEK_OIDC_CLIENT_SECRET")
        .ok()
        .filter(|v| !v.is_empty())
        .is_some()
    {
        "client_credentials"
    } else if std::env::var("DEK_CLOUD_API_KEY")
        .ok()
        .filter(|v| !v.is_empty())
        .is_some()
    {
        "static_bearer"
    } else {
        "none"
    };
    // Transport is mutual-TLS the moment the SVID triple is present.
    let transport_mode = if mtls_ready { "mtls" } else { "bearer" };

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
                "mode": transport_mode,
                "mtls_ready": mtls_ready,
                "svid_present": svid_present,
                "private_key_present": key_present,
                "trust_bundle_present": trust_bundle_present,
            },
            "workload_identity": workload,
            "tenant_binding": tenant_binding,
            "user_identity": {
                "oauth_configured": oauth_configured,
                "auth_mechanism": auth_mechanism,
                "oidc_issuer": oidc_issuer,
                "oidc_client_id": oidc_client_id,
                "auth_subject": state.identity.auth_subject,
            },
        })),
    )
}
