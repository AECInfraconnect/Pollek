use reqwest::Client;
use serde_json::json;

mod common;

#[tokio::test]
async fn e2e_publish_cedar_policy_and_get_manifest() {
    let harness = common::LocalControlPlaneHarness::start().await;
    let base = harness.base_url.clone();
    let client = Client::new();

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
        "tags": ["e2e"]
    });

    let policy = json!({
        "meta": meta,
        "policy_id": "policy-e2e-deny-critical",
        "name": "Deny Critical Tools",
        "description": "E2E Cedar deny policy",
        "policy_type": "cedar",
        "targets": {
            "agent_ids": [], "tool_ids": [], "resource_ids": [], "entity_ids": [], "route_ids": []
        },
        "source": {
            "kind": "raw_text",
            "language": "cedar",
            "text": "forbid(principal, action, resource) when { context.risk_level == \"critical\" };"
        },
        "compile_options": { "fail_on_warnings": true }
    });

    let created = client
        .post(format!("{base}/v1/tenants/local/policies"))
        .json(&policy)
        .send()
        .await
        .unwrap();
    assert!(created.status().is_success(), "Failed to create policy");

    let published = client
        .post(format!(
            "{base}/v1/tenants/local/policies/policy-e2e-deny-critical/publish"
        ))
        .json(&policy)
        .send()
        .await
        .unwrap();
    if !published.status().is_success() {
        let text = published.text().await.unwrap();
        panic!("Failed to publish policy: {}", text);
    }

    let manifest = client
        .get(format!(
            "{base}/v1/tenants/local/devices/device-001/bundles/manifest"
        ))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();

    assert_eq!(manifest["manifest"]["metadata"]["tenant"], "local");
    assert!(
        !manifest["manifest"]["artifacts"]
            .as_array()
            .unwrap()
            .is_empty(),
        "Missing artifacts"
    );
}
