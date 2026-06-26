use crate::state::AppState;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ConsentPayload {
    pub kind: String,
    pub version: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/consent/agreements", get(get_agreements))
        .route("/v1/consent", post(post_consent))
}

async fn get_agreements(State(_state): State<AppState>) -> Json<serde_json::Value> {
    let agreements = vec![
        serde_json::json!({
            "id": "eula",
            "version": "eula-2026-06",
            "title": "End User License Agreement",
            "body_markdown": "By using Pollek DEK, you agree to the EULA...",
            "required": true
        }),
        serde_json::json!({
            "id": "privacy_notice",
            "version": "privacy-2026-06",
            "title": "Privacy Notice",
            "body_markdown": "Pollek DEK operates locally. No data leaves your machine unless you enable cloud sync.",
            "required": true
        }),
        serde_json::json!({
            "id": "browser_history_scan",
            "version": "bh-2026-06",
            "title": "Browser History Scan (Optional)",
            "body_markdown": "Allow scanning of browser history for Web-based AI agents.",
            "required": false
        }),
    ];
    Json(serde_json::json!({ "agreements": agreements }))
}

async fn post_consent(
    State(_state): State<AppState>,
    Json(_payload): Json<ConsentPayload>,
) -> Json<serde_json::Value> {
    // In a real implementation, this would save to dek_consent::ConsentStore
    Json(serde_json::json!({ "status": "ok" }))
}
