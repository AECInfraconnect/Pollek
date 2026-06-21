use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dek_agent_observer::model::{AgentObservationEvent, CostLedgerEntry};
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

    // 1. Correlate Shadow Candidates
    dek_agent_observer::correlate::correlate_shadow_candidate(&mut ev);

    // 2. Insert to DB
    if let Err(e) = state.observability_store.insert_observation_event(&ev).await {
        tracing::error!("Failed to insert observation: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})));
    }

    // 3. Calculate Cost Ledger Entry
    // We mock a price catalog for now
    let mut catalog_providers = std::collections::HashMap::new();
    let mut openai_models = std::collections::HashMap::new();
    openai_models.insert("gpt-4o".into(), dek_agent_observer::cost::ModelPrice { input_per_1m: 5.0, output_per_1m: 15.0 });
    catalog_providers.insert("openai".into(), openai_models);
    
    let catalog = dek_agent_observer::cost::PriceCatalog {
        catalog_version: "2026-06".into(),
        currency: "USD".into(),
        providers: catalog_providers,
    };

    // Extract provider heuristic (e.g. from token_usage model or payload)
    // For now we assume openai if there is a model.
    if let Some(cost_entry) = dek_agent_observer::cost::calculate_cost(&ev, "openai", &catalog) {
        if let Err(e) = state.observability_store.insert_cost_ledger(&cost_entry).await {
            tracing::error!("Failed to insert cost ledger: {}", e);
        }
    }

    // 4. Generate Policy Suggestions
    // We mock passing all events by just passing this one event
    if let Ok(suggestions) = dek_policy_suggester::api::generate_suggestions(&tenant, &[], &[ev.clone()]) {
        for sug in suggestions {
            if let Err(e) = state.observability_store.upsert_policy_suggestion(&sug).await {
                tracing::error!("Failed to upsert suggestion: {}", e);
            }
        }
    }

    (StatusCode::CREATED, Json(json!({"status": "ingested"})))
}

async fn cost_summary(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
) -> impl IntoResponse {
    // A real implementation would query the cost_ledger table directly with SQL aggregation.
    // For now, we list observations and re-aggregate, or list costs.
    // We don't have list_cost_ledger in our store trait yet, so we'll just mock it or skip the detail implementation.
    
    let result = json!({
        "schema_version": "cost-summary.v1",
        "tenant_id": tenant,
        "currency": "USD",
        "total_cost": 0.0,
    });

    (StatusCode::OK, Json(result))
}
