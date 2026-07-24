//! Value-shape helpers: derive a canonical agent id / type / provider from a
//! discovered candidate or a raw JSON event, pull typed fields out of nested
//! JSON, and classify access mode. Leaf functions over serde_json values.

use super::*;

pub(super) fn payload_or_self(value: Value) -> Value {
    value.get("payload").cloned().unwrap_or(value)
}

pub(super) fn canonical_agent_id(candidate: &DiscoveredAgentCandidateV2) -> String {
    if !candidate.suggested_registration.agent_id.is_empty() {
        candidate.suggested_registration.agent_id.clone()
    } else {
        candidate.candidate_id.clone()
    }
}

pub(super) fn candidate_collects_token_usage(candidate: &DiscoveredAgentCandidateV2) -> bool {
    candidate.suggested_observation_profile.collect_token_usage
        || candidate.capability_tags.iter().any(|tag| {
            matches!(
                tag.as_str(),
                "llm.call" | "llm.chat" | "web.chat" | "net.egress.llm" | "model.server"
            )
        })
}

pub(super) fn agent_type_for_candidate(candidate: &DiscoveredAgentCandidateV2) -> AgentType {
    match candidate.inferred_agent_type {
        InferredAgentType::WebAIApp | InferredAgentType::BrowserAgent => AgentType::BrowserAi,
        InferredAgentType::CliAgent => {
            let name = candidate.display_name.to_ascii_lowercase();
            if name.contains("claude") {
                AgentType::ClaudeCode
            } else if name.contains("codex") {
                AgentType::CodexCli
            } else {
                AgentType::CodingAgent
            }
        }
        InferredAgentType::McpClient => AgentType::McpClient,
        InferredAgentType::McpServer => AgentType::McpServerAgent,
        _ => AgentType::LocalAgent,
    }
}

pub(super) fn provider_for_candidate(candidate: &DiscoveredAgentCandidateV2) -> Option<String> {
    let joined = format!(
        "{} {} {}",
        candidate.display_name,
        candidate.vendor.clone().unwrap_or_default(),
        candidate.product.clone().unwrap_or_default()
    )
    .to_ascii_lowercase();
    if joined.contains("openai") || joined.contains("chatgpt") || joined.contains("codex") {
        Some("openai".into())
    } else if joined.contains("anthropic") || joined.contains("claude") {
        Some("anthropic".into())
    } else if joined.contains("google") || joined.contains("gemini") {
        Some("google".into())
    } else if joined.contains("deepseek") {
        Some("deepseek".into())
    } else if joined.contains("mistral") {
        Some("mistral".into())
    } else if joined.contains("ollama") {
        Some("ollama".into())
    } else {
        None
    }
}

pub(super) fn infer_provider(value: &Value) -> Option<String> {
    let text = [
        string_path(value, &["provider"]),
        string_path(value, &["host"]),
        string_path(value, &["model"]),
        string_path(value, &["modelVersion"]),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
    .to_ascii_lowercase();

    if text.contains("openai") || text.contains("gpt") || text.contains("chatgpt") {
        Some("openai".into())
    } else if text.contains("anthropic") || text.contains("claude") {
        Some("anthropic".into())
    } else if text.contains("google") || text.contains("gemini") {
        Some("google".into())
    } else if text.contains("deepseek") {
        Some("deepseek".into())
    } else if text.contains("mistral") || text.contains("mixtral") {
        Some("mistral".into())
    } else if text.contains("cohere") {
        Some("cohere".into())
    } else if value.get("prompt_eval_count").is_some() || value.get("eval_count").is_some() {
        Some("ollama".into())
    } else {
        None
    }
}

pub(super) fn host_for_provider(provider: &str) -> &'static str {
    match provider {
        "openai" => "api.openai.com",
        "anthropic" => "api.anthropic.com",
        "google" | "gemini" => "generativelanguage.googleapis.com",
        "deepseek" => "api.deepseek.com",
        "mistral" => "api.mistral.ai",
        "cohere" => "api.cohere.com",
        "ollama" => "127.0.0.1:11434",
        _ => "local",
    }
}

pub(super) fn usage_subtree(value: &Value) -> Value {
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

pub(super) fn timestamp_from_value(value: &Value) -> Option<DateTime<Utc>> {
    for key in ["occurred_at", "timestamp", "created_at", "time"] {
        if let Some(raw) = value.get(key).and_then(Value::as_str) {
            if let Ok(ts) = DateTime::parse_from_rfc3339(raw) {
                return Some(ts.with_timezone(&Utc));
            }
        }
    }
    None
}

pub(super) fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for part in path {
        current = current.get(*part)?;
    }
    current.as_str().map(str::to_string)
}

pub(super) fn string_any(value: &Value, keys: &[&str]) -> Option<String> {
    let map = value.as_object()?;
    for key in keys {
        if let Some(raw) = map.get(*key).and_then(Value::as_str) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

pub(super) fn agent_id_from_value(value: &Value) -> String {
    string_any(
        value,
        &[
            "agent_id",
            "agentId",
            "agent",
            "app",
            "process_name",
            "processName",
        ],
    )
    .unwrap_or_else(|| "unknown_agent".to_string())
}

pub(super) fn agent_label_from_value(value: &Value) -> String {
    string_any(
        value,
        &[
            "agent_label",
            "agentLabel",
            "agent_name",
            "agentName",
            "app_name",
            "process_name",
        ],
    )
    .unwrap_or_else(|| agent_id_from_value(value))
}

pub(super) fn looks_like_local_path(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() < 2 || trimmed.contains("://") {
        return false;
    }
    trimmed.contains(":\\")
        || trimmed.starts_with("\\\\")
        || trimmed.starts_with('/')
        || trimmed.contains('\\')
        || (trimmed.contains('/') && !looks_like_host(trimmed))
        || trimmed
            .rsplit(['\\', '/'])
            .next()
            .and_then(|leaf| leaf.rsplit_once('.'))
            .map(|(_, ext)| ext.len() <= 8 && ext.chars().all(|ch| ch.is_ascii_alphanumeric()))
            .unwrap_or(false)
}

pub(super) fn is_likely_folder_key(value: &Value) -> bool {
    value
        .as_object()
        .map(|map| {
            map.keys().any(|key| {
                let key = key.to_ascii_lowercase();
                key.contains("folder")
                    || key.contains("directory")
                    || key == "cwd"
                    || key.contains("workspace")
            })
        })
        .unwrap_or(false)
}

pub(super) fn mode_from_value(value: &Value) -> &'static str {
    let raw = string_any(
        value,
        &["mode", "access_mode", "action", "verb", "operation"],
    )
    .unwrap_or_default()
    .to_ascii_lowercase();
    if raw.contains("delete") || raw.contains("unlink") || raw.contains("remove") {
        "delete"
    } else if raw.contains("write")
        || raw.contains("save")
        || raw.contains("create")
        || raw.contains("update")
        || raw.contains("insert")
    {
        "write"
    } else if raw.contains("execute") || raw.contains("exec") || raw.contains("run") {
        "execute"
    } else if raw.contains("connect") {
        "connect"
    } else {
        "read"
    }
}
