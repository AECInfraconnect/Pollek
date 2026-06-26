// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::state::AppState;
use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/mock/admin/decision-logs", get(get_decision_logs_json))
        .route("/mock/admin/decision-logs/view", get(view_decision_logs))
}

pub async fn get_decision_logs_json(State(state): State<AppState>) -> impl IntoResponse {
    let events = state.telemetry_events.lock().unwrap(); //
    let decisions: Vec<_> = events
        .iter()
        .filter_map(|e| {
            if let Some(decision_obj) = e.get("Decision") {
                Some(serde_json::json!({
                    "timestamp": decision_obj.get("timestamp").and_then(|v| v.as_str()).unwrap_or(""),
                    "device_id": decision_obj.get("device_id").and_then(|v| v.as_str()).unwrap_or(""),
                    "principal": decision_obj.get("principal_id").and_then(|v| v.as_str()).unwrap_or(""),
                    "action": decision_obj.get("action").and_then(|v| v.as_str()).unwrap_or(""),
                    "resource": decision_obj.get("resource_id").and_then(|v| v.as_str()).unwrap_or(""),
                    "decision": decision_obj.get("decision").and_then(|v| v.as_str()).unwrap_or(""),
                    "reason": decision_obj.get("reason").and_then(|v| v.as_str()).unwrap_or(""),
                }))
            } else if e.get("event_type").and_then(|v| v.as_str()) == Some("enforcement_result") {
                let payload = e.get("payload");
                Some(serde_json::json!({
                    "timestamp": e.get("timestamp").and_then(|v| v.as_str()).unwrap_or(""),
                    "device_id": e.get("device_id").and_then(|v| v.as_str()).unwrap_or(""),
                    "principal": e.get("tenant_id").and_then(|v| v.as_str()).unwrap_or(""), // mapping roughly
                    "action": payload.and_then(|p| p.get("action")).and_then(|v| v.as_str()).unwrap_or(""),
                    "resource": payload.and_then(|p| p.get("resource")).and_then(|v| v.as_str()).unwrap_or(""),
                    "decision": payload.and_then(|p| p.get("decision")).and_then(|v| v.as_str()).unwrap_or(""),
                    "reason": payload.and_then(|p| p.get("reason")).and_then(|v| v.as_str()).unwrap_or(""),
                }))
            } else {
                None
            }
        })
        .collect();

    Json(decisions)
}

#[derive(Template)]
#[template(path = "decision_logs.html")]
struct DecisionLogsTemplate {
    logs: Vec<DecisionLogEntry>,
}

struct DecisionLogEntry {
    timestamp: String,
    device_id: String,
    principal: String,
    action: String,
    resource: String,
    decision: String,
    reason: String,
}

pub async fn view_decision_logs(State(state): State<AppState>) -> impl IntoResponse {
    let events = state.telemetry_events.lock().unwrap(); //
    let mut logs: Vec<DecisionLogEntry> = events
        .iter()
        .filter_map(|e| {
            if let Some(decision_obj) = e.get("Decision") {
                Some(DecisionLogEntry {
                    timestamp: decision_obj
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    device_id: decision_obj
                        .get("device_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    principal: decision_obj
                        .get("principal_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    action: decision_obj
                        .get("action")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    resource: decision_obj
                        .get("resource_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    decision: decision_obj
                        .get("decision")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    reason: decision_obj
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
            } else if e.get("event_type").and_then(|v| v.as_str()) == Some("enforcement_result") {
                let payload = e.get("payload");
                Some(DecisionLogEntry {
                    timestamp: e
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    device_id: e
                        .get("device_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    principal: e
                        .get("tenant_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    action: payload
                        .and_then(|p| p.get("action"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    resource: payload
                        .and_then(|p| p.get("resource"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    decision: payload
                        .and_then(|p| p.get("decision"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    reason: payload
                        .and_then(|p| p.get("reason"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    logs.reverse(); // Newest first

    let tpl = DecisionLogsTemplate { logs };
    Html(
        tpl.render()
            .unwrap_or_else(|e| format!("Template render error: {}", e)),
    )
}
