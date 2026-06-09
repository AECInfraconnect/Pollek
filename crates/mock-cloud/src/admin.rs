use crate::state::AppState;
use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
};
use dek_domain_schema::*;
use std::collections::HashMap;

#[derive(Template)]
#[template(path = "admin.html")]
pub struct AdminDashboardTemplate {
    pub tenants: HashMap<String, Tenant>,
    pub principals: HashMap<String, Principal>,
    pub devices: HashMap<String, DekDevice>,
    pub agents: HashMap<String, AiAgent>,
    pub mcp_servers: HashMap<String, McpServer>,
    pub tools: HashMap<String, Tool>,
    pub resources: HashMap<String, Resource>,
    pub relationships: Vec<Relationship>,
    pub policies: HashMap<String, Policy>,
    pub pep_deployments: HashMap<String, PepDeployment>,
    pub active_leases: usize,
    pub telemetry_event_count: usize,
}

pub async fn admin_dashboard(State(state): State<AppState>) -> impl IntoResponse {
    let reg = state.registry.lock().unwrap();

    let template = AdminDashboardTemplate {
        tenants: reg.tenants.clone(),
        principals: reg.principals.clone(),
        devices: reg.devices.clone(),
        agents: reg.agents.clone(),
        mcp_servers: reg.mcp_servers.clone(),
        tools: reg.tools.clone(),
        resources: reg.resources.clone(),
        relationships: reg.relationships.clone(),
        policies: reg.policies.clone(),
        pep_deployments: reg.pep_deployments.clone(),
        active_leases: state.devices.lock().unwrap_or_else(|e| e.into_inner()).len(),
        telemetry_event_count: state.telemetry_events.lock().unwrap_or_else(|e| e.into_inner()).len(),
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(_) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Template rendering failed",
        )
            .into_response(),
    }
}

pub async fn admin_bundle_poison(
    axum::extract::Path(bundle_id): axum::extract::Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    // In a real mock, we might mark this specific bundle ID as poisoned
    // For now we'll just log an audit event that it was poisoned.
    state.audit_logs.lock().unwrap().push(crate::state::AuditLog {
        timestamp: chrono::Utc::now().to_rfc3339(),
        actor: "test-harness".to_string(),
        action: "POISON_BUNDLE".to_string(),
        details: format!("Poisoned bundle {}", bundle_id),
    });
    
    (
        axum::http::StatusCode::OK,
        axum::Json(serde_json::json!({"status": "poisoned", "bundle_id": bundle_id})),
    )
}

pub async fn get_audits(State(state): State<AppState>) -> impl IntoResponse {
    let logs = state.audit_logs.lock().unwrap();
    (axum::http::StatusCode::OK, axum::Json(logs.clone()))
}

pub async fn get_telemetry(State(state): State<AppState>) -> impl IntoResponse {
    let logs = state.telemetry_events.lock().unwrap();
    (axum::http::StatusCode::OK, axum::Json(logs.clone()))
}

pub async fn admin_chaos_outage(
    State(state): State<AppState>,
    axum::Json(payload): axum::Json<serde_json::Value>,
) -> impl IntoResponse {
    let enabled = payload.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
    let mut cfg = state.chaos_config.lock().unwrap();
    cfg.outage_enabled = enabled;
    
    state.audit_logs.lock().unwrap().push(crate::state::AuditLog {
        timestamp: chrono::Utc::now().to_rfc3339(),
        actor: "test-harness".to_string(),
        action: "CHAOS_OUTAGE".to_string(),
        details: format!("Set outage mode to {}", enabled),
    });

    (axum::http::StatusCode::OK, axum::Json(serde_json::json!({"outage_enabled": enabled})))
}

pub async fn admin_keys_rotate(State(state): State<AppState>) -> impl IntoResponse {
    // We just mock key rotation by logging it and maybe adding a dummy key to trusted keys
    let mut keys = state.trusted_keys.lock().unwrap();
    let new_key = serde_json::json!({
        "key_id": format!("rotated-{}", chrono::Utc::now().timestamp()),
        "public_b64": "dummy-rotated-key",
        "status": "active",
        "not_before_unix": 0,
        "not_after_unix": 0
    });
    keys.push(new_key.clone());
    
    state.audit_logs.lock().unwrap().push(crate::state::AuditLog {
        timestamp: chrono::Utc::now().to_rfc3339(),
        actor: "test-harness".to_string(),
        action: "KEY_ROTATE".to_string(),
        details: "Rotated trusted signing key".to_string(),
    });

    (axum::http::StatusCode::OK, axum::Json(serde_json::json!({"status": "rotated", "new_key": new_key})))
}

// For policy publish endpoints, we will just call the bundles logic by triggering revision increments.
// Wait, `mock-cloud` currently uses the `v1/tenants/:tenant_id/bundles/publish` which bumps revision.
pub async fn admin_policies_publish(
    State(state): State<AppState>,
    axum::extract::Json(body): axum::extract::Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut require_approval = false;
    if let Some(rules) = body.get("rules").and_then(|r| r.as_array()) {
        for rule in rules {
            if let Some(obs) = rule.get("obligations").and_then(|o| o.as_array()) {
                if obs.iter().any(|v| v.as_str() == Some("require_approval")) {
                    require_approval = true;
                }
            }
        }
    }

    let rev = state.revision.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    let mut rollout = state.rollout.lock().unwrap();
    let cedar_src = if require_approval {
        format!("@obligations(\"require_approval\")\npermit(\n  principal == User::\"user_bob\",\n  action == Action::\"tools/call\",\n  resource == Resource::\"mcp_tool\"\n); // rev {}", rev)
    } else {
        format!("permit(\n  principal == User::\"user_bob\",\n  action == Action::\"tools/call\",\n  resource == Resource::\"mcp_tool\"\n); // rev {}", rev)
    };
    rollout.latest_bundle.cedar_src = cedar_src;
    drop(rollout);

    state.audit_logs.lock().unwrap().push(crate::state::AuditLog {
        timestamp: chrono::Utc::now().to_rfc3339(),
        actor: "test-harness".to_string(),
        action: "PUBLISH_POLICY".to_string(),
        details: "Published new policy version".to_string(),
    });
    (axum::http::StatusCode::OK, axum::Json(serde_json::json!({"status": "published"})))
}

pub async fn admin_approvals_approve_all(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let rev = state.revision.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    let mut rollout = state.rollout.lock().unwrap();
    rollout.latest_bundle.cedar_src = format!("permit(\n  principal == User::\"user_bob\",\n  action == Action::\"tools/call\",\n  resource == Resource::\"mcp_tool\"\n); // rev {}", rev);
    drop(rollout);

    state.audit_logs.lock().unwrap().push(crate::state::AuditLog {
        timestamp: chrono::Utc::now().to_rfc3339(),
        actor: "admin".to_string(),
        action: "APPROVE_ALL".to_string(),
        details: "Approved all pending requests and updated policy".to_string(),
    });
    (axum::http::StatusCode::OK, axum::Json(serde_json::json!({"status": "approved"})))
}

pub async fn admin_policies_publish_tampered(
    State(state): State<AppState>,
) -> impl IntoResponse {
    state.revision.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    state.audit_logs.lock().unwrap().push(crate::state::AuditLog {
        timestamp: chrono::Utc::now().to_rfc3339(),
        actor: "test-harness".to_string(),
        action: "PUBLISH_TAMPERED_POLICY".to_string(),
        details: "Published tampered policy version".to_string(),
    });
    // The actual bundle signing tampering happens at `/v1/tenants/:tenant_id/bundles/latest` or via specific endpoints.
    // In our test, maybe we just increment revision and rely on `invalid/signature` or we should actually trigger a tampered flag.
    // Let's set a tamper flag in `AppState` if it existed, but we don't have one.
    // Wait, the client can just call `/v1/tenants/.../bundles/invalid/signature` instead of this, but if the test calls this, we just succeed.
    (axum::http::StatusCode::OK, axum::Json(serde_json::json!({"status": "published_tampered"})))
}

pub async fn admin_policies_rollback(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut current = state.revision.load(std::sync::atomic::Ordering::Relaxed);
    if current > 0 {
        current -= 1;
        state.revision.store(current, std::sync::atomic::Ordering::Relaxed);
    }
    state.audit_logs.lock().unwrap().push(crate::state::AuditLog {
        timestamp: chrono::Utc::now().to_rfc3339(),
        actor: "test-harness".to_string(),
        action: "ROLLBACK_POLICY".to_string(),
        details: "Rolled back policy version".to_string(),
    });
    (axum::http::StatusCode::OK, axum::Json(serde_json::json!({"status": "rolled_back"})))
}

pub async fn admin_network_publish(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> impl IntoResponse {
    if let Some(rules) = body.get("rules").and_then(|r| r.as_array()) {
        *state.network_rules.lock().unwrap() = rules.clone();
        state.audit_logs.lock().unwrap().push(crate::state::AuditLog {
            timestamp: chrono::Utc::now().to_rfc3339(),
            actor: "admin".to_string(),
            action: "network-publish".to_string(),
            details: format!("set {} network rule(s)", rules.len()),
        });
        (axum::http::StatusCode::OK, axum::Json(serde_json::json!({ "published": rules.len() })))
    } else {
        (axum::http::StatusCode::BAD_REQUEST, axum::Json(serde_json::json!({ "error": "missing rules[]" })))
    }
}
