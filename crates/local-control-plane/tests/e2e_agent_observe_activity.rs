#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::duplicated_attributes
)]
use reqwest::Client;
use serde_json::json;

mod common;

fn observation(
    event_id: &str,
    agent_id: Option<&str>,
    shadow_candidate_id: Option<&str>,
    event_kind: &str,
    timestamp: &str,
) -> serde_json::Value {
    let resource_access = if event_kind == "resource_access" {
        json!({
            "resource_type": "file",
            "target_redacted": "~/projects/notes.txt",
            "bytes": 2048,
            "verb": "read"
        })
    } else {
        serde_json::Value::Null
    };
    json!({
        "event_id": event_id,
        "tenant_id": "local",
        "trace_id": format!("trace-{event_id}"),
        "agent_id": agent_id,
        "shadow_candidate_id": shadow_candidate_id,
        "tool_id": null,
        "resource_id": null,
        "surface": "mcp",
        "action": "observe",
        "pep_type": "mcp_proxy",
        "risk_level": "low",
        "timestamp": timestamp,
        "payload_json": "{}",
        "token_usage": null,
        "event_kind": event_kind,
        "decision": null,
        "tool_call": null,
        "resource_access": resource_access,
        "latency_ms": null,
        "provider": null
    })
}

fn usage_event(event_id: &str, agent_id: &str, total_cost: f64) -> serde_json::Value {
    json!({
        "schema_version": "ai-usage-event.v1",
        "event_id": event_id,
        "event_kind": "model_call_completed",
        "occurred_at": "2026-06-26T00:00:00Z",
        "received_at": "2026-06-26T00:00:01Z",
        "tenant_id": "local",
        "workspace_id": "default",
        "device_id": "device_test",
        "trace_id": format!("trace-{event_id}"),
        "span_id": format!("span-{event_id}"),
        "agent_id": agent_id,
        "agent_type": "coding_agent",
        "provider": "fixture",
        "provider_api": "responses",
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
            "cached_input_cost": 0.0,
            "cache_write_input_cost": 0.0,
            "reasoning_output_cost": 0.0,
            "tool_cost": 0.0,
            "image_cost": 0.0,
            "audio_cost": 0.0,
            "total_cost": total_cost,
            "price_catalog_version": "fixture",
            "cost_source": "price_catalog_exact",
            "estimated": false,
            "cost_details_ext": {}
        },
        "status": "ok",
        "provider_usage_raw": {},
        "metadata": {},
        "cloud_sync_status": "pending",
        "idempotency_key": format!("idem-{event_id}")
    })
}

#[tokio::test]
async fn e2e_per_agent_activity_isolates_agents() {
    let harness = common::LocalControlPlaneHarness::start().await;
    let base = harness.base_url.clone();
    let client = Client::new();

    // Agent A: one resource access under its canonical id, one tool call under
    // its shadow candidate id. Agent B: unrelated noise that must not leak in.
    let events = [
        observation(
            "obs-a-1",
            Some("agent-a"),
            None,
            "resource_access",
            "2026-06-26T00:01:00Z",
        ),
        observation(
            "obs-a-2",
            None,
            Some("candidate-a"),
            "tool_call",
            "2026-06-26T00:02:00Z",
        ),
        observation(
            "obs-b-1",
            Some("agent-b"),
            None,
            "resource_access",
            "2026-06-26T00:03:00Z",
        ),
    ];
    for event in &events {
        let res = client
            .post(format!("{base}/v1/tenants/local/observations"))
            .json(event)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 201);
    }

    for (event_id, agent_id, cost) in [
        ("usage-a-1", "agent-a", 0.01),
        ("usage-a-2", "candidate-a", 0.02),
        ("usage-b-1", "agent-b", 0.99),
    ] {
        let res = client
            .post(format!("{base}/v1/tenants/local/usage/events"))
            .json(&usage_event(event_id, agent_id, cost))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 201);
    }

    let res = client
        .get(format!(
            "{base}/v1/tenants/local/observations/agents/agent-a/activity?alt_ids=candidate-a"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();

    assert_eq!(
        body["schema_version"].as_str(),
        Some("agent-observe-activity.v1")
    );
    assert_eq!(body["agent_id"].as_str(), Some("agent-a"));
    assert_eq!(body["counts"]["total_events"].as_u64(), Some(2));
    assert_eq!(
        body["counts"]["by_kind"]["resource_access"].as_u64(),
        Some(1)
    );
    assert_eq!(body["counts"]["by_kind"]["tool_call"].as_u64(), Some(1));

    let activity = body["activity"].as_array().unwrap();
    assert_eq!(activity.len(), 2);

    let resources = body["resources"].as_array().unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(
        resources[0]["target"].as_str(),
        Some("~/projects/notes.txt")
    );
    assert_eq!(resources[0]["access_count"].as_u64(), Some(1));
    assert_eq!(resources[0]["total_bytes"].as_i64(), Some(2048));

    // Usage must include both of agent A's ids but exclude agent B entirely.
    let usage = &body["usage"];
    assert_eq!(usage["request_count"].as_u64(), Some(2));
    assert_eq!(usage["total_tokens"].as_i64(), Some(250));
    let cost = usage["total_cost"].as_f64().unwrap();
    assert!((cost - 0.03).abs() < 1e-9, "unexpected cost {cost}");
    assert_eq!(usage["exact_events"].as_u64(), Some(2));
    assert_eq!(usage["estimated_events"].as_u64(), Some(0));

    // Agent B's view must not contain agent A's events.
    let res = client
        .get(format!(
            "{base}/v1/tenants/local/observations/agents/agent-b/activity"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["counts"]["total_events"].as_u64(), Some(1));
    let usage = &body["usage"];
    assert_eq!(usage["request_count"].as_u64(), Some(1));
    let cost = usage["total_cost"].as_f64().unwrap();
    assert!((cost - 0.99).abs() < 1e-9, "unexpected cost {cost}");
}

#[tokio::test]
async fn e2e_observation_list_filters_by_agent() {
    let harness = common::LocalControlPlaneHarness::start().await;
    let base = harness.base_url.clone();
    let client = Client::new();

    for (event_id, agent) in [("obs-f-1", "agent-x"), ("obs-f-2", "agent-y")] {
        let res = client
            .post(format!("{base}/v1/tenants/local/observations"))
            .json(&observation(
                event_id,
                Some(agent),
                None,
                "resource_access",
                "2026-06-26T00:01:00Z",
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 201);
    }

    let res = client
        .get(format!(
            "{base}/v1/tenants/local/observations?agent_id=agent-x"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["agent_id"].as_str(), Some("agent-x"));

    let res = client
        .get(format!(
            "{base}/v1/tenants/local/observations/resources?agent_id=agent-x"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["agent_id"].as_str(), Some("agent-x"));
}
