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
        .route(
            "/v1/tenants/:tenant/observations",
            post(ingest_observation).get(list_observations),
        )
        .route("/v1/tenants/:tenant/observations/costs", get(cost_summary))
        .route(
            "/v1/tenants/:tenant/observations/resources",
            get(list_resources),
        )
}

async fn ingest_observation(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Json(event): Json<AgentObservationEvent>,
) -> impl IntoResponse {
    let mut ev = event;
    ev.tenant_id = tenant.clone();

    // 1. Scope browser-hosted AI events to the same candidate id discovery uses.
    dek_agent_observer::browser_scope::apply_browser_scoped_agent_id(&mut ev);

    // 2. Correlate Shadow Candidates
    dek_agent_observer::correlate::correlate_shadow_candidate(&mut ev);

    // 3. Insert to DB
    if let Err(e) = state
        .observability_store
        .insert_observation_event(&ev)
        .await
    {
        tracing::error!("Failed to insert observation: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        );
    }

    // 4. Calculate V1 Cost Ledger Entry and bridge to canonical V2 usage.
    let catalog_path = std::path::PathBuf::from("pollek-local-data/price_catalog.v1.json");
    let catalog: dek_agent_observer::cost::PriceCatalog = if catalog_path.exists() {
        let content = std::fs::read_to_string(&catalog_path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_else(|_| dek_agent_observer::cost::PriceCatalog {
            catalog_version: "2026-06".into(),
            currency: "USD".into(),
            providers: std::collections::HashMap::new(),
        })
    } else {
        dek_agent_observer::cost::PriceCatalog {
            catalog_version: "2026-06".into(),
            currency: "USD".into(),
            providers: std::collections::HashMap::new(),
        }
    };

    let mut ai_usage_event =
        dek_agent_observer::usage_model::AiUsageEventV1::from_legacy_observation(
            &ev,
            ev.provider.clone(),
        );
    if let Some(provider) = ev.provider.clone() {
        if let Some(cost_entry) = dek_agent_observer::cost::calculate_cost(&ev, &provider, &catalog)
        {
            ai_usage_event.cost = dek_agent_observer::usage_model::CanonicalCostBreakdown {
                currency: cost_entry.currency.clone(),
                input_cost: cost_entry.input_cost,
                output_cost: cost_entry.output_cost,
                total_cost: cost_entry.total_cost,
                price_catalog_version: Some(catalog.catalog_version.clone()),
                cost_source: dek_agent_observer::usage_model::CostSource::PriceCatalogExact,
                estimated: cost_entry.estimated,
                ..dek_agent_observer::usage_model::CanonicalCostBreakdown::default()
            };
            if let Err(e) = state
                .observability_store
                .insert_cost_ledger(&cost_entry)
                .await
            {
                tracing::error!("Failed to insert cost ledger: {}", e);
            }
        }
    }

    // OTel metrics
    dek_agent_observer::otel::emit_span(&ev);
    if ev.token_usage.is_some() || ev.provider.is_some() {
        ai_usage_event = ai_usage_event.finalize();
        if let Err(e) = state
            .observability_store
            .insert_ai_usage_event(&ai_usage_event)
            .await
        {
            tracing::error!("Failed to insert AI usage event: {}", e);
        }
        if let Err(e) = state
            .observability_store
            .upsert_ai_usage_rollup(&ai_usage_event)
            .await
        {
            tracing::error!("Failed to upsert AI usage rollup: {}", e);
        }
        if let Err(e) = crate::usage_api::publish_ai_usage_event(&state, &ai_usage_event).await {
            tracing::error!("Failed to publish AI usage telemetry: {}", e);
        }
        dek_agent_observer::otel::emit_usage_span(&ai_usage_event);
    }

    // 5. Generate Policy Suggestions
    // We mock passing all events by just passing this one event
    if let Ok(suggestions) =
        dek_policy_suggester::api::generate_suggestions(&tenant, &[], &[ev.clone()])
    {
        for sug in suggestions {
            if let Err(e) = state
                .observability_store
                .upsert_policy_suggestion(&sug)
                .await
            {
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
    let ledger_entries = match state.observability_store.list_cost_ledger().await {
        Ok(entries) => entries,
        Err(e) => {
            tracing::error!("Failed to fetch cost ledger: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            );
        }
    };

    let mut total_cost = 0.0;
    let mut total_tokens = 0;
    let mut provider_costs = std::collections::HashMap::new();

    for entry in ledger_entries {
        // Simple mock to filter by tenant if we had tenant_id in cost_ledger
        // For now we aggregate all since local-control-plane is mostly single-tenant in demo.
        total_cost += entry.total_cost;
        total_tokens += entry.total_tokens;
        *provider_costs.entry(entry.provider).or_insert(0.0) += entry.total_cost;
    }

    let mut agent_breakdown = std::collections::HashMap::new();
    let mut agent_token_breakdown = std::collections::HashMap::new();
    let mut agent_usage_breakdown = serde_json::Map::new();
    if let Ok(agent_costs) = state
        .observability_store
        .cost_breakdown_by_agent(&tenant, "1970-01-01")
        .await
    {
        for ac in agent_costs {
            agent_breakdown.insert(ac.agent_id.clone(), ac.cost);
            agent_token_breakdown.insert(ac.agent_id.clone(), ac.tokens);
            agent_usage_breakdown.insert(
                ac.agent_id,
                json!({
                    "cost": ac.cost,
                    "tokens": ac.tokens,
                }),
            );
        }
    }

    let result = json!({
        "schema_version": "cost-summary.v1",
        "tenant_id": tenant,
        "period": "current_month",
        "total_estimated_cost_usd": total_cost,
        "total_tokens": total_tokens,
        "provider_breakdown": provider_costs,
        "agent_breakdown": agent_breakdown,
        "agent_token_breakdown": agent_token_breakdown,
        "agent_usage_breakdown": agent_usage_breakdown
    });

    (StatusCode::OK, Json(result))
}

#[derive(serde::Deserialize)]
struct ListQuery {
    kind: Option<String>,
}

async fn list_observations(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    axum::extract::Query(query): axum::extract::Query<ListQuery>,
) -> impl IntoResponse {
    let events = match state
        .observability_store
        .list_observation_events(&tenant)
        .await
    {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        }
    };

    let filtered: Vec<_> = if let Some(kind_str) = query.kind {
        let kind_enum = match kind_str.as_str() {
            "decision" => dek_agent_observer::model::EventKind::Decision,
            "tool_call" => dek_agent_observer::model::EventKind::ToolCall,
            "llm_call" => dek_agent_observer::model::EventKind::LlmCall,
            "resource_access" => dek_agent_observer::model::EventKind::ResourceAccess,
            _ => dek_agent_observer::model::EventKind::Generic,
        };
        events
            .into_iter()
            .filter(|e| e.event_kind == kind_enum)
            .collect()
    } else {
        events
    };

    (
        StatusCode::OK,
        Json(serde_json::to_value(filtered).unwrap_or(json!([]))),
    )
}

async fn list_resources(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
) -> impl IntoResponse {
    let events = match state
        .observability_store
        .list_observation_events(&tenant)
        .await
    {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        }
    };

    let resources: Vec<_> = events
        .into_iter()
        .filter(|e| e.event_kind == dek_agent_observer::model::EventKind::ResourceAccess)
        .filter_map(|e| {
            e.resource_access.map(|ra| {
                json!({
                    "resource_id": e.event_id,
                    "name": ra.target_redacted,
                    "resource_type": ra.resource_type,
                    "uri": ra.target_redacted,
                    "classification": "observed",
                    "verb": ra.verb,
                    "agent_id": e.agent_id,
                    "timestamp": e.timestamp,
                })
            })
        })
        .collect();

    (StatusCode::OK, Json(serde_json::Value::Array(resources)))
}
