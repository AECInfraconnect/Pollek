// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! telemetry.rs โ€” R3: full contract telemetry surface (ยง5).
//!
//! The DEK flusher (R2.1) POSTs typed telemetry to split endpoints. This mirrors
//! the Cloud contract so the same DEK code works against mock-cloud and the real
//! Cloud with only an endpoint/trust-store change.
//!
//!   POST /v1/telemetry/events          (generic / os_guardrail / os_lifecycle / audit)
//!   POST /v1/telemetry/decision-logs   (Decision)
//!   POST /v1/telemetry/security-events (Security)
//!   POST /v1/telemetry/traces          (Trace)
//!   POST /v1/telemetry/ebpf-events     (EbpfGuardrail)
//!   POST /v1/metrics                   (Metric)
//!
//! All typed events are ALSO mirrored into `telemetry_events` (the existing
//! buffer the UI/decision-logs view reads) so nothing regresses.

use crate::state::AppState;
use axum::{
    extract::Query,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dek_agent_observer::aggregate::{aggregate_identities, aggregate_resources, aggregate_tools};
use pollek_contract::{IdentityAccessPayload, ResourceAccessPayload, ToolUsagePayload};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/telemetry/events", post(ingest_events))
        .route("/v1/devices/:device_id/status", post(ingest_device_status))
        .route("/v1/telemetry/decision-logs", post(ingest_decision_logs))
        .route(
            "/v1/telemetry/security-events",
            post(ingest_security_events),
        )
        .route("/v1/telemetry/traces", post(ingest_traces))
        .route("/v1/telemetry/ebpf-events", post(ingest_ebpf_events))
        .route("/v1/metrics", post(ingest_metrics))
        .route("/v1/telemetry/batches", post(ingest_batches))
        .route("/v1/telemetry/resources", get(list_resources))
        .route("/v1/telemetry/tools", get(list_tools))
        .route("/v1/telemetry/identities", get(list_identities))
        .route("/v1/telemetry/observations", get(list_observations))
        // legacy/tenant-scoped alias kept for back-compat
        .route(
            "/v1/tenants/:tenant_id/telemetry/events",
            post(ingest_events_tenant),
        )
}

#[derive(serde::Deserialize)]
pub struct TelemetryPayload {
    pub events: Vec<serde_json::Value>,
}

fn validate_redaction(event_val: &serde_json::Value) -> Result<(), String> {
    if let Some(reason) = event_val.pointer("/reason").and_then(|r| r.as_str()) {
        let r = reason.to_lowercase();
        if r.contains("bearer") || r.contains("password") || r.contains("authorization:") {
            return Err("Unredacted secrets detected in telemetry payload".into());
        }
    }
    Ok(())
}

fn mirror_audit_event(state: &AppState, event_val: &serde_json::Value) {
    let is_audit = event_val
        .get("event_type")
        .and_then(|e| e.as_str())
        .map(|event_type| event_type.eq_ignore_ascii_case("audit"))
        .unwrap_or(false);
    if !is_audit {
        return;
    }

    if let Some(action) = event_val.pointer("/action").and_then(|a| a.as_str()) {
        let actor = event_val
            .pointer("/actor")
            .or_else(|| event_val.pointer("/device_id"))
            .and_then(|a| a.as_str())
            .unwrap_or("dek");
        let details = event_val.get("details").unwrap_or(event_val).to_string();
        state.audit_push(actor, action, &details);
    }
}

/// Shared ingest: redaction-check + store into the unified buffer. Returns the
/// count accepted, or an error if unredacted secrets are detected.
fn ingest(state: &AppState, events: Vec<serde_json::Value>, kind: &str) -> Result<usize, String> {
    let mut logs = state.telemetry_events.lock().unwrap(); //
    let mut n = 0;
    for event_val in events {
        validate_redaction(&event_val)?;
        mirror_audit_event(state, &event_val);
        logs.push_front(event_val);
        if logs.len() > 2000 {
            logs.pop_back();
        }
        n += 1;
    }
    drop(logs);
    state.audit_push(
        "dek",
        &format!("telemetry:{kind}"),
        &format!("{n} event(s)"),
    );
    Ok(n)
}

async fn handle(
    state: AppState,
    payload: TelemetryPayload,
    kind: &'static str,
) -> impl IntoResponse {
    let count = payload.events.len();
    match ingest(&state, payload.events, kind) {
        Ok(n) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "schema_version": "telemetry-ingest-response.v1",
                "accepted": n as i32,
                "rejected": (count - n) as i32
            })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        ),
    }
}

async fn ingest_events(
    State(s): State<AppState>,
    Json(p): Json<TelemetryPayload>,
) -> impl IntoResponse {
    handle(s, p, "events").await
}
async fn ingest_decision_logs(
    State(s): State<AppState>,
    Json(p): Json<TelemetryPayload>,
) -> impl IntoResponse {
    handle(s, p, "decision-logs").await
}
async fn ingest_security_events(
    State(s): State<AppState>,
    Json(p): Json<TelemetryPayload>,
) -> impl IntoResponse {
    handle(s, p, "security-events").await
}
async fn ingest_traces(
    State(s): State<AppState>,
    Json(p): Json<TelemetryPayload>,
) -> impl IntoResponse {
    handle(s, p, "traces").await
}
async fn ingest_ebpf_events(
    State(s): State<AppState>,
    Json(p): Json<TelemetryPayload>,
) -> impl IntoResponse {
    handle(s, p, "ebpf-events").await
}
async fn ingest_metrics(
    State(s): State<AppState>,
    Json(p): Json<TelemetryPayload>,
) -> impl IntoResponse {
    handle(s, p, "metrics").await
}
async fn ingest_events_tenant(
    axum::extract::Path(_tenant): axum::extract::Path<String>,
    State(s): State<AppState>,
    Json(p): Json<TelemetryPayload>,
) -> impl IntoResponse {
    handle(s, p, "events").await
}

#[derive(serde::Deserialize)]
pub struct TelemetryBatchRequest {
    pub schema_version: Option<String>,
    pub tenant_id: Option<String>,
    pub device_id: Option<String>,
    pub batch_id: Option<String>,
    #[serde(default)]
    pub events: Vec<serde_json::Value>,
    #[serde(default)]
    pub items: Vec<serde_json::Value>,
}

async fn ingest_batches(
    State(s): State<AppState>,
    Json(p): Json<TelemetryBatchRequest>,
) -> impl IntoResponse {
    let events = if !p.events.is_empty() {
        p.events
    } else {
        p.items
    };
    let count = events.len();
    match ingest(&s, events, "batches") {
        Ok(n) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "schema_version": "telemetry-ingest-response.v1",
                "accepted": n as i32,
                "rejected": (count - n) as i32
            })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        ),
    }
}

#[derive(serde::Deserialize)]
pub struct DeviceStatusPayload {
    pub device_id: String,
    pub bundle_id: String,
    pub status: String,
    #[serde(default)]
    pub capabilities: serde_json::Value,
    #[serde(default)]
    pub health: serde_json::Value,
    pub last_error: Option<String>,
}

async fn ingest_device_status(
    axum::extract::Path(_device_id): axum::extract::Path<String>,
    State(_s): State<AppState>,
    Json(p): Json<DeviceStatusPayload>,
) -> impl IntoResponse {
    // In a real implementation we would update the DeviceStatus in the database.
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "recorded",
            "device_id": p.device_id,
            "bundle_id": p.bundle_id
        })),
    )
}

#[derive(serde::Deserialize)]
pub struct ObservationsQuery {
    pub target_redacted: Option<String>,
    pub tool_id: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct InventoryQuery {
    pub agent_id: Option<String>,
    pub scope: Option<String>,
}

fn telemetry_payload(v: &serde_json::Value) -> serde_json::Value {
    v.get("payload")
        .cloned()
        .or_else(|| v.get("details").cloned())
        .unwrap_or_else(|| v.clone())
}

async fn list_observations(
    State(s): State<AppState>,
    Query(query): Query<ObservationsQuery>,
) -> impl IntoResponse {
    let logs = s
        .telemetry_events
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut filtered = Vec::new();
    for v in logs.iter() {
        if let Some(event_type) = v.get("event_type").and_then(|t| t.as_str()) {
            if event_type == "resource_access" {
                if let Some(target) = query.target_redacted.as_ref() {
                    let mut matched = false;
                    if let Some(redacted) = v
                        .pointer("/details/target_redacted")
                        .and_then(|t| t.as_str())
                    {
                        if target == redacted {
                            matched = true;
                        }
                    } else if let Some(redacted) = v.get("target_redacted").and_then(|t| t.as_str())
                    {
                        if target == redacted {
                            matched = true;
                        }
                    }
                    if matched {
                        filtered.push(v.clone());
                    }
                } else if query.tool_id.is_none() {
                    filtered.push(v.clone());
                }
            } else if event_type == "tool_usage" {
                if let Some(tool_id) = query.tool_id.as_ref() {
                    let mut matched = false;
                    if let Some(tid) = v.pointer("/details/tool_id").and_then(|t| t.as_str()) {
                        if tool_id == tid {
                            matched = true;
                        }
                    } else if let Some(tid) = v.get("tool_id").and_then(|t| t.as_str()) {
                        if tool_id == tid {
                            matched = true;
                        }
                    }
                    if matched {
                        filtered.push(v.clone());
                    }
                } else if query.target_redacted.is_none() {
                    filtered.push(v.clone());
                }
            }
        }
    }
    drop(logs);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "schema_version": "observations.v1",
            "items": filtered
        })),
    )
}

async fn list_resources(
    State(s): State<AppState>,
    Query(query): Query<InventoryQuery>,
) -> impl IntoResponse {
    let logs = s
        .telemetry_events
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut payloads = Vec::new();
    for v in logs.iter() {
        if let Some(event_type) = v.get("event_type").and_then(|t| t.as_str()) {
            if event_type == "resource_access" {
                let payload = telemetry_payload(v);
                if let Ok(p) = serde_json::from_value::<ResourceAccessPayload>(payload) {
                    if let Some(agent_id) = &query.agent_id {
                        if p.agent_id != *agent_id {
                            continue;
                        }
                    }
                    if let Some(scope) = &query.scope {
                        if p.scope.to_string() != *scope {
                            continue;
                        }
                    }
                    payloads.push(p);
                }
            }
        }
    }
    drop(logs);

    let items = aggregate_resources(&payloads);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "schema_version": "resource-inventory.v1",
            "items": items
        })),
    )
}

async fn list_tools(
    State(s): State<AppState>,
    Query(query): Query<InventoryQuery>,
) -> impl IntoResponse {
    let logs = s
        .telemetry_events
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut payloads = Vec::new();
    for v in logs.iter() {
        if let Some(event_type) = v.get("event_type").and_then(|t| t.as_str()) {
            if event_type == "tool_usage" {
                let payload = telemetry_payload(v);
                if let Ok(p) = serde_json::from_value::<ToolUsagePayload>(payload) {
                    if let Some(agent_id) = &query.agent_id {
                        if p.agent_id != *agent_id {
                            continue;
                        }
                    }
                    payloads.push(p);
                }
            }
        }
    }
    drop(logs);

    let items = aggregate_tools(&payloads);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "schema_version": "tool-inventory.v1",
            "items": items
        })),
    )
}

async fn list_identities(
    State(s): State<AppState>,
    Query(query): Query<InventoryQuery>,
) -> impl IntoResponse {
    let logs = s
        .telemetry_events
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut payloads = Vec::new();
    for v in logs.iter() {
        if let Some(event_type) = v.get("event_type").and_then(|t| t.as_str()) {
            if event_type == "identity_access" {
                let payload = telemetry_payload(v);
                if let Ok(p) = serde_json::from_value::<IdentityAccessPayload>(payload) {
                    if let Some(agent_id) = &query.agent_id {
                        if p.agent_id != *agent_id {
                            continue;
                        }
                    }
                    if let Some(scope) = &query.scope {
                        if p.scope.to_string() != *scope {
                            continue;
                        }
                    }
                    payloads.push(p);
                }
            }
        }
    }
    drop(logs);

    let items = aggregate_identities(&payloads);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "schema_version": "identity-inventory.v1",
            "items": items
        })),
    )
}
