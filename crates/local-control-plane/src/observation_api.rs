use crate::state::AppState;
use crate::store::{AiUsageQuery, ObservationEventQuery};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dek_agent_observer::model::AgentObservationEvent;
use serde_json::json;
use std::collections::BTreeMap;

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
        .route(
            "/v1/tenants/:tenant/observations/agents/:agent_id/activity",
            get(agent_activity),
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

    // 2. Correlate an agent-less signal to a discovered agent via the SSOT
    //    process-identity bindings; fall back to a shadow candidate on a miss.
    match crate::correlation::build_correlator(&state, &tenant).await {
        Ok(correlator) => {
            dek_agent_observer::correlate::correlate_event(&mut ev, &correlator);
        }
        Err(e) => {
            tracing::warn!("correlator build failed, shadow-only: {}", e);
            dek_agent_observer::correlate::correlate_shadow_candidate(&mut ev);
        }
    }

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
                .insert_cost_ledger(&ev.tenant_id, &cost_entry)
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
    let ledger_entries = match state.observability_store.list_cost_ledger(&tenant).await {
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

    // Entries are already scoped to this tenant by the store query.
    for entry in ledger_entries {
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

#[derive(serde::Deserialize, Default)]
struct ListQuery {
    kind: Option<String>,
    agent_id: Option<String>,
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
}

fn observation_query(tenant: &str, query: &ListQuery) -> ObservationEventQuery {
    ObservationEventQuery {
        tenant_id: tenant.to_string(),
        agent_ids: query.agent_id.clone().into_iter().collect(),
        event_kind: query.kind.clone(),
        from: query.from.clone(),
        to: query.to.clone(),
        limit: query.limit,
    }
}

async fn list_observations(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let events = match state
        .observability_store
        .query_observation_events(observation_query(&tenant, &query))
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

    (
        StatusCode::OK,
        Json(serde_json::to_value(events).unwrap_or(json!([]))),
    )
}

async fn list_resources(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let mut store_query = observation_query(&tenant, &query);
    store_query.event_kind = Some("resource_access".to_string());
    let events = match state
        .observability_store
        .query_observation_events(store_query)
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

#[derive(serde::Deserialize, Default)]
struct AgentActivityQuery {
    /// Extra ids (comma-separated) that also identify this agent, e.g. the
    /// discovery candidate id when it differs from the canonical agent id.
    alt_ids: Option<String>,
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
}

#[derive(Default)]
struct ResourceRollup {
    resource_type: String,
    verbs: std::collections::BTreeSet<String>,
    access_count: u64,
    total_bytes: i64,
    first_seen: String,
    last_seen: String,
}

/// Per-agent observe view: activity timeline, resource-access rollup, and
/// token/cost usage for one discovered agent.
async fn agent_activity(
    State(state): State<AppState>,
    Path((tenant, agent_id)): Path<(String, String)>,
    Query(query): Query<AgentActivityQuery>,
) -> impl IntoResponse {
    let mut agent_ids = vec![agent_id.clone()];
    for alt in query
        .alt_ids
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        if !agent_ids.iter().any(|id| id == alt) {
            agent_ids.push(alt.to_string());
        }
    }

    let events = match state
        .observability_store
        .query_observation_events(ObservationEventQuery {
            tenant_id: tenant.clone(),
            agent_ids: agent_ids.clone(),
            event_kind: None,
            from: query.from.clone(),
            to: query.to.clone(),
            limit: query.limit,
        })
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

    let mut kind_counts = BTreeMap::<&'static str, u64>::new();
    let mut resource_rollups = BTreeMap::<String, ResourceRollup>::new();
    for event in &events {
        let kind = match event.event_kind {
            dek_agent_observer::model::EventKind::ResourceAccess => "resource_access",
            dek_agent_observer::model::EventKind::ToolCall => "tool_call",
            dek_agent_observer::model::EventKind::LlmCall => "llm_call",
            dek_agent_observer::model::EventKind::Decision => "decision",
            dek_agent_observer::model::EventKind::Generic => "generic",
        };
        *kind_counts.entry(kind).or_default() += 1;

        if let Some(resource) = &event.resource_access {
            let rollup = resource_rollups
                .entry(resource.target_redacted.clone())
                .or_insert_with(|| ResourceRollup {
                    resource_type: resource.resource_type.clone(),
                    first_seen: event.timestamp.clone(),
                    last_seen: event.timestamp.clone(),
                    ..ResourceRollup::default()
                });
            rollup.verbs.insert(resource.verb.clone());
            rollup.access_count += 1;
            rollup.total_bytes += resource.bytes.unwrap_or(0);
            if event.timestamp < rollup.first_seen {
                rollup.first_seen = event.timestamp.clone();
            }
            if event.timestamp > rollup.last_seen {
                rollup.last_seen = event.timestamp.clone();
            }
        }
    }

    let items = dek_agent_observer::activity::activity_items_from_observations(&events);
    let counts = dek_agent_observer::activity::activity_counts(&items);

    let mut resources: Vec<_> = resource_rollups
        .into_iter()
        .map(|(target, rollup)| {
            json!({
                "target": target,
                "resource_type": rollup.resource_type,
                "verbs": rollup.verbs,
                "access_count": rollup.access_count,
                "total_bytes": rollup.total_bytes,
                "first_seen": rollup.first_seen,
                "last_seen": rollup.last_seen,
            })
        })
        .collect();
    resources.sort_by(|a, b| {
        b.get("access_count")
            .and_then(|v| v.as_u64())
            .cmp(&a.get("access_count").and_then(|v| v.as_u64()))
    });

    let usage = agent_usage_rollup(&state, &tenant, &agent_ids, &query).await;

    (
        StatusCode::OK,
        Json(json!({
            "schema_version": "agent-observe-activity.v1",
            "tenant_id": tenant,
            "agent_id": agent_id,
            "matched_agent_ids": agent_ids,
            "generated_at": chrono::Utc::now().to_rfc3339(),
            "counts": {
                "total_events": events.len(),
                "by_kind": kind_counts,
                "total_decisions": counts.total_decisions,
                "denied_actions": counts.denied_actions,
                "mcp_invocations": counts.mcp_invocations,
            },
            "activity": items,
            "resources": resources,
            "usage": usage,
        })),
    )
}

/// Token/cost rollup for one agent across every id it is known by. Computed
/// from raw usage events (rather than `ai_usage_summary`) so the response can
/// split exact vs estimated capture, which the summary shape does not carry.
async fn agent_usage_rollup(
    state: &AppState,
    tenant: &str,
    agent_ids: &[String],
    query: &AgentActivityQuery,
) -> serde_json::Value {
    let mut seen_event_ids = std::collections::HashSet::new();
    let mut usage_events = Vec::new();
    for agent_id in agent_ids {
        if let Ok(events) = state
            .observability_store
            .list_ai_usage_events(AiUsageQuery {
                tenant_id: tenant.to_string(),
                agent_id: Some(agent_id.clone()),
                from: query.from.clone(),
                to: query.to.clone(),
                limit: Some(5_000),
                ..AiUsageQuery::default()
            })
            .await
        {
            for event in events {
                if seen_event_ids.insert(event.event_id.clone()) {
                    usage_events.push(event);
                }
            }
        }
    }

    let mut request_count = 0u64;
    let mut input_tokens = 0i64;
    let mut output_tokens = 0i64;
    let mut cached_input_tokens = 0i64;
    let mut reasoning_output_tokens = 0i64;
    let mut total_tokens = 0i64;
    let mut total_cost = 0f64;
    let mut exact_events = 0u64;
    let mut estimated_events = 0u64;
    let mut currency = "USD".to_string();
    let mut last_event_at: Option<String> = None;
    let mut by_model = BTreeMap::<String, (u64, i64, f64)>::new();

    for event in &usage_events {
        request_count += 1;
        input_tokens += event.tokens.input_tokens;
        output_tokens += event.tokens.output_tokens;
        cached_input_tokens += event.tokens.cached_input_tokens;
        reasoning_output_tokens += event.tokens.reasoning_output_tokens;
        total_tokens += event.tokens.total_tokens;
        total_cost += event.cost.total_cost;
        if event.tokens.estimated {
            estimated_events += 1;
        } else {
            exact_events += 1;
        }
        if !event.cost.currency.is_empty() {
            currency = event.cost.currency.clone();
        }
        let occurred = event.occurred_at.to_rfc3339();
        if last_event_at
            .as_deref()
            .is_none_or(|t| occurred.as_str() > t)
        {
            last_event_at = Some(occurred);
        }
        let model_key = event.model.clone().unwrap_or_else(|| "unknown".to_string());
        let entry = by_model.entry(model_key).or_insert((0, 0, 0.0));
        entry.0 += 1;
        entry.1 += event.tokens.total_tokens;
        entry.2 += event.cost.total_cost;
    }

    let by_model: Vec<_> = by_model
        .into_iter()
        .map(|(model, (requests, tokens, cost))| {
            json!({
                "model": model,
                "request_count": requests,
                "total_tokens": tokens,
                "total_cost": cost,
            })
        })
        .collect();

    json!({
        "request_count": request_count,
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "cached_input_tokens": cached_input_tokens,
        "reasoning_output_tokens": reasoning_output_tokens,
        "total_tokens": total_tokens,
        "total_cost": total_cost,
        "currency": currency,
        "exact_events": exact_events,
        "estimated_events": estimated_events,
        "last_event_at": last_event_at,
        "by_model": by_model,
    })
}
