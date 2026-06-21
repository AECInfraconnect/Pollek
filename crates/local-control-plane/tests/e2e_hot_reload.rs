use reqwest::Client;
use serde_json::json;

mod common;

#[tokio::test]
async fn e2e_hot_reload_sse() {
    let harness = common::LocalControlPlaneHarness::start().await;
    let base = harness.base_url.clone();
    let client = Client::new();

    // Spawn a task to listen to SSE
    let mut resp = client.get(format!("{base}/v1/push")).send().await.unwrap();

    assert!(resp.status().is_success());

    // Wait briefly for the SSE connection to establish
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Trigger a policy publish
    let meta = json!({
        "schema_version": "v1",
        "tenant_id": "local",
        "workspace_id": "default",
        "environment_id": "local",
        "created_at": "2026-06-10T00:00:00Z",
        "updated_at": "2026-06-10T00:00:00Z",
        "created_by": "local-admin",
        "updated_by": "local-admin",
        "source": "manual",
        "status": "draft",
        "tags": []
    });

    let policy = json!({
        "meta": meta,
        "policy_id": "policy-e2e-hotreload",
        "name": "Hot Reload Test",
        "description": "SSE trigger",
        "policy_type": "cedar",
        "targets": {
            "agent_ids": [], "tool_ids": [], "resource_ids": [], "entity_ids": [], "route_ids": []
        },
        "source": {
            "kind": "raw_text",
            "language": "cedar",
            "text": "permit(principal, action, resource);"
        },
        "compile_options": { "fail_on_warnings": true }
    });

    client
        .post(format!("{base}/v1/tenants/local/policies"))
        .json(&policy)
        .send()
        .await
        .unwrap();

    let published = client
        .post(format!(
            "{base}/v1/tenants/local/policies/policy-e2e-hotreload/publish"
        ))
        .json(&policy)
        .send()
        .await
        .unwrap();

    assert!(published.status().is_success());

    // Now read the SSE stream to see if we got the bundle_id
    // Wait for the first message
    let mut found = false;
    for _ in 0..5 {
        let chunk = resp.chunk().await.unwrap().unwrap();
        let text = String::from_utf8(chunk.to_vec()).unwrap();
        if text.contains("data: bundle-") {
            found = true;
            break;
        }
    }

    // It should contain 'data: bundle-...'
    assert!(found, "SSE did not receive bundle push within 5 chunks");
}
