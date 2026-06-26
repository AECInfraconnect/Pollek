// SPDX-License-Identifier: Apache-2.0

use crate::{error::ApiResult, state::AppState};
use axum::{
    extract::{Path, State},
    Json,
};
use std::fs;

pub async fn do_apply_binding(binding_id: &str) -> Result<(), String> {
    let mock_config_dir = std::env::temp_dir().join(".pollek_dek").join("mcp_configs");

    fs::create_dir_all(&mock_config_dir).unwrap_or_default();

    let original_file = mock_config_dir.join(format!("{}.json", binding_id));
    let backup_file = mock_config_dir.join(format!("{}.json.pollek.bak", binding_id));

    // Ensure original file exists (mock it if it doesn't)
    if !original_file.exists() {
        let initial_config = serde_json::json!({
            "mcpServers": {
                "sqlite": {
                    "command": "uvx",
                    "args": ["mcp-server-sqlite", "--db-path", "~/test.db"]
                }
            }
        });
        let backup_content = serde_json::to_string_pretty(&initial_config)
            .map_err(|e| format!("Failed to serialize config backup: {}", e))?;
        let _ = fs::write(&original_file, backup_content);
    }

    // Create backup
    if let Ok(content) = fs::read_to_string(&original_file) {
        let _ = fs::write(&backup_file, &content);

        // Rewrite config with wrapper
        if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(servers) = config.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
                for (_name, server) in servers.iter_mut() {
                    if let Some(obj) = server.as_object_mut() {
                        let original_command = obj
                            .get("command")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let original_args = obj
                            .get("args")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default();

                        obj.insert(
                            "command".to_string(),
                            serde_json::json!("dek-stdio-wrapper"),
                        );

                        let mut new_args = vec![
                            serde_json::json!("--agent-id"),
                            serde_json::json!("agent_auto"),
                            serde_json::json!("--target-cmd"),
                            serde_json::json!(original_command),
                        ];
                        new_args.extend(original_args);

                        obj.insert("args".to_string(), serde_json::Value::Array(new_args));
                        obj.insert(
                            "env".to_string(),
                            serde_json::json!({
                                "POLLEK_DEK_ROUTER_URL": "http://127.0.0.1:3000/v1/pdp/route"
                            }),
                        );
                    }
                }
            }
            let _ = fs::write(
                &original_file,
                serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?,
            );
        }
    }
    Ok(())
}

pub async fn apply_control_binding(
    Path((_tenant, binding_id)): Path<(String, String)>,
    State(_st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let _ = do_apply_binding(&binding_id).await;

    let mock_config_dir = std::env::temp_dir().join(".pollek_dek").join("mcp_configs");
    let original_file = mock_config_dir.join(format!("{}.json", binding_id));
    let backup_file = mock_config_dir.join(format!("{}.json.pollek.bak", binding_id));

    Ok(Json(serde_json::json!({
        "binding_id": binding_id,
        "status": "applied",
        "mock_path": original_file.to_string_lossy(),
        "backup_path": backup_file.to_string_lossy(),
    })))
}

pub async fn rollback_control_binding(
    Path((_tenant, binding_id)): Path<(String, String)>,
    State(_st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let mock_config_dir = std::env::temp_dir().join(".pollek_dek").join("mcp_configs");

    let original_file = mock_config_dir.join(format!("{}.json", binding_id));
    let backup_file = mock_config_dir.join(format!("{}.json.pollek.bak", binding_id));

    if backup_file.exists() {
        let _ = fs::copy(&backup_file, &original_file);
        let _ = fs::remove_file(&backup_file);
    }

    Ok(Json(serde_json::json!({
        "binding_id": binding_id,
        "status": "rolled_back",
        "mock_path": original_file.to_string_lossy(),
    })))
}
