// SPDX-License-Identifier: Apache-2.0

use crate::model::AgentObservationEvent;
use sha2::{Digest, Sha256};

pub fn apply_browser_scoped_agent_id(event: &mut AgentObservationEvent) {
    let should_fill = event
        .agent_id
        .as_deref()
        .map(|id| id.is_empty() || id == "unknown")
        .unwrap_or(true);

    if !should_fill {
        return;
    }

    let payload = serde_json::from_str::<serde_json::Value>(&event.payload_json).ok();
    let typed_scope = event
        .browser_scope
        .as_ref()
        .and_then(|scope| serde_json::to_value(scope).ok());

    if let Some(candidate_id) = typed_scope
        .as_ref()
        .and_then(|scope| find_string(scope, &["candidate_id", "discovery_candidate_id"]))
        .or_else(|| payload.as_ref().and_then(candidate_id_from_payload))
    {
        event.agent_id = Some(candidate_id.to_string());
        return;
    }

    if let Some(agent_id) = payload
        .as_ref()
        .and_then(|payload| find_string(payload, &["agent_id"]))
    {
        event.agent_id = Some(agent_id.to_string());
        return;
    }

    let Some(display_name) = typed_scope
        .as_ref()
        .and_then(scoped_display_name_from_payload)
        .or_else(|| payload.as_ref().and_then(scoped_display_name_from_payload))
    else {
        return;
    };
    event.agent_id = Some(candidate_id_for_display_name(
        &event.tenant_id,
        &display_name,
    ));
}

pub fn scoped_display_name_from_payload(payload: &serde_json::Value) -> Option<String> {
    let scope = browser_scope_from_payload(payload);

    let browser_name = find_string(scope, &["browser_name"])
        .or_else(|| find_string(payload, &["browser_name"]))
        .map(str::to_string)
        .or_else(|| {
            find_string(scope, &["browser_id"])
                .or_else(|| find_string(payload, &["browser_id"]))
                .map(browser_name_from_id)
        })?;

    let base_name = find_string(
        scope,
        &["base_name", "display_name", "name", "provider_name"],
    )
    .or_else(|| {
        find_string(
            payload,
            &["base_name", "display_name", "name", "provider_name"],
        )
    })?;

    Some(browser_scoped_name(base_name, &browser_name))
}

fn candidate_id_from_payload(payload: &serde_json::Value) -> Option<&str> {
    let scope = browser_scope_from_payload(payload);
    find_string(scope, &["candidate_id", "discovery_candidate_id"])
        .or_else(|| find_string(payload, &["candidate_id", "discovery_candidate_id"]))
}

fn browser_scope_from_payload(payload: &serde_json::Value) -> &serde_json::Value {
    payload
        .get("browser_scope")
        .or_else(|| payload.get("web_ai"))
        .unwrap_or(payload)
}

pub fn candidate_id_for_display_name(tenant: &str, display_name: &str) -> String {
    let basis = format!("name:{}", display_name.to_lowercase());
    let identity_key = format!("{:x}", Sha256::digest(basis.as_bytes()))[..24].to_string();
    let candidate_hash = Sha256::digest(format!("{tenant}:{identity_key}").as_bytes())
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    format!("cand_{candidate_hash}")
}

fn find_string<'a>(payload: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| payload.get(key).and_then(|v| v.as_str()))
        .filter(|value| !value.trim().is_empty())
}

fn browser_scoped_name(base_name: &str, browser_name: &str) -> String {
    let trimmed = base_name.trim();
    let browser_suffix = format!(" ({browser_name})");
    if trimmed.ends_with(&browser_suffix) {
        return trimmed.to_string();
    }

    let base = trimmed.strip_suffix(" (Web)").unwrap_or(trimmed).trim();
    format!("{base} ({browser_name})")
}

fn browser_name_from_id(browser_id: &str) -> String {
    match browser_id.to_ascii_lowercase().as_str() {
        "chrome" => "Chrome",
        "edge" | "msedge" => "Edge",
        "brave" => "Brave",
        "opera" => "Opera",
        "vivaldi" => "Vivaldi",
        "chromium" => "Chromium",
        "arc" => "Arc",
        "firefox" => "Firefox",
        "safari" => "Safari",
        _ => "Browser",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AgentObservationEvent, EventKind, TokenUsage};
    use serde_json::json;

    fn event(payload: serde_json::Value) -> AgentObservationEvent {
        AgentObservationEvent {
            process_signal: None,
            event_id: "evt".into(),
            tenant_id: "local".into(),
            trace_id: "trace".into(),
            agent_id: None,
            shadow_candidate_id: None,
            tool_id: None,
            resource_id: None,
            surface: "browser_extension".into(),
            action: "llm.call".into(),
            pep_type: Some("browser_extension".into()),
            risk_level: None,
            timestamp: "2026-06-26T00:00:00Z".into(),
            payload_json: payload.to_string(),
            token_usage: Some(TokenUsage {
                input_tokens: Some(1),
                output_tokens: Some(2),
                total_tokens: Some(3),
                model: Some("gpt-4o".into()),
            }),
            browser_scope: None,
            event_kind: EventKind::LlmCall,
            decision: None,
            tool_call: None,
            resource_access: None,
            latency_ms: None,
            provider: Some("openai".into()),
        }
    }

    #[test]
    fn derives_browser_scoped_candidate_ids_for_same_ai_in_different_browsers() {
        let mut chrome = event(json!({
            "web_ai": {
                "base_name": "ChatGPT (Web)",
                "browser_id": "chrome"
            }
        }));
        let mut edge = event(json!({
            "web_ai": {
                "base_name": "ChatGPT (Web)",
                "browser_id": "edge"
            }
        }));

        apply_browser_scoped_agent_id(&mut chrome);
        apply_browser_scoped_agent_id(&mut edge);

        let expected_chrome = candidate_id_for_display_name("local", "ChatGPT (Chrome)");
        let expected_edge = candidate_id_for_display_name("local", "ChatGPT (Edge)");

        assert_eq!(chrome.agent_id.as_deref(), Some(expected_chrome.as_str()));
        assert_eq!(edge.agent_id.as_deref(), Some(expected_edge.as_str()));
        assert_ne!(chrome.agent_id, edge.agent_id);
    }

    #[test]
    fn explicit_candidate_id_wins_for_policy_events() {
        let mut event = event(json!({
            "candidate_id": "cand_existing",
            "web_ai": {
                "base_name": "Claude (Web)",
                "browser_id": "firefox"
            }
        }));

        apply_browser_scoped_agent_id(&mut event);

        assert_eq!(event.agent_id.as_deref(), Some("cand_existing"));
    }

    #[test]
    fn top_level_browser_scope_contract_derives_same_candidate_id() {
        let mut event = event(json!({}));
        event.browser_scope = Some(crate::model::BrowserAiObservationScope {
            base_name: Some("Claude (Web)".into()),
            browser_id: Some("edge".into()),
            ..Default::default()
        });

        apply_browser_scoped_agent_id(&mut event);

        let expected = candidate_id_for_display_name("local", "Claude (Edge)");
        assert_eq!(event.agent_id.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn already_scoped_browser_name_is_not_duplicated() {
        let payload = json!({
            "display_name": "ChatGPT (Chrome)",
            "browser_name": "Chrome"
        });

        assert_eq!(
            scoped_display_name_from_payload(&payload).as_deref(),
            Some("ChatGPT (Chrome)")
        );
    }
}
