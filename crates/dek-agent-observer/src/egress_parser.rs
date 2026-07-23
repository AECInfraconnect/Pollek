// SPDX-License-Identifier: Apache-2.0

use crate::model::{AgentObservationEvent, EventKind, TokenUsage};

fn provider_from_host(host: &str) -> String {
    let host = host.to_ascii_lowercase();

    if host.contains("api.openai.com") {
        "openai".into()
    } else if host.contains("api.anthropic.com") {
        "anthropic".into()
    } else if host.contains("generativelanguage.googleapis.com")
        || host.contains("aiplatform.googleapis.com")
    {
        "google".into()
    } else if host.contains("api.mistral.ai") {
        "mistral".into()
    } else if host.contains("api.deepseek.com") {
        "deepseek".into()
    } else if host.contains("api.x.ai") {
        "xai".into()
    } else if host.contains("api.groq.com") {
        "groq".into()
    } else if host.contains("api.together.xyz") {
        "together".into()
    } else if host.contains("api.perplexity.ai") || host.contains("sonar") {
        "perplexity".into()
    } else if host.contains("api.fireworks.ai") {
        "fireworks".into()
    } else if host.contains("api.cerebras.ai") {
        "cerebras".into()
    } else if host.contains("api.replicate.com") {
        "replicate".into()
    } else if host.contains("api-inference.huggingface.co")
        || host.contains("router.huggingface.co")
    {
        "huggingface".into()
    } else if host.contains("api.cohere.com") {
        "cohere".into()
    } else if host.contains("openrouter.ai") {
        "openrouter".into()
    } else if host.contains("11434") {
        "ollama".into()
    } else {
        "local".into()
    }
}

fn token_i64(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|v| v.as_i64())
}

fn nested_token_i64(value: &serde_json::Value, parent: &str, key: &str) -> Option<i64> {
    value.get(parent).and_then(|v| token_i64(v, key))
}

fn has_ollama_usage(body: &serde_json::Value) -> bool {
    body.get("prompt_eval_count").is_some() || body.get("eval_count").is_some()
}

fn usage_object(body: &serde_json::Value) -> Option<&serde_json::Value> {
    body.get("usage")
        .or_else(|| body.get("usageMetadata"))
        .or_else(|| body.get("message_delta").and_then(|m| m.get("usage")))
        .or_else(|| has_ollama_usage(body).then_some(body))
}

/// Detect provider and parse token usage from common LLM response schemas.
pub fn parse_llm_usage(host: &str, body: &serde_json::Value) -> Option<(String, TokenUsage)> {
    let provider = provider_from_host(host);
    let model = body
        .get("model")
        .or_else(|| body.get("modelVersion"))
        .and_then(|m| m.as_str())
        .map(String::from);

    let usage = usage_object(body)?;

    let input = token_i64(usage, "prompt_tokens")
        .or_else(|| token_i64(usage, "input_tokens"))
        .or_else(|| token_i64(usage, "promptTokenCount"))
        .or_else(|| token_i64(usage, "prompt_eval_count"))
        .or_else(|| nested_token_i64(usage, "tokens", "input_tokens"))
        .or_else(|| nested_token_i64(usage, "billed_units", "input_tokens"));

    let output = token_i64(usage, "completion_tokens")
        .or_else(|| token_i64(usage, "output_tokens"))
        .or_else(|| token_i64(usage, "candidatesTokenCount"))
        .or_else(|| token_i64(usage, "eval_count"))
        .or_else(|| nested_token_i64(usage, "tokens", "output_tokens"))
        .or_else(|| nested_token_i64(usage, "billed_units", "output_tokens"));

    let total = token_i64(usage, "total_tokens")
        .or_else(|| token_i64(usage, "totalTokenCount"))
        .or_else(|| input.zip(output).map(|(i, o)| i + o))
        .or_else(|| Some(input.unwrap_or(0) + output.unwrap_or(0)));

    Some((
        provider,
        TokenUsage {
            input_tokens: input,
            output_tokens: output,
            total_tokens: total,
            model,
        },
    ))
}

/// Build an observation event from one observed LLM egress response.
pub fn llm_call_event(
    tenant: &str,
    trace_id: &str,
    agent_id: Option<String>,
    host: &str,
    body: &serde_json::Value,
    latency_ms: i64,
) -> Option<AgentObservationEvent> {
    let (provider, usage) = parse_llm_usage(host, body)?;
    Some(AgentObservationEvent {
        process_signal: None,
        event_id: uuid::Uuid::new_v4().to_string(),
        tenant_id: tenant.into(),
        trace_id: trace_id.into(),
        agent_id,
        shadow_candidate_id: None,
        tool_id: None,
        resource_id: None,
        surface: "llm_egress".into(),
        action: "chat.completion".into(),
        pep_type: Some("network_egress".into()),
        risk_level: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
        payload_json: "{}".into(),
        token_usage: Some(usage),
        browser_scope: None,
        event_kind: EventKind::LlmCall,
        decision: None,
        tool_call: None,
        resource_access: None,
        latency_ms: Some(latency_ms),
        provider: Some(provider),
    })
}

/// Classify an egress destination using cloud_resource_signatures definition
pub fn classify_cloud_egress(host: &str) -> Option<(String, String)> {
    let baseline = dek_fingerprint_defs::load_latest_baseline();
    for sig in &baseline.cloud_resource_signatures {
        if host.contains(&sig.host_pattern) {
            return Some((sig.kind.clone(), sig.name.clone()));
        }
    }
    // Fallbacks
    if host.contains("api.openai.com") || host.contains("api.anthropic.com") {
        Some(("api".to_string(), "LLM API".to_string()))
    } else if host.contains("drive.google.com") {
        Some(("cloud_drive".to_string(), "Google Drive".to_string()))
    } else if host.contains("smtp") || host.contains("imap") {
        Some(("email".to_string(), "Email Service".to_string()))
    } else if host.contains("github.com") {
        Some(("saas".to_string(), "GitHub".to_string()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_openai_compatible_usage() -> Result<(), String> {
        let (_, usage) = parse_llm_usage(
            "api.deepseek.com",
            &json!({
                "model": "deepseek-chat",
                "usage": {
                    "prompt_tokens": 12,
                    "completion_tokens": 8,
                    "total_tokens": 20
                }
            }),
        )
        .ok_or("usage".to_string())?;

        assert_eq!(usage.model.as_deref(), Some("deepseek-chat"));
        assert_eq!(usage.input_tokens, Some(12));
        assert_eq!(usage.output_tokens, Some(8));
        assert_eq!(usage.total_tokens, Some(20));
        Ok(())
    }

    #[test]
    fn parses_anthropic_usage() -> Result<(), String> {
        let (provider, usage) = parse_llm_usage(
            "api.anthropic.com",
            &json!({
                "model": "claude-sonnet-4-5",
                "usage": {
                    "input_tokens": 31,
                    "output_tokens": 9
                }
            }),
        )
        .ok_or("usage".to_string())?;

        assert_eq!(provider, "anthropic");
        assert_eq!(usage.input_tokens, Some(31));
        assert_eq!(usage.output_tokens, Some(9));
        assert_eq!(usage.total_tokens, Some(40));
        Ok(())
    }

    #[test]
    fn parses_gemini_usage_metadata() -> Result<(), String> {
        let (provider, usage) = parse_llm_usage(
            "generativelanguage.googleapis.com",
            &json!({
                "modelVersion": "gemini-2.5-pro",
                "usageMetadata": {
                    "promptTokenCount": 17,
                    "candidatesTokenCount": 23,
                    "totalTokenCount": 40
                }
            }),
        )
        .ok_or("usage".to_string())?;

        assert_eq!(provider, "google");
        assert_eq!(usage.model.as_deref(), Some("gemini-2.5-pro"));
        assert_eq!(usage.input_tokens, Some(17));
        assert_eq!(usage.output_tokens, Some(23));
        assert_eq!(usage.total_tokens, Some(40));
        Ok(())
    }

    #[test]
    fn parses_ollama_final_response_counters() -> Result<(), String> {
        let (provider, usage) = parse_llm_usage(
            "127.0.0.1:11434",
            &json!({
                "model": "llama3.1",
                "prompt_eval_count": 44,
                "eval_count": 11
            }),
        )
        .ok_or("usage".to_string())?;

        assert_eq!(provider, "ollama");
        assert_eq!(usage.input_tokens, Some(44));
        assert_eq!(usage.output_tokens, Some(11));
        assert_eq!(usage.total_tokens, Some(55));
        Ok(())
    }

    #[test]
    fn parses_cohere_token_usage() -> Result<(), String> {
        let (provider, usage) = parse_llm_usage(
            "api.cohere.com",
            &json!({
                "model": "command-r-plus",
                "usage": {
                    "tokens": {
                        "input_tokens": 101,
                        "output_tokens": 19
                    },
                    "billed_units": {
                        "input_tokens": 100,
                        "output_tokens": 20
                    }
                }
            }),
        )
        .ok_or("usage".to_string())?;

        assert_eq!(provider, "cohere");
        assert_eq!(usage.input_tokens, Some(101));
        assert_eq!(usage.output_tokens, Some(19));
        assert_eq!(usage.total_tokens, Some(120));
        Ok(())
    }

    #[test]
    fn parses_broader_openai_compatible_providers() -> Result<(), String> {
        for (host, expected_provider) in [
            ("api.groq.com", "groq"),
            ("api.together.xyz", "together"),
            ("api.perplexity.ai", "perplexity"),
            ("api.fireworks.ai", "fireworks"),
            ("api.cerebras.ai", "cerebras"),
            ("api.replicate.com", "replicate"),
            ("router.huggingface.co", "huggingface"),
        ] {
            let (provider, usage) = parse_llm_usage(
                host,
                &json!({
                    "model": "provider-test-model",
                    "usage": {
                        "prompt_tokens": 21,
                        "completion_tokens": 9,
                        "total_tokens": 30
                    }
                }),
            )
            .ok_or_else(|| format!("usage for {host}"))?;

            assert_eq!(provider, expected_provider);
            assert_eq!(usage.input_tokens, Some(21));
            assert_eq!(usage.output_tokens, Some(9));
            assert_eq!(usage.total_tokens, Some(30));
        }
        Ok(())
    }
}
