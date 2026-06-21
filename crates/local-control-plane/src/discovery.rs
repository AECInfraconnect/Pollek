use crate::state::AppState;
use axum::{routing::get, Json, Router};

pub fn router() -> Router<AppState> {
    Router::new().route("/.well-known/pollen-contract", get(get_discovery))
}

async fn get_discovery() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "schema_version": "contract-discovery.v1",
        "supported": ["1.0"],
        "preferred": "1.0",
        "minimum_dek_version": "1.0.0-beta.6",
        "sunset": { "0.9": "2026-10-01T00:00:00Z" },
        "capabilities": [
            "contract.discovery.v1",
            "bundle.signed-envelope.v1",
            "telemetry.batch.v1",
            "policy.opa-wasm.v1",
            "policy.cedar.v1",
            "policy.openfga.v1"
        ]
    }))
}
