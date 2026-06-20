use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dek_agent_observer::model::AgentObservationEvent;
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tenants/:tenant/observations", post(ingest_observation))
        .route("/v1/tenants/:tenant/observations/costs", get(cost_summary))
}

async fn ingest_observation(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Json(event): Json<AgentObservationEvent>,
) -> impl IntoResponse {
    let mut ev = event;
    ev.tenant_id = tenant.clone();
    
    // Convert to Value and upsert to store as raw event for now
    if let Ok(v) = serde_json::to_value(&ev) {
        let _ = state.registry_store.upsert_raw(&tenant, "obs", &ev.event_id, &v).await;
    }
    
    (StatusCode::CREATED, Json(json!({"status": "ingested"})))
}

async fn cost_summary(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
) -> impl IntoResponse {
    let records = match state.registry_store.list_raw(&tenant, "obs").await {
        Ok(r) => r,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({}))),
    };

    let mut total_cost = 0.0;
    
    for r in records {
        if let Some(event_id) = r.get("event_id") {
            if let Some(cost) = r.get("cost") {
                if let Some(c) = cost.get("total_cost").and_then(|v| v.as_f64()) {
                    total_cost += c;
                }
            }
        }
    }

    let result = json!({
        "schema_version": "cost-summary.v1",
        "tenant_id": tenant,
        "currency": "USD",
        "total_cost": total_cost,
    });

    (StatusCode::OK, Json(result))
}
