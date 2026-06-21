use axum::{routing::post, Json, Router};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct DraftRequest {
    pub tenant: String,
    pub prompt: String,
    #[serde(default)]
    pub target_entities: Vec<String>,
}

pub fn router() -> Router<crate::state::AppState> {
    Router::new().route("/v1/policy-intents/draft", post(draft_policy_intent))
}

async fn draft_policy_intent(Json(input): Json<DraftRequest>) -> Json<serde_json::Value> {
    // Deterministic beta stub. Replace with AI orchestrator in Pollen Cloud.
    let ppi = serde_json::json!({
        "apiVersion": "pollen.ai/v1alpha1",
        "kind": "PolicyIntent",
        "metadata": {
            "id": format!("pol-{}", chrono::Utc::now().timestamp_millis()),
            "tenant": input.tenant,
            "name": "Generated from NL prompt",
            "version": "0.1.0"
        },
        "spec": {
            "decisionMode": "enforce",
            "priority": 500,
            "subjects": {
                "include": [
                    { "type": "agent", "selector": { "labels": { "approved": "true" } } }
                ]
            },
            "actions": ["mcp.call_tool"],
            "resources": {
                "include": [
                    { "type": "tool", "selector": { "risk": "low" } }
                ]
            },
            "constraints": {
                "requireDevicePosture": "healthy"
            },
            "enforcement": {
                "preferredPepTypes": ["mcp_proxy"],
                "fallback": "deny"
            },
            "obligations": {
                "audit": {
                    "level": "decision",
                    "includeInput": "redacted"
                }
            }
        }
    });

    Json(serde_json::json!({
        "ppi": ppi,
        "explanation": "Drafted from deterministic template. Review before deploy."
    }))
}
