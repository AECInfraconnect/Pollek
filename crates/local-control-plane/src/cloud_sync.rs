use crate::state::AppState;
use reqwest::Client;
use sha2::Digest;
use std::time::Duration;
use tracing::{info, warn};

fn local_device_id() -> String {
    if let Ok(id) = std::env::var("POLLEK_DEVICE_ID") {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "local-device".to_string());
    let mut hasher = sha2::Sha256::new();
    hasher.update(host.as_bytes());
    let digest = hasher.finalize();
    format!("dev_{}", hex::encode(&digest[..8]))
}

pub async fn start_cloud_registry_sync_loop(state: AppState) -> anyhow::Result<()> {
    tokio::spawn(async move {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        let device_id = local_device_id();

        loop {
            // Wait before starting the sync to avoid startup load
            tokio::time::sleep(Duration::from_secs(300)).await;

            // Retrieve configuration safely in case it changes or is absent
            // For now, it relies on DEK_CLOUD_URL from env if it was populated in AppState
            let cloud_url = match std::env::var("DEK_CLOUD_URL").ok() {
                Some(url) if !url.is_empty() => url,
                _ => {
                    // Skip sync if no cloud URL is configured
                    continue;
                }
            };

            let api_key = std::env::var("DEK_CLOUD_API_KEY").unwrap_or_default();
            let tenant_id = "local"; // In local control plane, default tenant is local

            info!("Cloud Sync Loop: Starting registry sync to {}", cloud_url);

            // Fetch explicitly registered objects.
            // Notice: We intentionally DO NOT fetch `discovery_scan`, `discovery_candidate`,
            // or `discovery_evidence` raw objects here.

            let mut all_objects: Vec<serde_json::Value> = Vec::new();

            // Agents
            if let Ok(agents) = state.registry_store.list_agents(tenant_id).await {
                for item in agents {
                    if let Ok(val) = serde_json::to_value(item) {
                        all_objects.push(serde_json::json!({
                            "type": "agent",
                            "data": val
                        }));
                    }
                }
            }

            // MCP Servers
            if let Ok(servers) = state.registry_store.list_mcp_servers(tenant_id).await {
                for item in servers {
                    if let Ok(val) = serde_json::to_value(item) {
                        all_objects.push(serde_json::json!({
                            "type": "mcp_server",
                            "data": val
                        }));
                    }
                }
            }

            // Tools
            if let Ok(tools) = state.registry_store.list_tools(tenant_id).await {
                for item in tools {
                    if let Ok(val) = serde_json::to_value(item) {
                        all_objects.push(serde_json::json!({
                            "type": "tool",
                            "data": val
                        }));
                    }
                }
            }

            // Resources
            if let Ok(resources) = state.registry_store.list_resources(tenant_id).await {
                for item in resources {
                    if let Ok(val) = serde_json::to_value(item) {
                        all_objects.push(serde_json::json!({
                            "type": "resource",
                            "data": val
                        }));
                    }
                }
            }

            // Entities
            if let Ok(entities) = state.registry_store.list_entities(tenant_id).await {
                for item in entities {
                    if let Ok(val) = serde_json::to_value(item) {
                        all_objects.push(serde_json::json!({
                            "type": "entity",
                            "data": val
                        }));
                    }
                }
            }

            // Relationships
            if let Ok(relationships) = state.registry_store.list_relationships(tenant_id).await {
                for item in relationships {
                    if let Ok(val) = serde_json::to_value(item) {
                        all_objects.push(serde_json::json!({
                            "type": "relationship",
                            "data": val
                        }));
                    }
                }
            }

            // Agent Inventories (Phase 7)
            if let Ok(inventories) = state.registry_store.list_agent_inventories(tenant_id).await {
                for item in inventories {
                    if let Ok(val) = serde_json::to_value(item) {
                        all_objects.push(serde_json::json!({
                            "type": "agent_inventory",
                            "data": val
                        }));
                    }
                }
            }

            // Canonical discovery capability inventory. Unlike raw discovery
            // scans/candidates/evidence (which stay local by design), these are
            // the user-approved, metadata-only entities produced by
            // retrieve-capabilities. Raw evidence is stripped before upload;
            // capabilities and relationships reference evidence by id only.
            if let Ok(entities) = state
                .registry_store
                .list_raw(tenant_id, "discovery_entity")
                .await
            {
                for item in entities {
                    all_objects.push(serde_json::json!({
                        "type": "discovery_entity",
                        "data": sanitize_discovery_entity(item)
                    }));
                }
            }
            for object_type in ["discovered_capability", "discovered_relationship"] {
                if let Ok(items) = state.registry_store.list_raw(tenant_id, object_type).await {
                    for item in items {
                        all_objects.push(serde_json::json!({
                            "type": object_type,
                            "data": item
                        }));
                    }
                }
            }

            // Telemetry: Deployments (Phase 7)
            if let Ok(deployments) = state
                .telemetry_store
                .list_telemetry(tenant_id, "policy_deployment")
                .await
            {
                for item in deployments {
                    all_objects.push(serde_json::json!({
                        "type": "telemetry_policy_deployment",
                        "data": item
                    }));
                }
            }

            // Telemetry: Tool Invocations (Phase 7)
            if let Ok(invocations) = state
                .telemetry_store
                .list_telemetry(tenant_id, "tool_invocation")
                .await
            {
                for item in invocations {
                    all_objects.push(serde_json::json!({
                        "type": "telemetry_tool_invocation",
                        "data": item
                    }));
                }
            }

            // Telemetry: Resource Access (Phase 7)
            if let Ok(accesses) = state
                .telemetry_store
                .list_telemetry(tenant_id, "resource_access")
                .await
            {
                for item in accesses {
                    all_objects.push(serde_json::json!({
                        "type": "telemetry_resource_access",
                        "data": item
                    }));
                }
            }

            // Push to cloud endpoint
            if all_objects.is_empty() {
                continue;
            }

            let payload = serde_json::json!({
                "tenant_id": tenant_id,
                "items": all_objects
            });

            let endpoint = format!(
                "{}/v1/tenants/{}/registry/sync",
                cloud_url.trim_end_matches('/'),
                tenant_id
            );

            let mut req = client.post(&endpoint).json(&payload);

            if !api_key.is_empty() {
                req = req.bearer_auth(&api_key);
            }

            match req.send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        info!(
                            "Cloud Sync Loop: Successfully synced {} registry objects to cloud",
                            all_objects.len()
                        );
                    } else {
                        warn!(
                            "Cloud Sync Loop: Failed to sync registry objects. Status: {}",
                            resp.status()
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        "Cloud Sync Loop: Connection error while syncing to cloud: {}",
                        e
                    );
                }
            }

            // Phase 6.5: Push Telemetry Batches
            if let Ok(records) = state.secure_spool.pop_batch(100) {
                if !records.is_empty() {
                    info!(
                        "Cloud Sync Loop: Pushing {} telemetry events",
                        records.len()
                    );

                    let mut envelopes = Vec::new();
                    let mut ids_to_delete = Vec::new();
                    let mut ai_usage_event_ids = Vec::new();
                    for (id, bytes) in records {
                        if let Ok(env) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                            if env.get("event_type").and_then(|value| value.as_str())
                                == Some("ai_usage_event")
                            {
                                if let Some(event_id) =
                                    env.get("event_id").and_then(|value| value.as_str())
                                {
                                    ai_usage_event_ids.push(event_id.to_string());
                                }
                            }
                            envelopes.push(env);
                            ids_to_delete.push(id);
                        }
                    }

                    if !envelopes.is_empty() {
                        let telemetry_endpoint =
                            format!("{}/v1/telemetry/batches", cloud_url.trim_end_matches('/'));

                        let batch_payload = serde_json::json!({
                            "schema_version": "telemetry-batch.v1",
                            "tenant_id": tenant_id,
                            "device_id": device_id.clone(),
                            "batch_id": format!("batch-{}", chrono::Utc::now().timestamp_millis()),
                            "events": envelopes
                        });

                        let mut push_req = client.post(&telemetry_endpoint).json(&batch_payload);
                        if !api_key.is_empty() {
                            push_req = push_req.bearer_auth(&api_key);
                        }

                        match push_req.send().await {
                            Ok(resp) if resp.status().is_success() => {
                                info!("Cloud Sync Loop: Successfully pushed telemetry batch");
                                if let Err(e) = state.secure_spool.delete_batch(&ids_to_delete) {
                                    warn!(
                                        "Cloud Sync Loop: Failed to delete spooled telemetry: {}",
                                        e
                                    );
                                }
                                if let Err(e) = state
                                    .observability_store
                                    .mark_ai_usage_events_sync_status(&ai_usage_event_ids, "acked")
                                    .await
                                {
                                    warn!(
                                        "Cloud Sync Loop: Failed to mark AI usage telemetry acked: {}",
                                        e
                                    );
                                }
                            }
                            Ok(resp) => {
                                warn!(
                                    "Cloud Sync Loop: Failed to push telemetry, status: {}",
                                    resp.status()
                                );
                            }
                            Err(e) => {
                                warn!("Cloud Sync Loop: Error pushing telemetry: {}", e);
                            }
                        }
                    }
                }
            }

            // Phase 7: Pull managed policy bundles
            let bundles_endpoint = format!(
                "{}/v1/tenants/{}/bundles/latest",
                cloud_url.trim_end_matches('/'),
                tenant_id
            );

            let mut pull_req = client.get(&bundles_endpoint);
            if !api_key.is_empty() {
                pull_req = pull_req.bearer_auth(&api_key);
            }

            match pull_req.send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        info!("Cloud Sync Loop: Successfully pulled latest managed bundles");
                        // If we had a real cloud, we would parse and save it via state.policy_store
                    } else if resp.status() != reqwest::StatusCode::NOT_FOUND {
                        // Mock cloud might return 404, which is fine
                        warn!(
                            "Cloud Sync Loop: Failed to pull bundles. Status: {}",
                            resp.status()
                        );
                    }
                }
                Err(e) => {
                    warn!("Cloud Sync Loop: Error pulling bundles: {}", e);
                }
            }

            // Phase 7: Pull Cloud PDP route suggestions
            let routes_endpoint = format!(
                "{}/v1/tenants/{}/pdp/routes/suggested",
                cloud_url.trim_end_matches('/'),
                tenant_id
            );
            let mut route_req = client.get(&routes_endpoint);
            if !api_key.is_empty() {
                route_req = route_req.bearer_auth(&api_key);
            }

            match route_req.send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        info!("Cloud Sync Loop: Successfully pulled cloud PDP route suggestions");
                    }
                }
                Err(e) => {
                    warn!("Cloud Sync Loop: Error pulling PDP routes: {}", e);
                }
            }
        }
    });

    Ok(())
}

/// Strips raw discovery evidence from a persisted `discovery_entity` before it
/// leaves the device. Cloud receives the canonical entity, its capabilities,
/// and relationships — evidence records stay local and are referenced from
/// capabilities by `evidence_ids` only.
fn sanitize_discovery_entity(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = value.as_object_mut() {
        obj.remove("evidence");
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_evidence_but_keeps_capabilities() {
        let entity = serde_json::json!({
            "candidate_id": "agent_demo",
            "entity_kind": "agent",
            "evidence": [{"evidence_id": "ev_1", "data": {"process": "secret"}}],
            "capabilities": [{"capability_id": "cap_1", "evidence_ids": ["ev_1"]}],
            "relationships": [],
        });

        let sanitized = sanitize_discovery_entity(entity);

        assert!(sanitized.get("evidence").is_none());
        assert_eq!(
            sanitized["capabilities"][0]["capability_id"].as_str(),
            Some("cap_1")
        );
        assert_eq!(sanitized["candidate_id"].as_str(), Some("agent_demo"));
    }
}
