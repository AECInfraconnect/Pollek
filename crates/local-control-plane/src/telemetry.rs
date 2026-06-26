// SPDX-License-Identifier: Apache-2.0
//! telemetry.rs — Local control-plane telemetry sink (L3).
//!
//! Accepts the SAME telemetry envelope the DEK sends to Pollen Cloud
//! (`TelemetryEventEnvelope`), on the SAME contract endpoints (R2 split), and
//! stores it in the local SQLite store. The Local Admin Dashboard reads decision
//! logs from here. Cutover Local->Cloud changes only the endpoint/trust — the
//! DEK's telemetry code is identical (invariant I1).
//!
//!   POST /v1/telemetry/events            (generic / sync / adapter health / os guardrail)
//!   POST /v1/telemetry/decision-logs     (DecisionLog)
//!   POST /v1/telemetry/security-events   (SecurityEvent)
//!   POST /v1/telemetry/traces            (trace spans)
//!   POST /v1/telemetry/ebpf-events       (OsGuardrailEvent / ebpf)
//!   POST /v1/metrics                     (RuntimeMetric)
//!   GET  /v1/tenants/:tenant/telemetry/decision-logs  (dashboard read)

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dek_agent_observer::{model::AgentObservationEvent, usage_model::AiUsageEventV1};
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/telemetry/events", post(ingest))
        .route("/v1/telemetry/decision-logs", post(ingest))
        .route("/v1/telemetry/security-events", post(ingest))
        .route("/v1/telemetry/traces", post(ingest))
        .route("/v1/telemetry/ebpf-events", post(ingest))
        .route("/v1/metrics", post(ingest))
        .route("/v1/telemetry/runtime-metrics", post(ingest))
        .route("/v1/telemetry/batches", post(ingest_batches))
        // tenant-scoped alias (DEK may post per-tenant)
        .route("/v1/tenants/:tenant/telemetry/events", post(ingest_tenant))
        // dashboard read-side
        .route(
            "/v1/tenants/:tenant/telemetry/decision-logs",
            get(list_decision_logs).delete(clear_decision_logs),
        )
        .route(
            "/v1/tenants/:tenant/logs/decisions",
            get(list_decision_logs).delete(clear_decision_logs),
        )
        .route(
            "/v1/tenants/:tenant/logs/tool-invocations",
            get(list_tool_invocations),
        )
        .route(
            "/v1/tenants/:tenant/logs/resource-access",
            get(list_resource_access),
        )
        .route(
            "/v1/tenants/:tenant/logs/policy-deployments",
            get(list_policy_deployments),
        )
        .route("/v1/tenants/:tenant/logs/pep-health", get(list_pep_health))
        .route(
            "/v1/tenants/:tenant/telemetry/export",
            get(export_telemetry),
        )
        .route("/v1/telemetry/observations", get(list_observations_v2))
        .route("/v1/telemetry/observations/stream", get(telemetry_stream))
        .route(
            "/v1/tenants/:tenant/telemetry/observations/stream",
            get(telemetry_stream),
        )
        .route(
            "/v1/tenants/:tenant/telemetry/resources/stream",
            get(telemetry_stream),
        )
        .route(
            "/v1/tenants/:tenant/telemetry/tools/stream",
            get(telemetry_stream),
        )
        .route(
            "/v1/tenants/:tenant/telemetry/identities/stream",
            get(telemetry_stream),
        )
        .route("/v1/telemetry/enforcement-status", get(enforcement_status))
        .route("/v1/decisions/:id/explain", get(explain_decision))
}

#[derive(serde::Serialize)]
pub struct ObservationPage {
    schema_version: String,
    items: Vec<pollen_contract::PollenTelemetryEnvelopeV1>,
    next_cursor: Option<String>,
}

#[derive(serde::Serialize)]
pub struct EnforcementStatusList {
    schema_version: String,
    items: Vec<pollen_contract::PollenTelemetryEnvelopeV1>,
}

async fn list_observations_v2(State(state): State<AppState>) -> impl IntoResponse {
    let mut items = Vec::new();
    if let Ok(records) = state.secure_spool.peek_recent(100) {
        for bytes in records {
            if let Ok(env) =
                serde_json::from_slice::<pollen_contract::PollenTelemetryEnvelopeV1>(&bytes)
            {
                if env.event_type == "agent_observation" {
                    items.push(env);
                }
            }
        }
    }

    (
        StatusCode::OK,
        Json(ObservationPage {
            schema_version: "observation-page.v1".to_string(),
            items,
            next_cursor: None,
        }),
    )
}

async fn enforcement_status(State(state): State<AppState>) -> impl IntoResponse {
    let mut items = Vec::new();
    if let Ok(records) = state.secure_spool.peek_recent(100) {
        for bytes in records {
            if let Ok(env) =
                serde_json::from_slice::<pollen_contract::PollenTelemetryEnvelopeV1>(&bytes)
            {
                if env.event_type == "enforcement_result" {
                    items.push(env);
                }
            }
        }
    }

    (
        StatusCode::OK,
        Json(EnforcementStatusList {
            schema_version: "enforcement-status.v1".to_string(),
            items,
        }),
    )
}

use axum::response::sse::{Event, Sse};
use futures_util::stream::Stream;
use futures_util::StreamExt;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;

async fn telemetry_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.telemetry_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| async move {
        match res {
            Ok(env) => {
                if let Ok(json_str) = serde_json::to_string(&env) {
                    Some(Ok(Event::default().data(json_str)))
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new())
}

async fn explain_decision(
    Path(id): Path<String>,
    State(_st): State<AppState>,
) -> impl IntoResponse {
    // In a full implementation, we'd query the telemetry store for the decision event
    // and return its `explanation` payload. For now, we return a mock explanation.
    // If the explanation isn't stored properly yet, we construct a fallback.
    let mock_explanation = dek_policy_runtime::explanation::DecisionExplanation {
        decision: "allow".into(),
        allow: true,
        pdp_engine: Some("cedar".into()),
        pdp_reason_th: "อนุญาตโดยนโยบายพื้นฐาน".into(),
        pep_plane: "McpStdio".into(),
        pep_capability: "Local".into(),
        pep_reason_th: "บังคับใช้ได้จริง".into(),
        enforced_for_real: true,
        success: true,
        status_badge: dek_policy_runtime::explanation::StatusBadge::Ok,
        user_action_th: None,
        correlation_id: id,
    };

    (StatusCode::OK, Json(mock_explanation))
}

#[derive(serde::Deserialize)]
pub struct ExportParams {
    pub format: Option<String>,
}

async fn export_telemetry(
    axum::extract::Path(tenant): axum::extract::Path<String>,
    axum::extract::Query(params): axum::extract::Query<ExportParams>,
    axum::extract::State(st): axum::extract::State<crate::state::AppState>,
) -> impl axum::response::IntoResponse {
    let format = params.format.unwrap_or_else(|| "json".into());
    let mut all_events = Vec::new();

    for kind in &[
        "decision",
        "tool_invocation",
        "resource_access",
        "policy_deployment",
        "agent_telemetry",
    ] {
        if let Ok(mut evs) = st.telemetry_store.list_telemetry(&tenant, kind).await {
            all_events.append(&mut evs);
        }
    }

    if format == "csv" {
        let mut csv = String::new();
        csv.push_str("timestamp,event_type,event_id,tenant_id,details\n");
        for ev in all_events {
            let ts = ev.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
            let etype = ev.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let eid = ev.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let details = ev.to_string().replace("\"", "\"\"");
            csv.push_str(&format!(
                "{},{},{},{},\"{}\"\n",
                ts, etype, eid, tenant, details
            ));
        }

        ([(axum::http::header::CONTENT_TYPE, "text/csv")], csv).into_response()
    } else {
        (
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            serde_json::to_string(&all_events).unwrap_or_else(|_| "[]".into()),
        )
            .into_response()
    }
}

#[derive(serde::Deserialize)]
pub struct TelemetryBatchRequest {
    pub tenant_id: String,
    pub events: Vec<serde_json::Value>,
}

#[derive(serde::Deserialize)]
pub struct TelemetryBatch {
    pub events: Vec<serde_json::Value>,
}

/// Reject payloads that carry unredacted secrets (defense in depth; the DEK
/// should already redact, but the sink must not persist leaked credentials).
fn has_unredacted_secret(ev: &serde_json::Value) -> bool {
    let blob = ev.to_string().to_lowercase();
    blob.contains("authorization:") || blob.contains("bearer ") || blob.contains("\"password\"")
}

async fn ingest_batches(
    State(st): State<AppState>,
    Json(batch): Json<TelemetryBatchRequest>,
) -> impl IntoResponse {
    store_events(&st, &batch.tenant_id, batch.events).await
}

async fn ingest(
    State(st): State<AppState>,
    Json(batch): Json<TelemetryBatch>,
) -> impl IntoResponse {
    store_events(&st, "local", batch.events).await
}

async fn ingest_tenant(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(batch): Json<TelemetryBatch>,
) -> impl IntoResponse {
    store_events(&st, &tenant, batch.events).await
}

async fn store_events(
    st: &AppState,
    tenant: &str,
    events: Vec<serde_json::Value>,
) -> (StatusCode, Json<serde_json::Value>) {
    let count = events.len();
    let mut stored = 0usize;
    for ev in events {
        if has_unredacted_secret(&ev) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "unredacted secret detected in telemetry payload" })),
            );
        }
        let val = ev.clone();
        // store keyed by event_id; object_type carries the event kind for filtering
        let kind = ev
            .get("event_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let event_id = ev
            .get("event_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                format!(
                    "ev_{}",
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                )
            });

        if st
            .telemetry_store
            .put_telemetry(tenant, &kind, &event_id, &val)
            .await
            .is_ok()
        {
            stored += 1;
            bridge_exact_usage_event(st, tenant, &kind, &val).await;
        }
    }
    (
        StatusCode::OK,
        Json(json!({
            "schema_version": "telemetry-ingest-response.v1",
            "accepted": stored as i32,
            "rejected": (count - stored) as i32
        })),
    )
}

async fn bridge_exact_usage_event(st: &AppState, tenant: &str, kind: &str, ev: &serde_json::Value) {
    let payload = ev.get("payload").cloned().unwrap_or_else(|| ev.clone());
    match kind {
        "ai_usage_event" => {
            if let Ok(mut usage) = serde_json::from_value::<AiUsageEventV1>(payload) {
                usage.metadata = crate::usage_api::merge_usage_metadata(
                    usage.metadata,
                    json!({
                        "capture_quality": if usage.tokens.estimated { "estimated_forwarded_usage" } else { "exact_forwarded_usage" },
                        "capture_source": "telemetry_ingest"
                    }),
                );
                let _ = crate::usage_api::persist_usage_event(st, tenant, usage).await;
            }
        }
        "agent_observation" => {
            if let Ok(obs) = serde_json::from_value::<AgentObservationEvent>(payload) {
                let _ = st.observability_store.insert_observation_event(&obs).await;
                if obs.token_usage.is_some() {
                    let mut usage =
                        AiUsageEventV1::from_legacy_observation(&obs, obs.provider.clone());
                    usage.metadata = crate::usage_api::merge_usage_metadata(
                        usage.metadata,
                        json!({
                            "capture_quality": "exact_agent_observation",
                            "capture_source": obs.pep_type.clone().unwrap_or_else(|| "telemetry_ingest".to_string())
                        }),
                    );
                    let _ = crate::usage_api::persist_usage_event(st, tenant, usage).await;
                }
            }
        }
        _ => {}
    }
}

/// Dashboard read-side: return DecisionLog events (newest first).
async fn list_decision_logs(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> impl IntoResponse {
    let mut items = vec![];
    if let Ok(mut logs) = st
        .telemetry_store
        .list_telemetry(&tenant, "decision_log")
        .await
    {
        items.append(&mut logs);
    }
    if let Ok(mut logs) = st.telemetry_store.list_telemetry(&tenant, "decision").await {
        items.append(&mut logs);
    }
    (
        StatusCode::OK,
        Json(json!({ "count": items.len(), "decisions": items })),
    )
}

async fn clear_decision_logs(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> impl IntoResponse {
    let _ = st
        .telemetry_store
        .clear_telemetry(&tenant, "decision_log")
        .await;
    let _ = st
        .telemetry_store
        .clear_telemetry(&tenant, "decision")
        .await;
    (StatusCode::OK, Json(json!({"status": "success"})))
}

async fn list_tool_invocations(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> impl IntoResponse {
    let logs = st
        .telemetry_store
        .list_telemetry(&tenant, "tool_invocation")
        .await
        .unwrap_or_default();
    (
        StatusCode::OK,
        Json(json!({ "count": logs.len(), "tool_invocations": logs })),
    )
}

async fn list_resource_access(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> impl IntoResponse {
    let logs = st
        .telemetry_store
        .list_telemetry(&tenant, "resource_access")
        .await
        .unwrap_or_default();
    (
        StatusCode::OK,
        Json(json!({ "count": logs.len(), "resource_accesses": logs })),
    )
}

async fn list_policy_deployments(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> impl IntoResponse {
    let logs = st
        .telemetry_store
        .list_telemetry(&tenant, "policy_deployment")
        .await
        .unwrap_or_default();
    (
        StatusCode::OK,
        Json(json!({ "count": logs.len(), "policy_deployments": logs })),
    )
}

async fn list_pep_health(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> impl IntoResponse {
    let logs = st
        .telemetry_store
        .list_telemetry(&tenant, "pep_binding_status")
        .await
        .unwrap_or_default();
    (
        StatusCode::OK,
        Json(json!({ "count": logs.len(), "pep_health": logs })),
    )
}
