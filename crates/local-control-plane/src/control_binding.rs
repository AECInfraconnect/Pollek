// SPDX-License-Identifier: Apache-2.0

use crate::{error::ApiResult, state::AppState};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use std::{
    fs,
    path::{Path as FsPath, PathBuf},
};

#[derive(Debug, Deserialize)]
pub struct ApplyControlBindingQuery {
    pub config_path: Option<String>,
}

fn default_binding_config_path(binding_id: &str) -> PathBuf {
    std::env::temp_dir()
        .join(".pollek_dek")
        .join("mcp_configs")
        .join(format!("{}.json", binding_id))
}

fn backup_path_for(config_path: &FsPath) -> Result<PathBuf, String> {
    let Some(file_name) = config_path.file_name().and_then(|name| name.to_str()) else {
        return Err("config_path does not include a file name".to_string());
    };
    Ok(config_path.with_file_name(format!("{}.pollek.bak", file_name)))
}

pub async fn do_apply_binding(binding_id: &str) -> Result<(), String> {
    let config_path = default_binding_config_path(binding_id);
    apply_binding_to_config(binding_id, &config_path)
        .await
        .map(|_| ())
}

pub async fn apply_binding_to_config(
    binding_id: &str,
    config_path: &FsPath,
) -> Result<PathBuf, String> {
    if !config_path.exists() {
        return Err(format!(
            "MCP config file was not found for binding {binding_id}. Provide config_path from Auto Discovery or install the MCP wrapper manually."
        ));
    }

    let content = fs::read_to_string(config_path).map_err(|error| {
        format!(
            "failed to read MCP config {}: {error}",
            config_path.display()
        )
    })?;
    let mut config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|error| format!("invalid MCP config JSON {}: {error}", config_path.display()))?;
    let servers = config
        .get_mut("mcpServers")
        .and_then(|value| value.as_object_mut())
        .ok_or_else(|| "MCP config does not contain an mcpServers object".to_string())?;

    let mut rewritten = 0usize;
    for server in servers.values_mut() {
        let Some(obj) = server.as_object_mut() else {
            continue;
        };
        let original_command = obj
            .get("command")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();

        if original_command.is_empty() {
            continue;
        }
        if original_command == "dek-stdio-wrapper" {
            rewritten += 1;
            continue;
        }

        let original_args = obj
            .get("args")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        let mut new_args = vec![
            serde_json::json!("--agent-id"),
            serde_json::json!("agent_auto"),
            serde_json::json!("--target-cmd"),
            serde_json::json!(original_command),
        ];
        new_args.extend(original_args);

        obj.insert(
            "command".to_string(),
            serde_json::json!("dek-stdio-wrapper"),
        );
        obj.insert("args".to_string(), serde_json::Value::Array(new_args));
        obj.insert(
            "env".to_string(),
            serde_json::json!({
                "POLLEK_DEK_ROUTER_URL": "http://127.0.0.1:3000/v1/pdp/route"
            }),
        );
        rewritten += 1;
    }

    if rewritten == 0 {
        return Err("MCP config has no server command to wrap".to_string());
    }

    let backup_path = backup_path_for(config_path)?;
    fs::write(&backup_path, content)
        .map_err(|error| format!("failed to write backup {}: {error}", backup_path.display()))?;
    let rewritten_json = serde_json::to_string_pretty(&config)
        .map_err(|error| format!("failed to serialize rewritten MCP config: {error}"))?;
    fs::write(config_path, rewritten_json).map_err(|error| {
        format!(
            "failed to write rewritten MCP config {}: {error}",
            config_path.display()
        )
    })?;

    Ok(backup_path)
}

pub async fn apply_control_binding(
    Path((_tenant, binding_id)): Path<(String, String)>,
    State(_st): State<AppState>,
    Query(query): Query<ApplyControlBindingQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let config_path = query
        .config_path
        .map(PathBuf::from)
        .unwrap_or_else(|| default_binding_config_path(&binding_id));

    match apply_binding_to_config(&binding_id, &config_path).await {
        Ok(backup_path) => Ok(Json(serde_json::json!({
            "binding_id": binding_id,
            "status": "applied",
            "applied_for_real": true,
            "config_path": config_path.to_string_lossy(),
            "backup_path": backup_path.to_string_lossy(),
        }))),
        Err(error) => Ok(Json(serde_json::json!({
            "binding_id": binding_id,
            "status": "failed",
            "applied_for_real": false,
            "config_path": config_path.to_string_lossy(),
            "error": error,
            "required_action": "Provide a real MCP config_path or use the manual wrapper instructions."
        }))),
    }
}

pub async fn rollback_control_binding(
    Path((_tenant, binding_id)): Path<(String, String)>,
    State(_st): State<AppState>,
    Query(query): Query<ApplyControlBindingQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let config_path = query
        .config_path
        .map(PathBuf::from)
        .unwrap_or_else(|| default_binding_config_path(&binding_id));
    let backup_path = backup_path_for(&config_path)
        .map_err(|error| crate::error::ApiError::BadRequest(error.to_string()))?;

    if backup_path.exists() {
        fs::copy(&backup_path, &config_path).map_err(|error| {
            crate::error::ApiError::Internal(anyhow::anyhow!(
                "failed to restore backup {}: {error}",
                backup_path.display()
            ))
        })?;
        fs::remove_file(&backup_path).map_err(|error| {
            crate::error::ApiError::Internal(anyhow::anyhow!(
                "failed to remove backup {}: {error}",
                backup_path.display()
            ))
        })?;
        Ok(Json(serde_json::json!({
            "binding_id": binding_id,
            "status": "rolled_back",
            "config_path": config_path.to_string_lossy(),
        })))
    } else {
        Ok(Json(serde_json::json!({
            "binding_id": binding_id,
            "status": "not_rolled_back",
            "config_path": config_path.to_string_lossy(),
            "error": "backup file not found"
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn apply_binding_rejects_missing_config_without_creating_mock() {
        let temp_dir = tempfile::tempdir();
        assert!(temp_dir.is_ok());
        let Ok(temp_dir) = temp_dir else {
            return;
        };
        let config_path = temp_dir.path().join("missing.json");

        let result = apply_binding_to_config("binding-test", &config_path).await;

        assert!(result.is_err());
        assert!(!config_path.exists());
    }

    #[tokio::test]
    async fn apply_binding_rewrites_real_mcp_config_and_backup() {
        let temp_dir = tempfile::tempdir();
        assert!(temp_dir.is_ok());
        let Ok(temp_dir) = temp_dir else {
            return;
        };
        let config_path = temp_dir.path().join("config.json");
        let write_result = fs::write(
            &config_path,
            r#"{"mcpServers":{"sqlite":{"command":"uvx","args":["mcp-server-sqlite"]}}}"#,
        );
        assert!(write_result.is_ok());

        let result = apply_binding_to_config("binding-test", &config_path).await;

        assert!(result.is_ok());
        let Ok(backup_path) = result else {
            return;
        };
        assert!(backup_path.exists());
        let rewritten = fs::read_to_string(&config_path);
        assert!(rewritten.is_ok());
        let Ok(rewritten) = rewritten else {
            return;
        };
        assert!(rewritten.contains("dek-stdio-wrapper"));
        assert!(rewritten.contains("POLLEK_DEK_ROUTER_URL"));
    }
}
