use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use dek_agent_observer::model::{
    AgentObservationEvent, BrowserAiObservationScope, DecisionInfo, EventKind, ResourceAccess,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::state::AppState;

const BROWSER_EXTENSION_STATUS_OBJECT: &str = "browser_extension_status";

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/browser-extension/status",
            get(browser_extension_status),
        )
        .route(
            "/v1/tenants/:tenant/browser-extension/events",
            post(ingest_browser_extension_event),
        )
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct BrowserExtensionObserveEvent {
    #[serde(default)]
    schema_version: Option<String>,
    event_type: String,
    #[serde(default)]
    extension_id: Option<String>,
    #[serde(default)]
    extension_version: Option<String>,
    #[serde(default)]
    browser_id: Option<String>,
    #[serde(default)]
    browser_name: Option<String>,
    #[serde(default)]
    provider_id: Option<String>,
    #[serde(default)]
    provider_label: Option<String>,
    #[serde(default)]
    tab_id: Option<i64>,
    #[serde(default)]
    window_id: Option<i64>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    occurred_at: Option<String>,
    #[serde(default)]
    text_length: Option<usize>,
    #[serde(default)]
    text_hash: Option<String>,
    #[serde(default)]
    response_length: Option<usize>,
    #[serde(default)]
    attachment_count: Option<usize>,
    #[serde(default)]
    attachment_extensions: Vec<String>,
    #[serde(default)]
    page_visibility: Option<String>,
    #[serde(default)]
    capture_mode: Option<String>,
    #[serde(default)]
    metadata: Map<String, Value>,
}

async fn browser_extension_status(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
) -> (StatusCode, Json<Value>) {
    let items = state
        .registry_store
        .list_raw(&tenant, BROWSER_EXTENSION_STATUS_OBJECT)
        .await
        .unwrap_or_default();
    (
        StatusCode::OK,
        Json(json!({
            "schema_version": "pollek.browser_extension.status.v1",
            "tenant_id": tenant,
            "items": items,
            "limitations": [
                "Browsers require user or enterprise approval before a local extension can run.",
                "The extension stores metadata only by default and does not persist raw prompt or response text.",
                "Server-side AI tool calls and provider billing details require wrapper, proxy, SDK, or provider integrations."
            ]
        })),
    )
}

async fn ingest_browser_extension_event(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Json(raw): Json<Value>,
) -> (StatusCode, Json<Value>) {
    if contains_forbidden_raw_text(&raw) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "raw_browser_text_not_accepted",
                "message": "Browser observe events must be metadata-only. Send prompt bodies through Prompt Guard check if explicitly enabled; Pollek will not persist raw text by default."
            })),
        );
    }

    let event: BrowserExtensionObserveEvent = match serde_json::from_value(raw) {
        Ok(event) => event,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "invalid_browser_extension_event",
                    "message": err.to_string()
                })),
            );
        }
    };

    let observation = build_observation_event(&tenant, &event);
    if let Err(err) = state
        .observability_store
        .insert_observation_event(&observation)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": err.to_string() })),
        );
    }

    let telemetry_result = publish_browser_telemetry(&state, &tenant, &event, &observation).await;
    let status_value = browser_status_value(&event);
    let status_id = format!(
        "{}:{}",
        event
            .browser_id
            .as_deref()
            .or(event.browser_name.as_deref())
            .unwrap_or("browser"),
        event.extension_id.as_deref().unwrap_or("pollek-extension")
    );
    let _ = state
        .registry_store
        .upsert_raw(
            &tenant,
            BROWSER_EXTENSION_STATUS_OBJECT,
            &status_id,
            &status_value,
        )
        .await;

    (
        StatusCode::CREATED,
        Json(json!({
            "schema_version": "pollek.browser_extension.ingest.v1",
            "status": "recorded",
            "event_id": observation.event_id,
            "telemetry_recorded": telemetry_result.is_ok(),
            "raw_prompt_or_response_stored": false,
            "capture_quality": capture_quality(&event)
        })),
    )
}

fn build_observation_event(
    tenant: &str,
    event: &BrowserExtensionObserveEvent,
) -> AgentObservationEvent {
    let now = Utc::now().to_rfc3339();
    let timestamp = event.occurred_at.clone().unwrap_or(now);
    let host = event.url.as_deref().and_then(url_host);
    let provider_label = event
        .provider_label
        .clone()
        .or_else(|| host.clone())
        .unwrap_or_else(|| "Browser AI".to_string());
    let event_id = stable_event_id(
        "browser_extension",
        &[
            tenant,
            event.event_type.as_str(),
            event.session_id.as_deref().unwrap_or("session"),
            event.text_hash.as_deref().unwrap_or("metadata"),
            timestamp.as_str(),
        ],
    );
    let action = action_for_event(&event.event_type).to_string();
    let payload = json!({
        "schema_version": "pollek.browser_observe_event.v1",
        "event_type": event.event_type,
        "extension_id": event.extension_id,
        "extension_version": event.extension_version,
        "browser_id": event.browser_id,
        "browser_name": event.browser_name,
        "provider_id": event.provider_id,
        "provider_label": provider_label,
        "tab_id": event.tab_id,
        "window_id": event.window_id,
        "url_host": host,
        "title_redacted": event.title.as_deref().map(redact_title),
        "session_id": event.session_id,
        "text_length": event.text_length,
        "text_hash": event.text_hash,
        "response_length": event.response_length,
        "attachment_count": event.attachment_count,
        "attachment_extensions": event.attachment_extensions,
        "page_visibility": event.page_visibility,
        "capture_mode": event.capture_mode,
        "capture_quality": capture_quality(event),
        "raw_prompt_or_response_stored": false,
        "metadata": event.metadata
    });
    AgentObservationEvent {
        process_signal: None,
        event_id: event_id.clone(),
        tenant_id: tenant.to_string(),
        trace_id: event.session_id.clone().unwrap_or_else(|| event_id.clone()),
        agent_id: event.provider_id.clone(),
        shadow_candidate_id: None,
        tool_id: None,
        resource_id: host.clone(),
        surface: "browser_extension".to_string(),
        action,
        pep_type: Some("browser_extension".to_string()),
        risk_level: Some(risk_for_event(event).to_string()),
        timestamp,
        payload_json: payload.to_string(),
        token_usage: None,
        browser_scope: Some(BrowserAiObservationScope {
            base_name: event.provider_label.clone().or(host.clone()),
            display_name: event.provider_label.clone(),
            browser_id: event.browser_id.clone(),
            browser_name: event.browser_name.clone(),
            candidate_id: event.provider_id.clone(),
            discovery_candidate_id: event.provider_id.clone(),
        }),
        event_kind: kind_for_event(&event.event_type),
        decision: Some(DecisionInfo {
            allow: true,
            reason_code: "browser_extension_observed_metadata".to_string(),
            obligations: vec!["metadata_only".to_string()],
            matched_policy_ids: Vec::new(),
            compliance_tags: vec!["browser_observe".to_string()],
            pep_plane: Some("browser_extension".to_string()),
            enforced_for_real: Some(false),
            status_badge: Some("watched_only".to_string()),
            message_th: None,
        }),
        tool_call: None,
        resource_access: Some(ResourceAccess {
            resource_type: "web_ai_session".to_string(),
            target_redacted: host.unwrap_or_else(|| "browser-tab".to_string()),
            bytes: None,
            verb: action_for_event(&event.event_type).to_string(),
        }),
        latency_ms: None,
        provider: provider_for_label(event.provider_label.as_deref()),
    }
}

async fn publish_browser_telemetry(
    state: &AppState,
    tenant: &str,
    event: &BrowserExtensionObserveEvent,
    observation: &AgentObservationEvent,
) -> anyhow::Result<()> {
    let payload =
        serde_json::from_str::<Value>(&observation.payload_json).unwrap_or_else(|_| json!({}));
    let envelope = pollek_contract::PollekTelemetryEnvelopeV1 {
        schema_version: "telemetry-envelope.v1".to_string(),
        event_id: observation.event_id.clone(),
        event_type: "browser_extension_observe".to_string(),
        timestamp: Utc::now(),
        tenant_id: tenant.to_string(),
        workspace_id: Some(state.identity.workspace_id.clone()),
        environment_id: Some(state.identity.environment_id.clone()),
        device_id: local_device_id(),
        trace_id: event.session_id.clone(),
        span_id: None,
        redaction_applied: true,
        payload: value_to_map(payload),
    };
    crate::usage_api::publish_telemetry_envelope(state, envelope).await
}

fn browser_status_value(event: &BrowserExtensionObserveEvent) -> Value {
    json!({
        "schema_version": "pollek.browser_extension.status_item.v1",
        "extension_id": event.extension_id,
        "extension_version": event.extension_version,
        "browser_id": event.browser_id,
        "browser_name": event.browser_name,
        "last_provider_id": event.provider_id,
        "last_provider_label": event.provider_label,
        "last_event_type": event.event_type,
        "last_seen": Utc::now().to_rfc3339(),
        "capture_mode": event.capture_mode,
        "raw_prompt_or_response_stored": false,
        "capabilities": [
            "tab_lifecycle_metadata",
            "prompt_submit_metadata",
            "attachment_metadata",
            "visible_response_metadata"
        ]
    })
}

fn contains_forbidden_raw_text(value: &Value) -> bool {
    match value {
        Value::Object(map) => map.iter().any(|(key, value)| {
            let key = key.to_ascii_lowercase();
            matches!(
                key.as_str(),
                "text" | "raw_text" | "prompt" | "response" | "completion" | "content"
            ) || contains_forbidden_raw_text(value)
        }),
        Value::Array(values) => values.iter().any(contains_forbidden_raw_text),
        _ => false,
    }
}

fn action_for_event(event_type: &str) -> &'static str {
    match event_type {
        "prompt_submitted" => "use",
        "attachment_detected" => "attach",
        "visible_response_metadata" => "read",
        "tab_visible" | "tab_loaded" => "connect",
        _ => "observe",
    }
}

fn kind_for_event(event_type: &str) -> EventKind {
    match event_type {
        "prompt_submitted" | "visible_response_metadata" => EventKind::LlmCall,
        _ => EventKind::ResourceAccess,
    }
}

fn risk_for_event(event: &BrowserExtensionObserveEvent) -> &'static str {
    if event.attachment_count.unwrap_or_default() > 0 {
        "medium"
    } else {
        "low"
    }
}

fn capture_quality(event: &BrowserExtensionObserveEvent) -> &'static str {
    match event.event_type.as_str() {
        "prompt_submitted" if event.text_hash.is_some() => "exact_prompt_metadata",
        "visible_response_metadata" => "response_metadata_only",
        "attachment_detected" => "attachment_metadata_only",
        _ => "tab_metadata_only",
    }
}

fn provider_for_label(label: Option<&str>) -> Option<String> {
    let label = label?.to_ascii_lowercase();
    if label.contains("chatgpt") || label.contains("openai") || label.contains("codex") {
        Some("openai".to_string())
    } else if label.contains("claude") || label.contains("anthropic") {
        Some("anthropic".to_string())
    } else if label.contains("gemini") || label.contains("google") || label.contains("antigravity")
    {
        Some("google".to_string())
    } else if label.contains("deepseek") {
        Some("deepseek".to_string())
    } else if label.contains("manus") {
        Some("manus".to_string())
    } else if label.contains("copilot") || label.contains("microsoft") {
        Some("microsoft".to_string())
    } else if label.contains("perplexity") {
        Some("perplexity".to_string())
    } else {
        None
    }
}

fn redact_title(title: &str) -> String {
    let trimmed = title.trim();
    if trimmed.chars().count() <= 80 {
        trimmed.to_string()
    } else {
        format!("{}...", trimmed.chars().take(80).collect::<String>())
    }
}

fn url_host(value: &str) -> Option<String> {
    value
        .split("://")
        .nth(1)
        .unwrap_or(value)
        .split('/')
        .next()
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .map(str::to_ascii_lowercase)
}

fn value_to_map(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn stable_event_id(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    for part in parts {
        hasher.update(b"|");
        hasher.update(part.as_bytes());
    }
    format!("{}_{}", prefix, hex::encode(&hasher.finalize()[..12]))
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
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    format!("dev_{}", hex::encode(&hasher.finalize()[..8]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_raw_prompt_text_keys() {
        assert!(contains_forbidden_raw_text(&json!({
            "event_type": "prompt_submitted",
            "prompt": "raw text"
        })));
        assert!(!contains_forbidden_raw_text(&json!({
            "event_type": "prompt_submitted",
            "text_length": 42,
            "text_hash": "sha256:abc"
        })));
    }

    #[test]
    fn browser_event_becomes_metadata_only_observation() {
        let event = BrowserExtensionObserveEvent {
            schema_version: Some("pollek.browser_observe_event.v1".into()),
            event_type: "prompt_submitted".into(),
            extension_id: Some("ext".into()),
            extension_version: Some("1.0.0".into()),
            browser_id: Some("edge".into()),
            browser_name: Some("Microsoft Edge".into()),
            provider_id: Some("chatgpt-browser".into()),
            provider_label: Some("ChatGPT".into()),
            tab_id: Some(1),
            window_id: Some(1),
            url: Some("https://chatgpt.com/c/abc".into()),
            title: Some("ChatGPT".into()),
            session_id: Some("s1".into()),
            occurred_at: Some("2026-06-29T00:00:00Z".into()),
            text_length: Some(120),
            text_hash: Some("sha256:abc".into()),
            response_length: None,
            attachment_count: None,
            attachment_extensions: vec![],
            page_visibility: Some("visible".into()),
            capture_mode: Some("observe".into()),
            metadata: Map::new(),
        };
        let observation = build_observation_event("local", &event);
        assert_eq!(observation.event_kind, EventKind::LlmCall);
        assert!(observation
            .payload_json
            .contains("raw_prompt_or_response_stored"));
        assert!(!observation.payload_json.contains("raw text"));
    }
}
