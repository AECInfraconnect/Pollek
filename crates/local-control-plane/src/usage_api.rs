use crate::state::AppState;
use crate::store::{AiUsageQuery, AiUsageSummaryQuery};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{Datelike, Duration, TimeZone, Utc};
use dek_agent_observer::usage_budget::{
    evaluate_budget, AiBudgetLimit, BudgetAction, UsageWindowTotals,
};
use dek_agent_observer::usage_cost::{calculate_cost_v2, CostCalculationInput, PriceCatalogV2};
use dek_agent_observer::usage_model::{AiUsageEventV1, CostSource};
use futures_util::{Stream, StreamExt};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/usage/events",
            post(ingest_usage_event).get(list_usage_events),
        )
        .route(
            "/v1/tenants/:tenant/usage/events/batch",
            post(ingest_usage_events_batch),
        )
        .route("/v1/tenants/:tenant/usage/summary", get(usage_summary))
        .route(
            "/v1/tenants/:tenant/usage/agents/:agent_id",
            get(agent_usage_summary),
        )
        .route("/v1/tenants/:tenant/usage/ledger", get(usage_ledger))
        .route("/v1/tenants/:tenant/usage/stream", get(usage_stream))
        .route("/v1/tenants/:tenant/usage/budgets", get(list_budgets))
        .route(
            "/v1/tenants/:tenant/usage/budgets/:budget_id",
            put(upsert_budget),
        )
        .route("/v1/tenants/:tenant/usage/reconcile", post(reconcile_usage))
}

#[derive(Debug, Deserialize)]
struct UsageEventsBatch {
    events: Vec<AiUsageEventV1>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UsageEventsParams {
    from: Option<String>,
    to: Option<String>,
    agent_id: Option<String>,
    agent_type: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    task_id: Option<String>,
    session_id: Option<String>,
    surface: Option<String>,
    sync_status: Option<String>,
    limit: Option<i64>,
    cursor: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UsageSummaryParams {
    from: Option<String>,
    to: Option<String>,
    bucket: Option<String>,
    agent_id: Option<String>,
    agent_type: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    task_id: Option<String>,
    session_id: Option<String>,
    surface: Option<String>,
    group_by: Option<String>,
}

async fn ingest_usage_event(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Json(event): Json<AiUsageEventV1>,
) -> impl IntoResponse {
    match persist_usage_event(&state, &tenant, event).await {
        Ok(event) => (StatusCode::CREATED, Json(json!({ "item": event }))),
        Err((status, message)) => (status, Json(json!({ "error": message }))),
    }
}

async fn ingest_usage_events_batch(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Json(batch): Json<UsageEventsBatch>,
) -> impl IntoResponse {
    let mut accepted = 0_i64;
    let mut rejected = 0_i64;
    for event in batch.events {
        match persist_usage_event(&state, &tenant, event).await {
            Ok(_) => accepted += 1,
            Err(_) => rejected += 1,
        }
    }
    (
        StatusCode::OK,
        Json(json!({
            "schema_version": "ai-usage-batch-ingest-response.v1",
            "accepted": accepted,
            "rejected": rejected
        })),
    )
}

async fn list_usage_events(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Query(params): Query<UsageEventsParams>,
) -> impl IntoResponse {
    let query = usage_query(tenant, params);
    match state.observability_store.list_ai_usage_events(query).await {
        Ok(events) => (
            StatusCode::OK,
            Json(json!({
                "schema_version": "ai-usage-event-page.v1",
                "items": events,
                "next_cursor": null
            })),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err.to_string() })),
        ),
    }
}

async fn usage_summary(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Query(params): Query<UsageSummaryParams>,
) -> impl IntoResponse {
    usage_summary_response(state, summary_query(tenant, params)).await
}

async fn agent_usage_summary(
    State(state): State<AppState>,
    Path((tenant, agent_id)): Path<(String, String)>,
    Query(mut params): Query<UsageSummaryParams>,
) -> impl IntoResponse {
    params.agent_id = Some(agent_id);
    usage_summary_response(state, summary_query(tenant, params)).await
}

async fn usage_ledger(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Query(params): Query<UsageEventsParams>,
) -> impl IntoResponse {
    let query = usage_query(tenant, params);
    match state.observability_store.list_ai_usage_events(query).await {
        Ok(events) => (
            StatusCode::OK,
            Json(json!({
                "schema_version": "ai-usage-ledger.v1",
                "items": events
            })),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err.to_string() })),
        ),
    }
}

async fn usage_summary_response(
    state: AppState,
    query: AiUsageSummaryQuery,
) -> (StatusCode, Json<Value>) {
    match state.observability_store.ai_usage_summary(query).await {
        Ok(summary) => (StatusCode::OK, Json(json!(summary))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err.to_string() })),
        ),
    }
}

async fn usage_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.telemetry_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| async move {
        match res {
            Ok(env)
                if env.event_type == "ai_usage_event" || env.event_type == "ai_budget_alert" =>
            {
                let data = json!({
                    "type": env.event_type,
                    "data": env.payload,
                });
                serde_json::to_string(&data)
                    .ok()
                    .map(|payload| Ok(Event::default().event(env.event_type).data(payload)))
            }
            _ => None,
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::new())
}

async fn list_budgets(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
) -> impl IntoResponse {
    match state.observability_store.list_ai_budgets(&tenant).await {
        Ok(items) => (
            StatusCode::OK,
            Json(json!({
                "schema_version": "ai-budget-limit-list.v1",
                "items": items
            })),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err.to_string() })),
        ),
    }
}

async fn upsert_budget(
    State(state): State<AppState>,
    Path((tenant, budget_id)): Path<(String, String)>,
    Json(mut budget): Json<AiBudgetLimit>,
) -> impl IntoResponse {
    let now = Utc::now().to_rfc3339();
    budget.schema_version = if budget.schema_version.is_empty() {
        "ai-budget-limit.v1".to_string()
    } else {
        budget.schema_version
    };
    budget.budget_id = budget_id;
    budget.tenant_id = tenant;
    if budget.created_at.is_empty() {
        budget.created_at = now.clone();
    }
    budget.updated_at = now;

    match state.observability_store.upsert_ai_budget(&budget).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "item": budget }))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err.to_string() })),
        ),
    }
}

async fn reconcile_usage(
    Path(tenant): Path<String>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    (
        StatusCode::ACCEPTED,
        Json(json!({
            "schema_version": "ai-usage-reconcile-response.v1",
            "tenant_id": tenant,
            "status": "accepted",
            "received": payload
        })),
    )
}

async fn persist_usage_event(
    state: &AppState,
    tenant: &str,
    mut event: AiUsageEventV1,
) -> Result<AiUsageEventV1, (StatusCode, String)> {
    event.tenant_id = tenant.to_string();
    event = apply_cost_catalog(event);
    event = event.finalize();

    if has_unredacted_secret(&event.provider_usage_raw) || has_unredacted_secret(&event.metadata) {
        return Err((
            StatusCode::BAD_REQUEST,
            "unredacted secret detected in AI usage payload".to_string(),
        ));
    }

    state
        .observability_store
        .insert_ai_usage_event(&event)
        .await
        .map_err(internal_error)?;
    state
        .observability_store
        .upsert_ai_usage_rollup(&event)
        .await
        .map_err(internal_error)?;
    publish_ai_usage_event(state, &event)
        .await
        .map_err(internal_error)?;
    emit_budget_alerts(state, &event).await;

    Ok(event)
}

pub async fn publish_ai_usage_event(
    state: &AppState,
    event: &AiUsageEventV1,
) -> anyhow::Result<()> {
    let payload = to_payload_map(serde_json::to_value(event)?);
    let envelope = pollen_contract::PollenTelemetryEnvelopeV1 {
        schema_version: "telemetry-envelope.v1".to_string(),
        event_id: event.event_id.clone(),
        event_type: "ai_usage_event".to_string(),
        timestamp: event.received_at,
        tenant_id: event.tenant_id.clone(),
        workspace_id: event.workspace_id.clone(),
        environment_id: Some(state.identity.environment_id.clone()),
        device_id: event
            .device_id
            .clone()
            .unwrap_or_else(|| "local-device".to_string()),
        trace_id: Some(event.trace_id.clone()),
        span_id: Some(event.span_id.clone()),
        redaction_applied: true,
        payload,
    };
    publish_envelope(state, envelope).await
}

async fn emit_budget_alerts(state: &AppState, event: &AiUsageEventV1) {
    let Ok(budgets) = state
        .observability_store
        .list_ai_budgets(&event.tenant_id)
        .await
    else {
        return;
    };
    for budget in budgets
        .iter()
        .filter(|budget| budget_matches_event(budget, event))
    {
        let from = window_start(&budget.window);
        let summary = state
            .observability_store
            .ai_usage_summary(AiUsageSummaryQuery {
                tenant_id: event.tenant_id.clone(),
                from: Some(from),
                agent_id: event.agent_id.clone(),
                provider: event.provider.clone(),
                model: event.model.clone(),
                surface: Some(event.surface.clone()),
                ..AiUsageSummaryQuery::default()
            })
            .await;
        let Ok(summary) = summary else {
            continue;
        };
        let evaluation = evaluate_budget(
            budget,
            UsageWindowTotals {
                cost: summary.totals.total_cost,
                tokens: summary.totals.total_tokens,
            },
            UsageWindowTotals::default(),
        );
        if matches!(evaluation.action, BudgetAction::Allow) {
            continue;
        }
        let payload = json!({
            "schema_version": "ai-budget-alert.v1",
            "budget_id": budget.budget_id,
            "usage_event_id": event.event_id,
            "evaluation": evaluation,
        });
        let envelope = pollen_contract::PollenTelemetryEnvelopeV1 {
            schema_version: "telemetry-envelope.v1".to_string(),
            event_id: format!("budget_alert_{}", event.event_id),
            event_type: "ai_budget_alert".to_string(),
            timestamp: Utc::now(),
            tenant_id: event.tenant_id.clone(),
            workspace_id: event.workspace_id.clone(),
            environment_id: Some(state.identity.environment_id.clone()),
            device_id: event
                .device_id
                .clone()
                .unwrap_or_else(|| "local-device".to_string()),
            trace_id: Some(event.trace_id.clone()),
            span_id: Some(event.span_id.clone()),
            redaction_applied: true,
            payload: to_payload_map(payload),
        };
        if let Err(err) = publish_envelope(state, envelope).await {
            tracing::warn!("failed to publish AI budget alert: {}", err);
        }
    }
}

async fn publish_envelope(
    state: &AppState,
    envelope: pollen_contract::PollenTelemetryEnvelopeV1,
) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(&envelope)?;
    state
        .secure_spool
        .push(dek_secure_spool::sqlite_spool::Priority::Normal, &bytes)?;
    let value = serde_json::to_value(&envelope)?;
    state
        .telemetry_store
        .put_telemetry(
            &envelope.tenant_id,
            &envelope.event_type,
            &envelope.event_id,
            &value,
        )
        .await?;
    let _sent = state.telemetry_tx.send(envelope);
    Ok(())
}

fn usage_query(tenant_id: String, params: UsageEventsParams) -> AiUsageQuery {
    AiUsageQuery {
        tenant_id,
        from: params.from,
        to: params.to,
        agent_id: params.agent_id,
        agent_type: params.agent_type,
        provider: params.provider,
        model: params.model,
        task_id: params.task_id,
        session_id: params.session_id,
        surface: params.surface,
        sync_status: params.sync_status,
        limit: params.limit,
        cursor: params.cursor,
    }
}

fn summary_query(tenant_id: String, params: UsageSummaryParams) -> AiUsageSummaryQuery {
    AiUsageSummaryQuery {
        tenant_id,
        from: params.from,
        to: params.to,
        bucket: params.bucket,
        agent_id: params.agent_id,
        agent_type: params.agent_type,
        provider: params.provider,
        model: params.model,
        task_id: params.task_id,
        session_id: params.session_id,
        surface: params.surface,
        group_by: params.group_by,
    }
}

fn apply_cost_catalog(mut event: AiUsageEventV1) -> AiUsageEventV1 {
    if !matches!(event.cost.cost_source, CostSource::Unknown) {
        return event;
    }
    let Some(provider) = event.provider.clone() else {
        return event;
    };
    let Some(model) = event.model.clone() else {
        return event;
    };
    let Some(catalog) = load_price_catalog_v2() else {
        return event;
    };
    event.cost = calculate_cost_v2(
        &catalog,
        CostCalculationInput {
            provider: &provider,
            provider_api: event.provider_api.as_deref(),
            model: &model,
            occurred_at: event.occurred_at,
            tokens: &event.tokens,
            provider_reported_cost: None,
            provider_reported_currency: None,
        },
    );
    event
}

fn load_price_catalog_v2() -> Option<PriceCatalogV2> {
    let path = std::path::PathBuf::from("pollen-local-data/price_catalog.v2.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

fn has_unredacted_secret(value: &Value) -> bool {
    let blob = value.to_string().to_lowercase();
    blob.contains("authorization:")
        || blob.contains("\"authorization\"")
        || blob.contains("bearer ")
        || blob.contains("\"api_key\"")
        || blob.contains("\"password\"")
        || blob.contains("secret_access_key")
}

fn to_payload_map(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn budget_matches_event(budget: &AiBudgetLimit, event: &AiUsageEventV1) -> bool {
    if !budget.enabled || budget.tenant_id != event.tenant_id {
        return false;
    }
    match budget.scope_type.as_str() {
        "tenant" => true,
        "agent" => event.agent_id.as_deref() == Some(budget.scope_id.as_str()),
        "agent_type" => serde_json::to_string(&event.agent_type)
            .map(|value| value.trim_matches('"') == budget.scope_id.as_str())
            .unwrap_or(false),
        "provider" => event.provider.as_deref() == Some(budget.scope_id.as_str()),
        "model" => event.model.as_deref() == Some(budget.scope_id.as_str()),
        "task" => event.task_id.as_deref() == Some(budget.scope_id.as_str()),
        "session" => event.session_id.as_deref() == Some(budget.scope_id.as_str()),
        "surface" => event.surface == budget.scope_id,
        _ => false,
    }
}

fn window_start(window: &str) -> String {
    let now = Utc::now();
    let start = match window {
        "minute" => now - Duration::minutes(1),
        "hour" => now - Duration::hours(1),
        "month" => Utc
            .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
            .single()
            .unwrap_or(now),
        _ => Utc
            .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
            .single()
            .unwrap_or(now),
    };
    start.to_rfc3339()
}

fn internal_error(err: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
