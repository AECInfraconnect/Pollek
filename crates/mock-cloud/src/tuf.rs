#![allow(clippy::unwrap_used, clippy::expect_used)]
use crate::spire::is_device_revoked;
use crate::state::AppState;
use crate::{bundle_pubkey_b64, BUNDLE_SEED};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use ed25519_dalek::Signer;
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant_id/devices/:device_id/bundles/metadata/:role",
            get(get_tuf_metadata),
        )
        .route(
            "/v1/tenants/:tenant_id/devices/:device_id/bundles/artifacts/:hash",
            get(get_tuf_artifact),
        )
}

async fn get_tuf_metadata(
    Path((_tenant_id, device_id, role)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if is_device_revoked(&state, &device_id) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "device revoked"})),
        );
    }

    let signing_key = ed25519_dalek::SigningKey::from_bytes(&BUNDLE_SEED);

    let now = Utc::now();
    let expires = now + chrono::Duration::days(7);

    let (payload, _role_name) = match role.as_str() {
        "root.json" => (
            json!({
                "signed": {
                    "_type": "root",
                    "spec_version": "1.0",
                    "version": 1,
                    "expires": expires.to_rfc3339(),
                    "keys": {
                        "key-prod-1": {
                            "keytype": "ed25519",
                            "scheme": "ed25519",
                            "keyval": {
                                "public": bundle_pubkey_b64()
                            }
                        }
                    },
                    "roles": {
                        "root": { "keyids": ["key-prod-1"], "threshold": 1 },
                        "snapshot": { "keyids": ["key-prod-1"], "threshold": 1 },
                        "targets": { "keyids": ["key-prod-1"], "threshold": 1 },
                        "timestamp": { "keyids": ["key-prod-1"], "threshold": 1 }
                    }
                },
                "signatures": []
            }),
            "root",
        ),
        "targets.json" => {
            let routes_json = json!([
                { "id": "route_tools_call", "priority": 100, "match_rule": { "method": "tools/call", "tool_category": null }, "pdp_required": ["openfga"] }
            ]);
            let manifest_json = json!({
                "manifest_version": "1.0",
                "bundle_id": "bnd-123",
                "bundle_version": "1.0.0",
                "bundle_generation": 1,
                "tenant_id": "tenant-production-1",
                "created_at": "2024-01-01T00:00:00Z",
                "expires_at": "2025-01-01T00:00:00Z",
                "activation_mode": "full",
                "artifacts": []
            });

            use sha2::{Sha256, Digest};
            let mut h1 = Sha256::new(); h1.update(serde_json::to_vec(&routes_json).unwrap());
            let routes_hash = hex::encode(h1.finalize());

            let mut h2 = Sha256::new(); h2.update(serde_json::to_vec(&manifest_json).unwrap());
            let manifest_hash = hex::encode(h2.finalize());

            (
            json!({
                "signed": {
                    "_type": "targets",
                    "spec_version": "1.0",
                    "version": 1,
                    "expires": expires.to_rfc3339(),
                    "targets": {
                        "routes.json": {
                            "hashes": {
                                "sha256": routes_hash
                            },
                            "length": 1234
                        },
                        "bundle_manifest.json": {
                            "hashes": {
                                "sha256": manifest_hash
                            },
                            "length": 5678
                        }
                    }
                },
                "signatures": []
            }),
            "targets",
            )
        },
        "snapshot.json" => (
            json!({
                "signed": {
                    "_type": "snapshot",
                    "spec_version": "1.0",
                    "version": 1,
                    "expires": expires.to_rfc3339(),
                    "meta": {
                        "targets.json": {
                            "version": 1
                        }
                    }
                },
                "signatures": []
            }),
            "snapshot",
        ),
        "timestamp.json" => (
            json!({
                "signed": {
                    "_type": "timestamp",
                    "spec_version": "1.0",
                    "version": 1,
                    "expires": expires.to_rfc3339(),
                    "meta": {
                        "snapshot.json": {
                            "version": 1
                        }
                    }
                },
                "signatures": []
            }),
            "timestamp",
        ),
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "role not found"})),
            )
        }
    };

    let signed_bytes = serde_jcs::to_vec(&payload["signed"]).unwrap();
    let signature = signing_key.sign(&signed_bytes);

    let mut response = payload;
    use base64::Engine;
    response["signatures"] = json!([{
        "keyid": "key-prod-1",
        "sig": base64::prelude::BASE64_STANDARD.encode(signature.to_bytes())
    }]);

    (StatusCode::OK, Json(response))
}

async fn get_tuf_artifact(
    Path((_tenant_id, _device_id, hash)): Path<(String, String, String)>,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    let routes_json = json!([
        { "id": "route_tools_call", "priority": 100, "match_rule": { "method": "tools/call", "tool_category": null }, "pdp_required": ["openfga"] }
    ]);
    let manifest_json = json!({
        "manifest_version": "1.0",
        "bundle_id": "bnd-123",
        "bundle_version": "1.0.0",
        "bundle_generation": 1,
        "tenant_id": "tenant-production-1",
        "created_at": "2024-01-01T00:00:00Z",
        "expires_at": "2025-01-01T00:00:00Z",
        "activation_mode": "full",
        "artifacts": []
    });

    use sha2::{Sha256, Digest};
    let mut h1 = Sha256::new(); h1.update(serde_json::to_vec(&routes_json).unwrap());
    let routes_hash = hex::encode(h1.finalize());

    let mut h2 = Sha256::new(); h2.update(serde_json::to_vec(&manifest_json).unwrap());
    let manifest_hash = hex::encode(h2.finalize());

    if hash == routes_hash {
        (StatusCode::OK, Json(routes_json))
    } else if hash == manifest_hash {
        (StatusCode::OK, Json(manifest_json))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "artifact not found"})),
        )
    }
}
