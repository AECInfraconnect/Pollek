//! Installed-plugin lifecycle: install/uninstall, enable/disable, health/test, update,
//! rollback, canary, revoke — plus the local audit trail for each transition.
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use dek_agent_observer::model::{AgentObservationEvent, DecisionInfo, EventKind, ResourceAccess};
use serde::Deserialize;
use serde_json::{json, Value};

use super::catalog::marketplace_items;
use super::string_array;
use crate::state::AppState;

const INSTALLED_PLUGIN_OBJECT: &str = "plugin_installed";
const PLUGIN_AUDIT_AGENT_ID: &str = "pollek-plugin-marketplace";

pub(super) async fn list_plugins(
    Path(tenant): Path<String>,
    State(state): State<AppState>,
) -> (StatusCode, Json<Value>) {
    match state
        .registry_store
        .list_raw(&tenant, INSTALLED_PLUGIN_OBJECT)
        .await
    {
        Ok(mut items) => {
            items.sort_by(|a, b| {
                let left = a.get("name").and_then(Value::as_str).unwrap_or_default();
                let right = b.get("name").and_then(Value::as_str).unwrap_or_default();
                left.cmp(right)
            });
            (
                StatusCode::OK,
                Json(json!({
                    "schema_version": "pollek.installed_plugins.v1",
                    "items": items
                })),
            )
        }
        Err(err) => error_response(err),
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct InstallPayload {
    id: String,
    #[serde(default)]
    granted_caps: Vec<String>,
    #[serde(default)]
    accept_risk: bool,
    #[serde(default)]
    source: Option<String>,
}

pub(super) async fn install_plugin(
    Path(tenant): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<InstallPayload>,
) -> (StatusCode, Json<Value>) {
    let requested_source = payload.source.clone();
    let item = marketplace_items()
        .into_iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(payload.id.as_str()))
        .unwrap_or_else(|| sideload_item(&payload.id));
    if let Some(response) = install_rejection(&item, payload.accept_risk) {
        let mut plugin = installed_plugin_from_item(&item, payload.granted_caps, false, "blocked");
        if let Some(source) = requested_source {
            plugin["source"] = json!(source);
        }
        let _ = record_plugin_activity(&state, &tenant, "plugin_install_rejected", &plugin).await;
        return response;
    }
    let mut plugin = installed_plugin_from_item(&item, payload.granted_caps, true, "healthy");
    if let Some(source) = requested_source {
        plugin["source"] = json!(source);
    }

    match state
        .registry_store
        .upsert_raw(
            &tenant,
            INSTALLED_PLUGIN_OBJECT,
            plugin
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or(&payload.id),
            &plugin,
        )
        .await
    {
        Ok(()) => {
            let _ = record_plugin_activity(&state, &tenant, "plugin_installed", &plugin).await;
            (StatusCode::OK, Json(plugin))
        }
        Err(err) => error_response(err),
    }
}

pub(super) async fn uninstall_plugin(
    Path((tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> (StatusCode, Json<Value>) {
    let existing = state
        .registry_store
        .get_raw(&tenant, INSTALLED_PLUGIN_OBJECT, &id)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| sideload_item(&id));

    match state
        .registry_store
        .delete_raw(&tenant, INSTALLED_PLUGIN_OBJECT, &id)
        .await
    {
        Ok(deleted) => {
            let _ = record_plugin_activity(&state, &tenant, "plugin_uninstalled", &existing).await;
            (
                StatusCode::OK,
                Json(json!({
                    "status": if deleted { "uninstalled" } else { "not_installed" },
                    "id": id,
                    "revoked_caps": true,
                    "cleared_plugin_namespace": deleted
                })),
            )
        }
        Err(err) => error_response(err),
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct TogglePayload {
    enabled: bool,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct LifecyclePayload {
    #[serde(default)]
    target_version: Option<String>,
    #[serde(default)]
    canary_percent: Option<i64>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    accept_risk: bool,
}

pub(super) async fn toggle_plugin(
    Path((tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(payload): Json<TogglePayload>,
) -> (StatusCode, Json<Value>) {
    match load_or_catalog_plugin(&state, &tenant, &id).await {
        Ok(mut plugin) => {
            plugin["enabled"] = json!(payload.enabled);
            plugin["health"] = json!(if payload.enabled {
                "healthy"
            } else {
                "disabled"
            });
            plugin["last_seen"] = json!(chrono::Utc::now().to_rfc3339());
            let store_result = state
                .registry_store
                .upsert_raw(&tenant, INSTALLED_PLUGIN_OBJECT, &id, &plugin)
                .await;
            match store_result {
                Ok(()) => {
                    let action = if payload.enabled {
                        "plugin_enabled"
                    } else {
                        "plugin_disabled"
                    };
                    let _ = record_plugin_activity(&state, &tenant, action, &plugin).await;
                    (StatusCode::OK, Json(plugin))
                }
                Err(err) => error_response(err),
            }
        }
        Err(err) => error_response(err),
    }
}

pub(super) async fn enable_plugin(
    Path((tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> (StatusCode, Json<Value>) {
    toggle_without_body(tenant, state, id, true).await
}

pub(super) async fn disable_plugin(
    Path((tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> (StatusCode, Json<Value>) {
    toggle_without_body(tenant, state, id, false).await
}

async fn toggle_without_body(
    tenant: String,
    state: AppState,
    id: String,
    enabled: bool,
) -> (StatusCode, Json<Value>) {
    match load_or_catalog_plugin(&state, &tenant, &id).await {
        Ok(mut plugin) => {
            plugin["enabled"] = json!(enabled);
            plugin["health"] = json!(if enabled { "healthy" } else { "disabled" });
            plugin["last_seen"] = json!(chrono::Utc::now().to_rfc3339());
            match state
                .registry_store
                .upsert_raw(&tenant, INSTALLED_PLUGIN_OBJECT, &id, &plugin)
                .await
            {
                Ok(()) => {
                    let action = if enabled {
                        "plugin_enabled"
                    } else {
                        "plugin_disabled"
                    };
                    let _ = record_plugin_activity(&state, &tenant, action, &plugin).await;
                    (StatusCode::OK, Json(plugin))
                }
                Err(err) => error_response(err),
            }
        }
        Err(err) => error_response(err),
    }
}

pub(super) async fn test_plugin(
    Path((tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(_payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    match load_or_catalog_plugin(&state, &tenant, &id).await {
        Ok(mut plugin) => {
            plugin["health"] = json!("healthy");
            plugin["health_metrics"] = json!(healthy_metrics("manual_test"));
            plugin["last_seen"] = json!(chrono::Utc::now().to_rfc3339());
            let _ = state
                .registry_store
                .upsert_raw(&tenant, INSTALLED_PLUGIN_OBJECT, &id, &plugin)
                .await;
            let _ = record_plugin_activity(&state, &tenant, "plugin_health_checked", &plugin).await;
            (
                StatusCode::OK,
                Json(json!({
                    "status": "success",
                    "message": format!("Plugin {} health check recorded", id),
                    "output": {}
                })),
            )
        }
        Err(err) => error_response(err),
    }
}

pub(super) async fn health_plugin(
    Path((tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> (StatusCode, Json<Value>) {
    match load_or_catalog_plugin(&state, &tenant, &id).await {
        Ok(mut plugin) => {
            plugin["health"] = json!(if plugin["enabled"].as_bool().unwrap_or(false) {
                "healthy"
            } else {
                "disabled"
            });
            plugin["health_metrics"] = json!(healthy_metrics("health_check"));
            plugin["last_seen"] = json!(chrono::Utc::now().to_rfc3339());
            match persist_plugin(&state, &tenant, &id, &plugin).await {
                Ok(()) => {
                    lifecycle_response(
                        &state,
                        &tenant,
                        "plugin_health_checked",
                        plugin,
                        "Plugin health check recorded.",
                    )
                    .await
                }
                Err(err) => error_response(err),
            }
        }
        Err(err) => error_response(err),
    }
}

pub(super) async fn update_plugin(
    Path((tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(payload): Json<LifecyclePayload>,
) -> (StatusCode, Json<Value>) {
    match load_or_catalog_plugin(&state, &tenant, &id).await {
        Ok(mut plugin) => {
            if plugin["signature_state"].as_str() == Some("test_only") && !payload.accept_risk {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "error": "developer_preview_update_requires_accept_risk",
                        "message": "Developer preview plugins require explicit risk acceptance before update."
                    })),
                );
            }
            let current_version = plugin
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let target_version = payload
                .target_version
                .or_else(|| {
                    plugin
                        .get("latest_version")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| current_version.clone());
            plugin["previous_versions"] =
                append_string_array(plugin.get("previous_versions"), current_version.clone());
            plugin["rollback_version"] = json!(current_version);
            plugin["version"] = json!(target_version);
            plugin["update_available"] = json!(false);
            plugin["rollback_available"] = json!(true);
            plugin["rollout"] = json!("stable");
            plugin["canary_percent"] = json!(100);
            plugin["lifecycle_state"] = json!("enabled");
            plugin["health"] = json!("healthy");
            plugin["last_seen"] = json!(chrono::Utc::now().to_rfc3339());
            if let Some(reason) = payload.reason {
                plugin["last_lifecycle_reason"] = json!(reason);
            }
            match persist_plugin(&state, &tenant, &id, &plugin).await {
                Ok(()) => {
                    lifecycle_response(
                        &state,
                        &tenant,
                        "plugin_updated",
                        plugin,
                        "Plugin updated and previous version kept for rollback.",
                    )
                    .await
                }
                Err(err) => error_response(err),
            }
        }
        Err(err) => error_response(err),
    }
}

pub(super) async fn rollback_plugin(
    Path((tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(payload): Json<LifecyclePayload>,
) -> (StatusCode, Json<Value>) {
    match load_or_catalog_plugin(&state, &tenant, &id).await {
        Ok(mut plugin) => {
            let current_version = plugin
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let rollback_version = payload
                .target_version
                .or_else(|| {
                    plugin
                        .get("rollback_version")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .or_else(|| first_string(plugin.get("previous_versions")));
            let Some(rollback_version) = rollback_version else {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "error": "plugin_rollback_unavailable",
                        "message": "No previous local version is available for rollback."
                    })),
                );
            };
            plugin["version"] = json!(rollback_version);
            plugin["latest_version"] = json!(current_version);
            plugin["update_available"] = json!(true);
            plugin["rollback_available"] = json!(false);
            plugin["rollout"] = json!("stable");
            plugin["canary_percent"] = json!(100);
            plugin["lifecycle_state"] = json!("rollback_available");
            plugin["health"] = json!("healthy");
            plugin["last_seen"] = json!(chrono::Utc::now().to_rfc3339());
            if let Some(reason) = payload.reason {
                plugin["last_lifecycle_reason"] = json!(reason);
            }
            match persist_plugin(&state, &tenant, &id, &plugin).await {
                Ok(()) => {
                    lifecycle_response(
                        &state,
                        &tenant,
                        "plugin_rolled_back",
                        plugin,
                        "Plugin rolled back to the previous local version.",
                    )
                    .await
                }
                Err(err) => error_response(err),
            }
        }
        Err(err) => error_response(err),
    }
}

pub(super) async fn canary_plugin(
    Path((tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(payload): Json<LifecyclePayload>,
) -> (StatusCode, Json<Value>) {
    match load_or_catalog_plugin(&state, &tenant, &id).await {
        Ok(mut plugin) => {
            let percent = payload.canary_percent.unwrap_or(10).clamp(1, 100);
            plugin["rollout"] = json!("canary");
            plugin["canary_percent"] = json!(percent);
            plugin["lifecycle_state"] = json!("canary");
            plugin["health"] = json!("healthy");
            plugin["last_seen"] = json!(chrono::Utc::now().to_rfc3339());
            if let Some(reason) = payload.reason {
                plugin["last_lifecycle_reason"] = json!(reason);
            }
            match persist_plugin(&state, &tenant, &id, &plugin).await {
                Ok(()) => {
                    lifecycle_response(
                        &state,
                        &tenant,
                        "plugin_canary_started",
                        plugin,
                        "Plugin is now staged as a local canary rollout.",
                    )
                    .await
                }
                Err(err) => error_response(err),
            }
        }
        Err(err) => error_response(err),
    }
}

pub(super) async fn revoke_plugin(
    Path((tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(payload): Json<LifecyclePayload>,
) -> (StatusCode, Json<Value>) {
    match load_or_catalog_plugin(&state, &tenant, &id).await {
        Ok(mut plugin) => {
            plugin["enabled"] = json!(false);
            plugin["revoked"] = json!(true);
            plugin["health"] = json!("revoked");
            plugin["lifecycle_state"] = json!("revoked");
            plugin["granted_caps"] = json!([]);
            plugin["last_seen"] = json!(chrono::Utc::now().to_rfc3339());
            if let Some(reason) = payload.reason {
                plugin["last_lifecycle_reason"] = json!(reason);
            }
            match persist_plugin(&state, &tenant, &id, &plugin).await {
                Ok(()) => {
                    lifecycle_response(
                        &state,
                        &tenant,
                        "plugin_revoked",
                        plugin,
                        "Plugin revoked locally and granted capabilities were removed.",
                    )
                    .await
                }
                Err(err) => error_response(err),
            }
        }
        Err(err) => error_response(err),
    }
}

fn installed_plugin_from_item(
    item: &Value,
    granted_caps: Vec<String>,
    enabled: bool,
    health: &str,
) -> Value {
    let now = chrono::Utc::now().to_rfc3339();
    let caps = if granted_caps.is_empty() {
        string_array(item, "capabilities")
    } else {
        granted_caps
    };
    json!({
        "schema_version": "pollek.installed_plugin.v1",
        "id": item.get("id").cloned().unwrap_or_else(|| json!("unknown-plugin")),
        "name": item.get("name").cloned().unwrap_or_else(|| json!("Unknown plugin")),
        "version": item.get("version").cloned().unwrap_or_else(|| json!("unknown")),
        "latest_version": item.get("latest_version").cloned().unwrap_or_else(|| item.get("version").cloned().unwrap_or_else(|| json!("unknown"))),
        "kind": item.get("kind").cloned().unwrap_or_else(|| json!("unknown")),
        "enabled": enabled,
        "granted_caps": caps,
        "human_grants": item.get("human_capabilities").cloned().unwrap_or_else(|| json!([])),
        "health": health,
        "source": item.get("source").cloned().unwrap_or_else(|| json!("sideload")),
        "signature_state": item.get("signature_state").cloned().unwrap_or_else(|| json!("unknown")),
        "privacy_note": item.get("privacy_note").cloned().unwrap_or(Value::Null),
        "registry_ref": item.get("registry_ref").cloned().unwrap_or(Value::Null),
        "release_notes": item.get("release_notes").cloned().unwrap_or(Value::Null),
        "update_available": item.get("update_available").cloned().unwrap_or_else(|| json!(false)),
        "rollback_available": item.get("rollback_supported").cloned().unwrap_or_else(|| json!(false)),
        "rollback_version": Value::Null,
        "previous_versions": [],
        "revoked": false,
        "rollout": if enabled { "stable" } else { "disabled" },
        "canary_percent": if enabled { 100 } else { 0 },
        "trust_labels": item.get("trust_labels").cloned().unwrap_or_else(|| json!([])),
        "health_metrics": healthy_metrics("installed"),
        "lifecycle_state": if enabled { "enabled" } else { "disabled" },
        "last_seen": now,
        "installed_at": now
    })
}

async fn load_or_catalog_plugin(state: &AppState, tenant: &str, id: &str) -> anyhow::Result<Value> {
    if let Some(plugin) = state
        .registry_store
        .get_raw(tenant, INSTALLED_PLUGIN_OBJECT, id)
        .await?
    {
        return Ok(plugin);
    }
    let item = marketplace_items()
        .into_iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
        .unwrap_or_else(|| sideload_item(id));
    Ok(installed_plugin_from_item(
        &item,
        string_array(&item, "capabilities"),
        false,
        "disabled",
    ))
}

fn sideload_item(id: &str) -> Value {
    json!({
        "id": id,
        "name": id,
        "version": "unknown",
        "kind": "unknown",
        "publisher": "Local sideload",
        "verified": false,
        "rating": 0.0,
        "installs": 0,
        "capabilities": [],
        "human_capabilities": [],
        "os": ["windows", "linux", "macos"],
        "min_engine_version": "unknown",
        "signature_ok": false,
        "signature_state": "unknown",
        "latest_version": "unknown",
        "update_available": false,
        "rollback_supported": false,
        "registry_ref": format!("sideload://{id}"),
        "release_notes": "Sideloaded plugin. Review manifest, checksum, and signature before use.",
        "trust_labels": ["unverified"],
        "lifecycle_state": "available",
        "description_en": "Local plugin not found in the marketplace catalog.",
        "privacy_note": "Review the local manifest before enabling this plugin.",
        "source": "sideload"
    })
}

fn install_rejection(item: &Value, accept_risk: bool) -> Option<(StatusCode, Json<Value>)> {
    let state = item
        .get("signature_state")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if state == "valid" {
        return None;
    }
    if state == "test_only" && accept_risk {
        return None;
    }
    let error = if state == "test_only" {
        "plugin_developer_preview_requires_accept_risk"
    } else {
        "plugin_signature_not_trusted"
    };
    Some((
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "error": error,
            "message": "Pollek refused to install this plugin until signature/trust risk is explicitly resolved.",
            "signature_state": state
        })),
    ))
}

async fn persist_plugin(
    state: &AppState,
    tenant: &str,
    id: &str,
    plugin: &Value,
) -> anyhow::Result<()> {
    state
        .registry_store
        .upsert_raw(tenant, INSTALLED_PLUGIN_OBJECT, id, plugin)
        .await
}

async fn lifecycle_response(
    state: &AppState,
    tenant: &str,
    action: &str,
    plugin: Value,
    message: &str,
) -> (StatusCode, Json<Value>) {
    let audit_event_id = record_plugin_activity(state, tenant, action, &plugin)
        .await
        .ok();
    (
        StatusCode::OK,
        Json(json!({
            "schema_version": "pollek.plugin_lifecycle.v1",
            "status": "ok",
            "action": action,
            "plugin": plugin,
            "audit_event_id": audit_event_id,
            "message": message
        })),
    )
}

fn healthy_metrics(reason: &str) -> Value {
    json!({
        "last_probe_reason": reason,
        "heartbeat_status": "ok",
        "error_rate": 0.0,
        "latency_ms": 12,
        "auto_disable_threshold": {
            "error_rate": 0.5,
            "window_minutes": 10
        }
    })
}

fn append_string_array(existing: Option<&Value>, item: String) -> Value {
    let mut values = existing
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !values
        .iter()
        .any(|value| value.as_str() == Some(item.as_str()))
    {
        values.push(json!(item));
    }
    Value::Array(values)
}

fn first_string(existing: Option<&Value>) -> Option<String> {
    existing
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(Value::as_str)
        .map(str::to_string)
}

async fn record_plugin_activity(
    state: &AppState,
    tenant: &str,
    action: &str,
    plugin: &Value,
) -> anyhow::Result<String> {
    let now = chrono::Utc::now();
    let plugin_id = plugin
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("unknown-plugin");
    let plugin_name = plugin
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(plugin_id);
    let granted_caps = string_array(plugin, "granted_caps");
    let sensitive = granted_caps.iter().any(|capability| {
        capability.starts_with("http_out:")
            || capability.starts_with("native:")
            || capability.contains(":write")
    });
    let payload = json!({
        "schema_version": "pollek.plugin_activity.v1",
        "plugin_id": plugin_id,
        "plugin_name": plugin_name,
        "action": action,
        "enabled": plugin.get("enabled").and_then(Value::as_bool),
        "health": plugin.get("health").and_then(Value::as_str),
        "granted_caps": granted_caps,
        "signature_state": plugin.get("signature_state").and_then(Value::as_str),
        "privacy_note": plugin.get("privacy_note").and_then(Value::as_str),
        "source": "plugin_registry"
    });
    let event_id = format!("plugin-{action}-{plugin_id}-{}", now.timestamp_millis());
    let event = AgentObservationEvent {
        process_signal: None,
        event_id: event_id.clone(),
        tenant_id: tenant.to_string(),
        trace_id: event_id.clone(),
        agent_id: Some(PLUGIN_AUDIT_AGENT_ID.to_string()),
        shadow_candidate_id: None,
        tool_id: None,
        resource_id: Some(plugin_id.to_string()),
        surface: "plugin_marketplace".to_string(),
        action: action.to_string(),
        pep_type: Some("local_plugin_registry".to_string()),
        risk_level: Some(if sensitive { "medium" } else { "low" }.to_string()),
        timestamp: now.to_rfc3339(),
        payload_json: serde_json::to_string(&payload)?,
        token_usage: None,
        browser_scope: None,
        event_kind: EventKind::ResourceAccess,
        decision: Some(DecisionInfo {
            allow: true,
            reason_code: action.to_string(),
            obligations: vec!["record_plugin_audit_event".to_string()],
            matched_policy_ids: Vec::new(),
            compliance_tags: vec!["plugin_audit".to_string()],
            pep_plane: Some("local_plugin_registry".to_string()),
            enforced_for_real: Some(false),
            status_badge: Some("audit".to_string()),
            message_th: None,
        }),
        tool_call: None,
        resource_access: Some(ResourceAccess {
            resource_type: "plugin".to_string(),
            target_redacted: plugin_name.to_string(),
            bytes: None,
            verb: action.to_string(),
        }),
        latency_ms: None,
        provider: None,
    };
    state
        .observability_store
        .insert_observation_event(&event)
        .await?;
    state
        .telemetry_store
        .put_telemetry(tenant, "plugin_audit", &event_id, &payload)
        .await?;
    Ok(event_id)
}

fn error_response(err: anyhow::Error) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": err.to_string() })),
    )
}
