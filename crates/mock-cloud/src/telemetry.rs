// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! telemetry.rs — R3: full contract telemetry surface (§5).
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
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use dek_domain_schema::TelemetryEvent;

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
        // legacy/tenant-scoped alias kept for back-compat
        .route(
            "/v1/tenants/:tenant_id/telemetry/events",
            post(ingest_events_tenant),
        )
}

#[derive(serde::Deserialize)]
pub struct TelemetryPayload {
    pub events: Vec<TelemetryEvent>,
}

/// Shared ingest: redaction-check + store into the unified buffer. Returns the
/// count accepted, or an error if unredacted secrets are detected.
fn ingest(state: &AppState, events: Vec<TelemetryEvent>, kind: &str) -> Result<usize, String> {
    let mut logs = state.telemetry_events.lock().unwrap();
    let mut n = 0;
    for event in events {
        // Redaction validation: assert no raw credentials leak into telemetry.
        if let TelemetryEvent::Decision { reason, .. } = &event {
            let r = reason.to_lowercase();
            if r.contains("bearer") || r.contains("password") || r.contains("authorization:") {
                return Err("Unredacted secrets detected in telemetry payload".into());
            }
        }
        if let TelemetryEvent::Audit {
            action, details, ..
        } = &event
        {
            state.audit_push(
                "dek",
                action,
                &serde_json::to_string(details).unwrap_or_default(),
            );
        }
        logs.push_front(event);
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
    match ingest(&state, payload.events, kind) {
        Ok(n) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ingested", "kind": kind, "count": n })),
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
    pub schema_version: String,
    pub tenant_id: String,
    pub device_id: String,
    pub batch_id: String,
    pub events: Vec<dek_domain_schema::TelemetryEvent>,
}

async fn ingest_batches(
    State(s): State<AppState>,
    Json(p): Json<TelemetryBatchRequest>,
) -> impl IntoResponse {
    let mut logs = s.telemetry_events.lock().unwrap();
    let mut n = 0;
    for event in p.events {
        logs.push_front(event);
        if logs.len() > 2000 {
            logs.pop_back();
        }
        n += 1;
    }
    drop(logs);

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "ok", "processed": n })),
    )
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
