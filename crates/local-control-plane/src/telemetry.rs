// SPDX-License-Identifier: Apache-2.0
//! telemetry.rs — Local control-plane telemetry sink (L3).
//!
//! Accepts the SAME telemetry envelope the DEK sends to Pollek Cloud
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
        .route(
            "/v1/tenants/:tenant/telemetry/guard-events/stream",
            get(telemetry_stream),
        )
        .route("/v1/telemetry/enforcement-status", get(enforcement_status))
        .route("/v1/decisions/:id/explain", get(explain_decision))
}

#[derive(serde::Serialize)]
pub struct ObservationPage {
    schema_version: String,
    items: Vec<pollek_contract::PollekTelemetryEnvelopeV1>,
    next_cursor: Option<String>,
}

#[derive(serde::Serialize)]
pub struct EnforcementStatusList {
    schema_version: String,
    items: Vec<pollek_contract::PollekTelemetryEnvelopeV1>,
}

async fn list_observations_v2(State(state): State<AppState>) -> impl IntoResponse {
    let mut items = Vec::new();
    if let Ok(records) = state.secure_spool.peek_recent(100) {
        for bytes in records {
            if let Ok(env) =
                serde_json::from_slice::<pollek_contract::PollekTelemetryEnvelopeV1>(&bytes)
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
                serde_json::from_slice::<pollek_contract::PollekTelemetryEnvelopeV1>(&bytes)
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

async fn explain_decision(Path(id): Path<String>, State(st): State<AppState>) -> impl IntoResponse {
    match find_decision_explanation(&st, &id).await {
        Ok(Some(explanation)) => (StatusCode::OK, Json(explanation)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "decision evidence not found",
                "decision_id": id
            })),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": error.to_string(),
                "decision_id": id
            })),
        )
            .into_response(),
    }
}

async fn find_decision_explanation(
    st: &AppState,
    decision_id: &str,
) -> anyhow::Result<Option<dek_policy_runtime::explanation::DecisionExplanation>> {
    for kind in ["decision_log", "decision"] {
        for event in st.telemetry_store.list_telemetry("local", kind).await? {
            if decision_event_matches(&event, decision_id) {
                if let Some(explanation) = event_to_decision_explanation(&event, decision_id) {
                    return Ok(Some(explanation));
                }
            }
        }
    }
    Ok(None)
}

fn decision_event_matches(event: &serde_json::Value, decision_id: &str) -> bool {
    ["event_id", "id", "correlation_id", "trace_id", "request_id"]
        .iter()
        .any(|field| event.get(*field).and_then(|value| value.as_str()) == Some(decision_id))
}

fn event_to_decision_explanation(
    event: &serde_json::Value,
    decision_id: &str,
) -> Option<dek_policy_runtime::explanation::DecisionExplanation> {
    if let Some(value) = event
        .get("explanation")
        .or_else(|| event.pointer("/payload/explanation"))
    {
        if let Ok(explanation) = serde_json::from_value(value.clone()) {
            return Some(explanation);
        }
    }

    let decision = event
        .get("decision")
        .or_else(|| event.pointer("/final_decision/decision"))
        .and_then(|value| value.as_str())
        .map(str::to_ascii_lowercase)?;
    let allow = event
        .get("allow")
        .and_then(|value| value.as_bool())
        .unwrap_or_else(|| decision == "allow" || decision == "allowed");
    let reason = first_string(
        event,
        &[
            "/reason",
            "/reason_code",
            "/final_decision/reason",
            "/metadata/reason",
        ],
    )
    .unwrap_or_else(|| "no reason captured in decision telemetry".to_string());
    let pep_plane =
        first_string(event, &["/pep_plane", "/pep_type"]).unwrap_or_else(|| "unknown".to_string());
    let enforced_for_real = event
        .get("enforced_for_real")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let status_badge = if allow && enforced_for_real {
        dek_policy_runtime::explanation::StatusBadge::Ok
    } else if allow {
        dek_policy_runtime::explanation::StatusBadge::Degraded
    } else {
        dek_policy_runtime::explanation::StatusBadge::Failed
    };

    Some(dek_policy_runtime::explanation::DecisionExplanation {
        decision,
        allow,
        pdp_engine: first_string(
            event,
            &[
                "/pdp_engine",
                "/selected_pdp",
                "/evaluator_id",
                "/metadata/pdp_engine",
            ],
        ),
        pdp_reason_th: reason.clone(),
        pep_plane,
        pep_capability: first_string(event, &["/pep_capability", "/pep_coverage"]).unwrap_or_else(
            || {
                if enforced_for_real {
                    "enforce".to_string()
                } else {
                    "observe".to_string()
                }
            },
        ),
        pep_reason_th: reason,
        enforced_for_real,
        success: allow,
        status_badge,
        user_action_th: first_string(event, &["/user_action_th", "/metadata/user_action_th"]),
        correlation_id: event
            .get("correlation_id")
            .or_else(|| event.get("event_id"))
            .and_then(|value| value.as_str())
            .unwrap_or(decision_id)
            .to_string(),
    })
}

fn first_string(event: &serde_json::Value, pointers: &[&str]) -> Option<String> {
    pointers.iter().find_map(|pointer| {
        event
            .pointer(pointer)
            .and_then(|value| value.as_str())
            .map(str::to_string)
    })
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
            if let Ok(envelope) =
                serde_json::from_value::<pollek_contract::PollekTelemetryEnvelopeV1>(val)
            {
                let _sent = st.telemetry_tx.send(envelope);
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_explanation_uses_embedded_evidence() {
        let event = json!({
            "event_id": "decision-1",
            "explanation": {
                "decision": "deny",
                "allow": false,
                "pdp_engine": "cedar",
                "pdp_reason_th": "policy denied",
                "pep_plane": "McpProxy",
                "pep_capability": "enforce",
                "pep_reason_th": "blocked by proxy",
                "enforced_for_real": true,
                "success": false,
                "status_badge": "failed",
                "user_action_th": null,
                "correlation_id": "decision-1"
            }
        });

        let explanation = event_to_decision_explanation(&event, "decision-1");

        assert!(matches!(
            explanation,
            Some(dek_policy_runtime::explanation::DecisionExplanation {
                allow: false,
                enforced_for_real: true,
                ..
            })
        ));
    }

    #[test]
    fn decision_explanation_falls_back_to_decision_fields_without_mocking() {
        let event = json!({
            "event_id": "decision-2",
            "decision": "allow",
            "reason": "matched allow policy",
            "pdp_engine": "local.cedar",
            "pep_type": "mcp_proxy",
            "enforced_for_real": true
        });

        let explanation = event_to_decision_explanation(&event, "decision-2");
        assert!(explanation.is_some());
        let Some(explanation) = explanation else {
            return;
        };

        assert_eq!(explanation.decision, "allow");
        assert!(explanation.allow);
        assert_eq!(explanation.pdp_engine.as_deref(), Some("local.cedar"));
        assert_eq!(explanation.pep_plane, "mcp_proxy");
        assert_eq!(explanation.pdp_reason_th, "matched allow policy");
        assert!(explanation.enforced_for_real);
    }

    #[test]
    fn decision_explanation_requires_decision_evidence() {
        let event = json!({
            "event_id": "decision-3",
            "reason": "metadata only"
        });

        assert!(event_to_decision_explanation(&event, "decision-3").is_none());
    }
}
