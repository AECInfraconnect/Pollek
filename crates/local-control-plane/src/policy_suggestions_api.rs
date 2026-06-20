use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};

use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/policy-suggestions",
            get(list_suggestions),
        )
        .route(
            "/v1/tenants/:tenant/policy-suggestions/generate",
            post(generate),
        )
}

async fn list_suggestions(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let items = st
        .registry_store
        .list_raw(&tenant, "policy_suggestion")
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(serde_json::json!({
        "schema_version": "policy-suggestions-list.v1",
        "suggestions": items
    })))
}

async fn generate(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let raw_candidates = st
        .registry_store
        .list_raw(&tenant, "discovery_candidate")
        .await
        .map_err(ApiError::Internal)?;

    let mut candidates = vec![];
    for raw in raw_candidates {
        if let Ok(c) = serde_json::from_value(raw) {
            candidates.push(c);
        }
    }

    let raw_events = st
        .registry_store
        .list_raw(&tenant, "obs")
        .await
        .unwrap_or_default();

    let mut events = vec![];
    for raw in raw_events {
        if let Ok(e) = serde_json::from_value(raw) {
            events.push(e);
        }
    }

    let suggestions = dek_policy_suggester::api::generate_suggestions(&tenant, &candidates, &events)
        .map_err(|_| ApiError::Internal(anyhow::anyhow!("failed generation")))?;

    for s in &suggestions {
        if let Ok(v) = serde_json::to_value(s) {
            let _ = st
                .registry_store
                .upsert_raw(&tenant, "policy_suggestion", &s.suggestion_id, &v)
                .await;
        }
    }
    Ok(Json(serde_json::json!({
        "schema_version": "generate-suggestions-response.v1",
        "generated_count": suggestions.len()
    })))
}
