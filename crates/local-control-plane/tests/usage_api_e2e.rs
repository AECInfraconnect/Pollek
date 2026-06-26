#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::duplicated_attributes
)]
use reqwest::Client;
use serde_json::json;

mod common;

#[tokio::test]
async fn e2e_ai_usage_ingest_summary_and_budget() {
    let harness = common::LocalControlPlaneHarness::start().await;
    let base = harness.base_url.clone();
    let client = Client::new();

    let event = json!({
        "schema_version": "ai-usage-event.v1",
        "event_id": "usage_evt_1",
        "event_kind": "model_call_completed",
        "occurred_at": "2026-06-26T00:00:00Z",
        "received_at": "2026-06-26T00:00:01Z",
        "tenant_id": "local",
        "workspace_id": "default",
        "device_id": "device_test",
        "trace_id": "trace_usage_1",
        "span_id": "span_usage_1",
        "agent_id": "agent_usage",
        "agent_type": "coding_agent",
        "provider": "fixture",
        "provider_api": "responses",
        "provider_request_id": "provider_req_1",
        "model": "fixture-model",
        "surface": "sdk",
        "policy_ids": [],
        "tokens": {
            "input_tokens": 100,
            "output_tokens": 25,
            "total_tokens": 125,
            "cached_input_tokens": 10,
            "cache_write_input_tokens": 0,
            "reasoning_output_tokens": 5,
            "tool_prompt_tokens": 0,
            "tool_result_tokens": 0,
            "image_input_tokens": 0,
            "image_output_tokens": 0,
            "audio_input_tokens": 0,
            "audio_output_tokens": 0,
            "video_input_tokens": 0,
            "by_modality": {},
            "usage_details_ext": {},
            "estimated": false,
            "source": "provider_response"
        },
        "cost": {
            "currency": "USD",
            "input_cost": 0.001,
            "output_cost": 0.002,
            "cached_input_cost": 0.0001,
            "cache_write_input_cost": 0.0,
            "reasoning_output_cost": 0.0005,
            "tool_cost": 0.0,
            "image_cost": 0.0,
            "audio_cost": 0.0,
            "total_cost": 0.0036,
            "price_catalog_version": "fixture",
            "cost_source": "price_catalog_exact",
            "estimated": true,
            "cost_details_ext": {}
        },
        "status": "ok",
        "provider_usage_raw": { "input_tokens": 100, "output_tokens": 25 },
        "metadata": {},
        "cloud_sync_status": "pending",
        "idempotency_key": "usage-idempotency-1"
    });

    for _ in 0..2 {
        let res = client
            .post(format!("{base}/v1/tenants/local/usage/events"))
            .json(&event)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 201);
    }

    let summary = client
        .get(format!(
            "{base}/v1/tenants/local/usage/summary?from=2026-06-25T00:00:00Z&bucket=1m"
        ))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert_eq!(summary["schema_version"], "ai-usage-summary.v1");
    assert_eq!(summary["totals"]["request_count"], 1);
    assert_eq!(summary["totals"]["total_tokens"], 125);
    assert_eq!(summary["by_agent"][0]["key"], "agent_usage");

    let budget = json!({
        "schema_version": "ai-budget-limit.v1",
        "budget_id": "budget_agent_usage",
        "tenant_id": "local",
        "scope_type": "agent",
        "scope_id": "agent_usage",
        "window": "day",
        "currency": "USD",
        "soft_cost_limit": 0.001,
        "hard_cost_limit": 1.0,
        "soft_token_limit": 100,
        "hard_token_limit": 1000,
        "action_on_soft": "warn",
        "action_on_hard": "deny",
        "enabled": true,
        "created_at": "2026-06-26T00:00:00Z",
        "updated_at": "2026-06-26T00:00:00Z"
    });

    let res = client
        .put(format!(
            "{base}/v1/tenants/local/usage/budgets/budget_agent_usage"
        ))
        .json(&budget)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    let budgets = client
        .get(format!("{base}/v1/tenants/local/usage/budgets"))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert_eq!(budgets["items"][0]["budget_id"], "budget_agent_usage");
}
