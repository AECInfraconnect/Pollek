use crate::state::AppState;
use reqwest::Client;
use std::time::Duration;
use tracing::{info, warn};

pub async fn start_cloud_registry_sync_loop(state: AppState) -> anyhow::Result<()> {
    tokio::spawn(async move {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

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
        }
    });

    Ok(())
}
