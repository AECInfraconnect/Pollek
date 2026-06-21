use axum::{extract::State, routing::get, Json, Router};
use std::sync::Arc;

pub struct AgentAppState {
    // normally contains stores or db connections
}

pub fn router(state: Arc<AgentAppState>) -> Router {
    Router::new()
        .route("/v1/agents/bindings", get(list_bindings))
        .route("/v1/agents/fingerprints", get(list_fingerprints))
        .with_state(state)
}

async fn list_bindings(State(_state): State<Arc<AgentAppState>>) -> Json<serde_json::Value> {
    // Mocked response for now. Will wire to dek_agent_observer::SharedBindingStore in the real app.
    Json(serde_json::json!({
        "bindings": []
    }))
}

async fn list_fingerprints(State(_state): State<Arc<AgentAppState>>) -> Json<serde_json::Value> {
    // Mocked response for now. Will wire to dek_fingerprint_defs::FingerprintService
    Json(serde_json::json!({
        "fingerprints": []
    }))
}
