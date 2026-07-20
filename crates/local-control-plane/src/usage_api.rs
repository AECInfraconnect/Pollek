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
use dek_agent_observer::usage_cost::{
    calculate_cost_v2, CostCalculationInput, ModelPriceRuleV2, PriceCatalogV2,
};
use dek_agent_observer::usage_model::{
    AgentType, AiUsageEventKind, AiUsageEventV1, CanonicalCostBreakdown, CanonicalTokenUsage,
    CostSource, UsageSource,
};
use dek_agent_observer::usage_normalizer::{NormalizationContext, UsageNormalizer};
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
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
        .route(
            "/v1/tenants/:tenant/usage/provider-response",
            post(ingest_provider_response),
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
        .route(
            "/v1/tenants/:tenant/usage/credits",
            get(get_credit_ledger).put(put_credit_ledger),
        )
}

#[derive(Debug, Deserialize)]
struct UsageEventsBatch {
    events: Vec<AiUsageEventV1>,
}

#[derive(Debug, Deserialize)]
struct ProviderResponseUsageRequest {
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    host: Option<String>,
    #[serde(default)]
    provider_api: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    agent_type: Option<AgentType>,
    #[serde(default)]
    surface: Option<String>,
    #[serde(default)]
    pep_type: Option<String>,
    #[serde(default)]
    control_mode: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default)]
    invocation_id: Option<String>,
    #[serde(default)]
    resource_id: Option<String>,
    #[serde(default)]
    resource_type: Option<String>,
    #[serde(default)]
    trace_id: Option<String>,
    #[serde(default)]
    span_id: Option<String>,
    #[serde(default)]
    source: Option<String>,
    raw_response: Value,
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

async fn ingest_provider_response(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Json(req): Json<ProviderResponseUsageRequest>,
) -> impl IntoResponse {
    let event = match usage_event_from_provider_response(&state, &tenant, req) {
        Ok(event) => event,
        Err(message) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": message,
                    "hint": "Send a provider response object that contains an exact usage field, or send a canonical AiUsageEventV1 to /usage/events."
                })),
            );
        }
    };

    match persist_usage_event(&state, &tenant, event).await {
        Ok(event) => (StatusCode::CREATED, Json(json!({ "item": event }))),
        Err((status, message)) => (status, Json(json!({ "error": message }))),
    }
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

pub(crate) async fn persist_usage_event(
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
    let envelope = pollek_contract::PollekTelemetryEnvelopeV1 {
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
        let envelope = pollek_contract::PollekTelemetryEnvelopeV1 {
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

pub(crate) async fn publish_telemetry_envelope(
    state: &AppState,
    envelope: pollek_contract::PollekTelemetryEnvelopeV1,
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

async fn publish_envelope(
    state: &AppState,
    envelope: pollek_contract::PollekTelemetryEnvelopeV1,
) -> anyhow::Result<()> {
    publish_telemetry_envelope(state, envelope).await
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

fn usage_event_from_provider_response(
    state: &AppState,
    tenant: &str,
    req: ProviderResponseUsageRequest,
) -> Result<AiUsageEventV1, String> {
    let provider = req
        .provider
        .clone()
        .or_else(|| req.host.as_deref().and_then(provider_from_host))
        .or_else(|| infer_provider_from_response(&req.raw_response))
        .ok_or_else(|| {
            "provider or recognizable host is required for exact usage capture".to_string()
        })?;

    let trace_id = req
        .trace_id
        .clone()
        .unwrap_or_else(|| format!("trace_{}", uuid::Uuid::new_v4()));
    let span_id = req
        .span_id
        .clone()
        .unwrap_or_else(|| format!("span_{}", uuid::Uuid::new_v4()));
    let mut ctx = NormalizationContext::local(trace_id, span_id);
    ctx.tenant_id = tenant.to_string();
    ctx.workspace_id = Some(state.identity.workspace_id.clone());
    ctx.device_id = Some(local_device_id());
    ctx.agent_id = req.agent_id.clone();
    ctx.agent_type = req.agent_type.clone().unwrap_or(AgentType::Unknown);
    ctx.surface = req
        .surface
        .clone()
        .unwrap_or_else(|| "local_usage_capture".to_string());
    ctx.pep_type = req.pep_type.clone();
    ctx.control_mode = req.control_mode.clone();
    ctx.session_id = req.session_id.clone();
    ctx.task_id = req.task_id.clone();
    ctx.invocation_id = req.invocation_id.clone();

    let mut event = match provider.as_str() {
        "openai" | "azure-openai" | "deepseek" | "xai" | "groq" | "together" | "mistral"
        | "cohere" | "openrouter" | "perplexity" | "fireworks" | "cerebras" | "replicate"
        | "huggingface" => dek_agent_observer::providers::OpenAiUsageNormalizer
            .normalize(&req.raw_response, ctx.clone())
            .or_else(|_| usage_event_from_generic_usage_object(&provider, &req, ctx.clone())),
        "anthropic" => dek_agent_observer::providers::AnthropicUsageNormalizer
            .normalize(&req.raw_response, ctx.clone())
            .or_else(|_| usage_event_from_generic_usage_object(&provider, &req, ctx.clone())),
        "google" | "gemini" => dek_agent_observer::providers::GeminiUsageNormalizer
            .normalize(&req.raw_response, ctx.clone())
            .or_else(|_| usage_event_from_generic_usage_object("google", &req, ctx.clone())),
        "bedrock" | "aws-bedrock" => dek_agent_observer::providers::BedrockUsageNormalizer
            .normalize(&req.raw_response, ctx.clone())
            .or_else(|_| usage_event_from_generic_usage_object("bedrock", &req, ctx.clone())),
        "ollama" | "local" => usage_event_from_generic_usage_object(&provider, &req, ctx.clone()),
        _ => usage_event_from_generic_usage_object(&provider, &req, ctx.clone()),
    }
    .map_err(|err| err.to_string())?;

    event.provider = Some(provider);
    if event.provider_api.is_none() {
        event.provider_api = req.provider_api;
    }
    event.resource_id = req.resource_id;
    event.resource_type = req.resource_type;
    event.metadata = merge_usage_metadata(
        event.metadata,
        json!({
            "capture_quality": "exact_provider_response",
            "capture_source": req.source.unwrap_or_else(|| "local_provider_response".to_string()),
            "host": req.host,
            "plaintext_seen_by": event.pep_type.clone().unwrap_or_else(|| "approved_local_integration".to_string())
        }),
    );
    Ok(event.finalize())
}

fn usage_event_from_generic_usage_object(
    provider: &str,
    req: &ProviderResponseUsageRequest,
    ctx: NormalizationContext,
) -> Result<AiUsageEventV1, dek_agent_observer::usage_normalizer::UsageNormalizeError> {
    let host = req
        .host
        .clone()
        .unwrap_or_else(|| host_for_provider(provider).to_string());
    let (_parsed_provider, usage) =
        dek_agent_observer::egress_parser::parse_llm_usage(&host, &req.raw_response).ok_or_else(
            || dek_agent_observer::usage_normalizer::UsageNormalizeError::MissingUsage {
                provider: provider.to_string(),
            },
        )?;
    let tokens = CanonicalTokenUsage {
        input_tokens: usage.input_tokens.unwrap_or(0),
        output_tokens: usage.output_tokens.unwrap_or(0),
        total_tokens: usage
            .total_tokens
            .unwrap_or_else(|| usage.input_tokens.unwrap_or(0) + usage.output_tokens.unwrap_or(0)),
        estimated: false,
        source: UsageSource::ProviderResponse,
        ..CanonicalTokenUsage::default()
    };

    Ok(AiUsageEventV1 {
        schema_version: AiUsageEventV1::SCHEMA_VERSION.to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
        event_kind: AiUsageEventKind::ModelCallCompleted,
        occurred_at: Utc::now(),
        received_at: Utc::now(),
        tenant_id: ctx.tenant_id,
        workspace_id: ctx.workspace_id,
        device_id: ctx.device_id,
        actor_id_hash: ctx.actor_id_hash,
        actor_kind: ctx.actor_kind,
        trace_id: ctx.trace_id,
        span_id: ctx.span_id,
        parent_span_id: ctx.parent_span_id,
        session_id: ctx.session_id,
        task_id: ctx.task_id,
        agent_run_id: ctx.agent_run_id,
        agent_step_id: ctx.agent_step_id,
        invocation_id: ctx.invocation_id,
        agent_id: ctx.agent_id,
        agent_instance_id: None,
        agent_type: ctx.agent_type,
        parent_agent_id: ctx.parent_agent_id,
        subagent_id: ctx.subagent_id,
        shadow_candidate_id: ctx.shadow_candidate_id,
        provider: Some(provider.to_string()),
        provider_api: req.provider_api.clone(),
        provider_request_id: req
            .raw_response
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string),
        model: usage.model,
        model_version: req
            .raw_response
            .get("modelVersion")
            .and_then(Value::as_str)
            .map(str::to_string),
        service_tier: None,
        inference_region: None,
        surface: ctx.surface,
        pep_type: ctx.pep_type,
        control_mode: ctx.control_mode,
        policy_ids: ctx.policy_ids,
        tokens,
        cost: CanonicalCostBreakdown::default(),
        tool_id: None,
        tool_name: None,
        mcp_server_id: None,
        resource_id: None,
        resource_type: None,
        latency_ms: None,
        status: "ok".to_string(),
        error_code: None,
        provider_usage_raw: usage_subtree(&req.raw_response),
        metadata: json!({}),
        local_sequence: None,
        cloud_sync_status: Some("pending".to_string()),
        idempotency_key: String::new(),
    }
    .finalize())
}

fn local_device_id() -> String {
    let seed = format!(
        "{}:{}:{}",
        std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "local".into()),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest as _;
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();
    format!("dev_{}", hex::encode(&digest[..8]))
}

fn provider_from_host(host: &str) -> Option<String> {
    let host = host.to_ascii_lowercase();
    if host.contains("openai.azure.com") || host.contains("azure.com/openai") {
        Some("azure-openai".into())
    } else if host.contains("openai.com") || host.contains("chatgpt.com") {
        Some("openai".into())
    } else if host.contains("anthropic.com") || host.contains("claude.ai") {
        Some("anthropic".into())
    } else if host.contains("googleapis.com")
        || host.contains("gemini.google.com")
        || host.contains("aistudio.google.com")
    {
        Some("google".into())
    } else if host.contains("deepseek.com") {
        Some("deepseek".into())
    } else if host.contains("api.x.ai") || host.contains("x.ai") {
        Some("xai".into())
    } else if host.contains("api.groq.com") || host.contains("groq.com") {
        Some("groq".into())
    } else if host.contains("api.together.xyz") || host.contains("together.ai") {
        Some("together".into())
    } else if host.contains("mistral.ai") {
        Some("mistral".into())
    } else if host.contains("cohere.com") {
        Some("cohere".into())
    } else if host.contains("openrouter.ai") {
        Some("openrouter".into())
    } else if host.contains("perplexity.ai") {
        Some("perplexity".into())
    } else if host.contains("fireworks.ai") {
        Some("fireworks".into())
    } else if host.contains("cerebras.ai") {
        Some("cerebras".into())
    } else if host.contains("replicate.com") {
        Some("replicate".into())
    } else if host.contains("huggingface.co") {
        Some("huggingface".into())
    } else if host.contains("11434") || host.contains("ollama") {
        Some("ollama".into())
    } else {
        None
    }
}

fn infer_provider_from_response(value: &Value) -> Option<String> {
    let blob = value
        .get("model")
        .or_else(|| value.get("modelVersion"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if blob.contains("claude") {
        Some("anthropic".into())
    } else if blob.contains("gpt")
        || blob.contains("o1")
        || blob.contains("o3")
        || blob.contains("o4")
    {
        Some("openai".into())
    } else if blob.contains("gemini") {
        Some("google".into())
    } else if blob.contains("deepseek") {
        Some("deepseek".into())
    } else if blob.contains("grok") {
        Some("xai".into())
    } else if blob.contains("llama") && blob.contains("groq") {
        Some("groq".into())
    } else if blob.contains("sonar") {
        Some("perplexity".into())
    } else if blob.contains("mistral") || blob.contains("mixtral") {
        Some("mistral".into())
    } else if blob.contains("command") || blob.contains("embed-") {
        Some("cohere".into())
    } else {
        None
    }
}

fn host_for_provider(provider: &str) -> &'static str {
    match provider {
        "openai" => "api.openai.com",
        "azure-openai" => "openai.azure.com",
        "anthropic" => "api.anthropic.com",
        "google" | "gemini" => "generativelanguage.googleapis.com",
        "deepseek" => "api.deepseek.com",
        "xai" => "api.x.ai",
        "groq" => "api.groq.com",
        "together" => "api.together.xyz",
        "mistral" => "api.mistral.ai",
        "cohere" => "api.cohere.com",
        "openrouter" => "openrouter.ai",
        "perplexity" => "api.perplexity.ai",
        "fireworks" => "api.fireworks.ai",
        "cerebras" => "api.cerebras.ai",
        "replicate" => "api.replicate.com",
        "huggingface" => "router.huggingface.co",
        "ollama" => "127.0.0.1:11434",
        _ => "local",
    }
}

fn usage_subtree(value: &Value) -> Value {
    value
        .get("usage")
        .or_else(|| value.get("usageMetadata"))
        .or_else(|| value.get("message_delta").and_then(|m| m.get("usage")))
        .cloned()
        .unwrap_or_else(|| {
            let mut usage = Map::new();
            for key in ["prompt_eval_count", "eval_count", "total_duration"] {
                if let Some(v) = value.get(key) {
                    usage.insert(key.to_string(), v.clone());
                }
            }
            Value::Object(usage)
        })
}

pub(crate) fn merge_usage_metadata(existing: Value, extra: Value) -> Value {
    let mut map = match existing {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    if let Value::Object(extra) = extra {
        for (key, value) in extra {
            if !value.is_null() {
                map.insert(key, value);
            }
        }
    }
    Value::Object(map)
}

fn load_price_catalog_v2() -> Option<PriceCatalogV2> {
    let path = std::path::PathBuf::from("pollek-local-data/price_catalog.v2.json");
    if let Some(catalog) = std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
    {
        return Some(catalog);
    }

    if let Some(catalog) = load_legacy_price_catalog_v1() {
        return Some(catalog);
    }

    embedded_price_catalog()
}

/// Built-in price catalog used when no `pollek-local-data/price_catalog.*`
/// file exists on the device (fresh installs, or the process running from a
/// different working directory). Without this fallback every usage event kept
/// cost = $0.00 even though exact tokens had been captured. Prices are
/// estimated list prices per 1M tokens; a shipped catalog file always wins.
fn embedded_price_catalog() -> Option<PriceCatalogV2> {
    static CATALOG: std::sync::OnceLock<Option<PriceCatalogV2>> = std::sync::OnceLock::new();
    CATALOG
        .get_or_init(|| {
            serde_json::from_str(include_str!("../data/price_catalog.default.json")).ok()
        })
        .clone()
}

fn load_legacy_price_catalog_v1() -> Option<PriceCatalogV2> {
    let path = std::path::PathBuf::from("pollek-local-data/price_catalog.v1.json");
    let value: Value = std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())?;
    let providers = value.get("providers")?.as_object()?;
    let default_currency = value
        .get("currency")
        .and_then(Value::as_str)
        .unwrap_or("USD")
        .to_string();
    let catalog_version = value
        .get("catalog_version")
        .and_then(Value::as_str)
        .unwrap_or("legacy-v1")
        .to_string();
    let mut models = Vec::new();

    for (provider, provider_models) in providers {
        let Some(provider_models) = provider_models.as_object() else {
            continue;
        };
        for (model_match, price) in provider_models {
            let input = price
                .get("input_per_1m")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let output = price
                .get("output_per_1m")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let mut prices_per_1m = std::collections::BTreeMap::new();
            prices_per_1m.insert("input_tokens".to_string(), input);
            prices_per_1m.insert("output_tokens".to_string(), output);
            models.push(ModelPriceRuleV2 {
                provider: provider.clone(),
                provider_api: None,
                model_match: model_match.clone(),
                effective_from: None,
                effective_to: None,
                source_url: None,
                currency: Some(default_currency.clone()),
                prices_per_1m,
                tiers: Vec::new(),
            });
        }
    }

    Some(PriceCatalogV2 {
        schema_version: "price-catalog.v2".to_string(),
        catalog_version,
        default_currency,
        models,
    })
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

// ---------------------------------------------------------------------------
// Provider credit ledger
//
// Many teams pay AI providers in prepaid "credits" rather than watching a raw
// dollar figure. This is a real, local accounting layer: the user declares how
// much one credit is worth in the account currency (currency_per_credit) and,
// optionally, a prepaid balance. Pollek then derives credits consumed from the
// cost it already observes, so the Usage Bar "Credit" view and a remaining
// balance are honest arithmetic on real spend — never fabricated numbers.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderCreditConfigV1 {
    provider: String,
    /// Value of one credit in the account currency (e.g. 0.001 => $10 buys
    /// 10,000 credits). Must be > 0 to contribute credits.
    currency_per_credit: f64,
    /// Optional prepaid balance, in credits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    initial_credits: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CreditLedgerConfigV1 {
    #[serde(default = "credit_ledger_schema")]
    schema_version: String,
    #[serde(default = "default_currency")]
    currency: String,
    #[serde(default)]
    providers: Vec<ProviderCreditConfigV1>,
}

fn credit_ledger_schema() -> String {
    "pollek.credit_ledger.v1".to_string()
}

fn default_currency() -> String {
    "USD".to_string()
}

impl Default for CreditLedgerConfigV1 {
    fn default() -> Self {
        Self {
            schema_version: credit_ledger_schema(),
            currency: default_currency(),
            providers: Vec::new(),
        }
    }
}

fn credit_ledger_path() -> std::path::PathBuf {
    std::path::PathBuf::from("pollek-local-data/provider_credits.v1.json")
}

fn load_credit_ledger() -> CreditLedgerConfigV1 {
    std::fs::read_to_string(credit_ledger_path())
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

fn save_credit_ledger(config: &CreditLedgerConfigV1) -> std::io::Result<()> {
    let path = credit_ledger_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_string_pretty(config)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    std::fs::write(path, body)
}

/// Normalize provider keys so "OpenAI" configured by the user matches
/// "openai" observed on events.
fn credit_provider_key(provider: &str) -> String {
    provider.trim().to_ascii_lowercase()
}

/// Compute per-provider credit consumption from observed cost. Only providers
/// with a positive currency_per_credit contribute credits.
fn compute_credit_status(
    config: &CreditLedgerConfigV1,
    cost_by_provider: &[(String, f64)],
) -> Value {
    let cost_lookup: std::collections::HashMap<String, f64> = cost_by_provider
        .iter()
        .map(|(provider, cost)| (credit_provider_key(provider), *cost))
        .collect();

    let mut providers = Vec::new();
    let mut total_consumed = 0.0_f64;
    let mut total_remaining = 0.0_f64;
    let mut has_remaining = false;

    for entry in &config.providers {
        if entry.currency_per_credit <= 0.0 {
            continue;
        }
        let consumed_cost = cost_lookup
            .get(&credit_provider_key(&entry.provider))
            .copied()
            .unwrap_or(0.0);
        let consumed_credits = consumed_cost / entry.currency_per_credit;
        total_consumed += consumed_credits;
        let remaining_credits = entry
            .initial_credits
            .map(|initial| initial - consumed_credits);
        if let Some(remaining) = remaining_credits {
            total_remaining += remaining;
            has_remaining = true;
        }
        providers.push(json!({
            "provider": entry.provider,
            "label": entry.label,
            "currency_per_credit": entry.currency_per_credit,
            "initial_credits": entry.initial_credits,
            "consumed_cost": consumed_cost,
            "consumed_credits": consumed_credits,
            "remaining_credits": remaining_credits,
        }));
    }

    json!({
        "currency": config.currency,
        "providers": providers,
        "total_consumed_credits": total_consumed,
        "total_remaining_credits": if has_remaining { Some(total_remaining) } else { None },
    })
}

async fn get_credit_ledger(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Query(params): Query<UsageSummaryParams>,
) -> impl IntoResponse {
    let config = load_credit_ledger();
    let query = summary_query(tenant, params);
    let cost_by_provider = match state.observability_store.ai_usage_summary(query).await {
        Ok(summary) => summary
            .by_provider
            .into_iter()
            .map(|row| (row.label, row.total_cost))
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    let status = compute_credit_status(&config, &cost_by_provider);
    (StatusCode::OK, Json(json!({ "config": config, "status": status })))
}

async fn put_credit_ledger(
    Path(_tenant): Path<String>,
    Json(mut config): Json<CreditLedgerConfigV1>,
) -> impl IntoResponse {
    if config.schema_version.is_empty() {
        config.schema_version = credit_ledger_schema();
    }
    if config.currency.is_empty() {
        config.currency = default_currency();
    }
    match save_credit_ledger(&config) {
        Ok(()) => (StatusCode::OK, Json(json!({ "config": config }))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err.to_string() })),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_broader_provider_hosts() {
        for (host, expected) in [
            ("https://api.x.ai/v1/chat/completions", "xai"),
            ("https://api.groq.com/openai/v1/chat/completions", "groq"),
            ("https://api.together.xyz/v1/chat/completions", "together"),
            (
                "https://openrouter.ai/api/v1/chat/completions",
                "openrouter",
            ),
            ("https://api.perplexity.ai/chat/completions", "perplexity"),
            (
                "https://api.fireworks.ai/inference/v1/chat/completions",
                "fireworks",
            ),
            ("https://api.cerebras.ai/v1/chat/completions", "cerebras"),
            ("https://api.replicate.com/v1/predictions", "replicate"),
            (
                "https://router.huggingface.co/v1/chat/completions",
                "huggingface",
            ),
        ] {
            assert_eq!(
                provider_from_host(host).as_deref(),
                Some(expected),
                "{host}"
            );
        }
    }

    #[test]
    fn infers_common_model_provider_families() {
        for (model, expected) in [
            ("grok-4-latest", "xai"),
            ("sonar-pro", "perplexity"),
            ("mistral-large-latest", "mistral"),
            ("command-r-plus", "cohere"),
            ("gemini-2.5-pro", "google"),
            ("deepseek-chat", "deepseek"),
        ] {
            assert_eq!(
                infer_provider_from_response(&json!({ "model": model })).as_deref(),
                Some(expected),
                "{model}"
            );
        }
    }

    fn catalog_test_event(provider: &str, model: &str) -> AiUsageEventV1 {
        AiUsageEventV1 {
            schema_version: AiUsageEventV1::SCHEMA_VERSION.to_string(),
            event_id: "evt_test".to_string(),
            event_kind: AiUsageEventKind::ModelCallCompleted,
            occurred_at: Utc::now(),
            received_at: Utc::now(),
            tenant_id: "local".to_string(),
            workspace_id: None,
            device_id: None,
            actor_id_hash: None,
            actor_kind: None,
            trace_id: "trace_test".to_string(),
            span_id: "span_test".to_string(),
            parent_span_id: None,
            session_id: None,
            task_id: None,
            agent_run_id: None,
            agent_step_id: None,
            invocation_id: None,
            agent_id: None,
            agent_instance_id: None,
            agent_type: AgentType::Unknown,
            parent_agent_id: None,
            subagent_id: None,
            shadow_candidate_id: None,
            provider: Some(provider.to_string()),
            provider_api: None,
            provider_request_id: None,
            model: Some(model.to_string()),
            model_version: None,
            service_tier: None,
            inference_region: None,
            surface: "test".to_string(),
            pep_type: None,
            control_mode: None,
            policy_ids: vec![],
            tokens: CanonicalTokenUsage {
                input_tokens: 1_000_000,
                output_tokens: 100_000,
                total_tokens: 1_100_000,
                estimated: false,
                ..CanonicalTokenUsage::default()
            },
            cost: CanonicalCostBreakdown::default(),
            tool_id: None,
            tool_name: None,
            mcp_server_id: None,
            resource_id: None,
            resource_type: None,
            latency_ms: None,
            status: "ok".to_string(),
            error_code: None,
            provider_usage_raw: json!({}),
            metadata: json!({}),
            local_sequence: None,
            cloud_sync_status: None,
            idempotency_key: String::new(),
        }
    }

    #[test]
    fn embedded_price_catalog_prices_codex_usage_when_no_catalog_file_exists() {
        // Test cwd (the crate dir) has no pollek-local-data catalog, so this
        // exercises the embedded fallback that real fresh installs hit.
        let catalog = embedded_price_catalog();
        assert!(catalog.is_some(), "embedded price catalog must parse");
        let Some(catalog) = catalog else {
            return;
        };
        assert_eq!(catalog.schema_version, "pollek.price_catalog.v2");

        let mut event = catalog_test_event("openai", "gpt-5.1-codex");
        event = apply_cost_catalog(event);
        assert!(
            event.cost.total_cost > 0.0,
            "captured codex tokens must produce a nonzero cost, got {}",
            event.cost.total_cost
        );

        // Local engines are explicitly zero-cost (not unknown).
        let mut local = catalog_test_event("ollama", "llama3.2:latest");
        local = apply_cost_catalog(local);
        assert_eq!(local.cost.total_cost, 0.0);
        assert!(
            !matches!(
                local.cost.cost_source,
                dek_agent_observer::usage_model::CostSource::Unknown
            ),
            "local engine pricing should resolve via catalog, not stay unknown"
        );
    }

    #[test]
    fn credit_status_derives_consumed_and_remaining_from_observed_cost() {
        let config = CreditLedgerConfigV1 {
            schema_version: credit_ledger_schema(),
            currency: "USD".to_string(),
            providers: vec![
                ProviderCreditConfigV1 {
                    provider: "OpenAI".to_string(),
                    currency_per_credit: 0.001, // $10 => 10,000 credits
                    initial_credits: Some(10_000.0),
                    label: Some("OpenAI prepaid".to_string()),
                },
                ProviderCreditConfigV1 {
                    // No configured rate elsewhere => provider ignored.
                    provider: "anthropic".to_string(),
                    currency_per_credit: 0.0,
                    initial_credits: None,
                    label: None,
                },
            ],
        };
        // Observed cost is case-insensitively matched to configured providers.
        let cost = vec![
            ("openai".to_string(), 2.5_f64),
            ("anthropic".to_string(), 4.0_f64),
        ];
        let status = compute_credit_status(&config, &cost);

        let providers = status["providers"].as_array().unwrap();
        assert_eq!(providers.len(), 1, "zero-rate providers contribute nothing");
        let openai = &providers[0];
        assert_eq!(openai["provider"], "OpenAI");
        // $2.50 / $0.001 per credit = 2500 credits consumed.
        assert!((openai["consumed_credits"].as_f64().unwrap() - 2500.0).abs() < 1e-6);
        // 10,000 - 2,500 = 7,500 remaining.
        assert!((openai["remaining_credits"].as_f64().unwrap() - 7500.0).abs() < 1e-6);
        assert!((status["total_consumed_credits"].as_f64().unwrap() - 2500.0).abs() < 1e-6);
        assert!((status["total_remaining_credits"].as_f64().unwrap() - 7500.0).abs() < 1e-6);
    }

    #[test]
    fn credit_status_without_balance_reports_no_remaining_total() {
        let config = CreditLedgerConfigV1 {
            schema_version: credit_ledger_schema(),
            currency: "USD".to_string(),
            providers: vec![ProviderCreditConfigV1 {
                provider: "openrouter".to_string(),
                currency_per_credit: 1.0, // 1 credit == $1
                initial_credits: None,
                label: None,
            }],
        };
        let status = compute_credit_status(&config, &[("openrouter".to_string(), 3.0)]);
        assert!((status["total_consumed_credits"].as_f64().unwrap() - 3.0).abs() < 1e-6);
        assert!(status["total_remaining_credits"].is_null());
    }
}
