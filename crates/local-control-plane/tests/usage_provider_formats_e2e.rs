#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::duplicated_attributes
)]
//! End-to-end proof that exact token/cost retrieval works for agent types
//! whose providers report usage in *different* response shapes, all through
//! the real `POST /usage/provider-response` endpoint the SDK wrapper / MCP
//! proxy / browser extension call:
//!
//! - OpenAI-compatible: `usage.prompt_tokens` / `usage.completion_tokens`
//! - Anthropic:         `usage.input_tokens` / `usage.output_tokens`
//! - Ollama (local):    top-level `prompt_eval_count` / `eval_count`
//!
//! Each is normalized to the same canonical token counts, and the per-agent
//! usage summary then aggregates them.

use reqwest::Client;
use serde_json::{json, Value};

mod common;

async fn post_provider_response(client: &Client, base: &str, body: Value) -> Value {
    let res = client
        .post(format!("{base}/v1/tenants/local/usage/provider-response"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        201,
        "provider-response should normalize and persist: {body}"
    );
    res.json::<Value>().await.unwrap()
}

#[tokio::test]
async fn e2e_per_provider_format_token_retrieval() {
    let harness = common::LocalControlPlaneHarness::start().await;
    let base = harness.base_url.clone();
    let client = Client::new();

    // 1. OpenAI-style (also covers deepseek/groq/mistral/... via same shape).
    let openai = post_provider_response(
        &client,
        &base,
        json!({
            "provider": "openai",
            "host": "api.openai.com",
            "agent_id": "agent_openai",
            "agent_type": "coding_agent",
            "model": "gpt-4o",
            "raw_response": {
                "id": "chatcmpl-1",
                "model": "gpt-4o",
                "usage": { "prompt_tokens": 100, "completion_tokens": 25, "total_tokens": 125 }
            }
        }),
    )
    .await;
    let item = &openai["item"];
    assert_eq!(item["tokens"]["input_tokens"], 100);
    assert_eq!(item["tokens"]["output_tokens"], 25);
    assert_eq!(item["tokens"]["total_tokens"], 125);
    assert_eq!(item["tokens"]["estimated"], false);
    assert_eq!(item["provider"], "openai");

    // 2. Anthropic-style: input_tokens / output_tokens.
    let anthropic = post_provider_response(
        &client,
        &base,
        json!({
            "provider": "anthropic",
            "host": "api.anthropic.com",
            "agent_id": "agent_anthropic",
            "agent_type": "coding_agent",
            "model": "claude-sonnet-5",
            "raw_response": {
                "id": "msg_1",
                "model": "claude-sonnet-5",
                "usage": { "input_tokens": 200, "output_tokens": 50 }
            }
        }),
    )
    .await;
    let item = &anthropic["item"];
    assert_eq!(item["tokens"]["input_tokens"], 200);
    assert_eq!(item["tokens"]["output_tokens"], 50);
    assert_eq!(item["tokens"]["total_tokens"], 250);
    assert_eq!(item["provider"], "anthropic");

    // 3. Ollama (local model server): top-level prompt_eval_count / eval_count,
    //    a schema unlike the OpenAI/Anthropic `usage` object. The agent_type
    //    "local_model" is deliberately a non-canonical label -- it must be
    //    tolerated (mapped to Unknown) so the exact tokens are still retrieved.
    let ollama = post_provider_response(
        &client,
        &base,
        json!({
            "provider": "ollama",
            "host": "http://127.0.0.1:11434",
            "agent_id": "agent_ollama",
            "agent_type": "local_model",
            "model": "llama3",
            "raw_response": {
                "model": "llama3",
                "prompt_eval_count": 300,
                "eval_count": 75,
                "done": true
            }
        }),
    )
    .await;
    let item = &ollama["item"];
    assert_eq!(
        item["tokens"]["input_tokens"], 300,
        "Ollama prompt_eval_count must map to input tokens"
    );
    assert_eq!(
        item["tokens"]["output_tokens"], 75,
        "Ollama eval_count must map to output tokens"
    );
    assert_eq!(item["tokens"]["total_tokens"], 375);
    assert_eq!(item["provider"], "ollama");

    // The three distinct formats aggregate into one tenant usage summary.
    let summary = client
        .get(format!(
            "{base}/v1/tenants/local/usage/summary?from=2026-01-01T00:00:00Z&bucket=1h"
        ))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(summary["totals"]["request_count"], 3);
    // 125 + 250 + 375 = 750 tokens total across the three providers.
    assert_eq!(summary["totals"]["total_tokens"], 750);

    // Each provider is retrievable as its own agent in the per-agent breakdown.
    let by_agent = summary["by_agent"].as_array().unwrap();
    let agent_keys: Vec<&str> = by_agent.iter().filter_map(|g| g["key"].as_str()).collect();
    for expected in ["agent_openai", "agent_anthropic", "agent_ollama"] {
        assert!(
            agent_keys.contains(&expected),
            "usage summary must include {expected}; got {agent_keys:?}"
        );
    }
}

/// A provider response with no recognizable usage object is rejected with a
/// clear hint rather than silently recording zero tokens.
#[tokio::test]
async fn e2e_provider_response_without_usage_is_rejected() {
    let harness = common::LocalControlPlaneHarness::start().await;
    let base = harness.base_url.clone();
    let client = Client::new();

    let res = client
        .post(format!("{base}/v1/tenants/local/usage/provider-response"))
        .json(&json!({
            "provider": "openai",
            "host": "api.openai.com",
            "raw_response": { "id": "chatcmpl-x", "choices": [] }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    let body = res.json::<Value>().await.unwrap();
    assert!(body["hint"].as_str().unwrap_or_default().contains("usage"));
}
