// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub agent_id: String,
    pub path: PathBuf,
    pub should_wrap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewritePlan {
    pub agent_id: String,
    pub original_path: PathBuf,
    pub backup_path: PathBuf,
    pub new_content: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewriteReport {
    pub agent_id: String,
    pub status: String,
    pub backup_path: PathBuf,
}

pub trait AgentConfigRewriter {
    fn scan(&self) -> Result<Vec<AgentConfig>>;
    fn plan_rewrite(&self, config: &AgentConfig) -> Result<RewritePlan>;
    fn apply_rewrite(&self, plan: RewritePlan) -> Result<RewriteReport>;
    fn restore(&self, agent_id: &str) -> Result<()>;
}

pub struct ClaudeDesktopRewriter {
    app_data_dir: PathBuf,
    wrapper_path: PathBuf,
}

impl ClaudeDesktopRewriter {
    pub fn new(app_data_dir: PathBuf, wrapper_path: PathBuf) -> Self {
        Self {
            app_data_dir,
            wrapper_path,
        }
    }

    fn config_path(&self) -> PathBuf {
        self.app_data_dir
            .join("Claude")
            .join("claude_desktop_config.json")
    }

    fn backup_path(&self) -> PathBuf {
        self.app_data_dir
            .join("Claude")
            .join("claude_desktop_config.backup.json")
    }
}

impl AgentConfigRewriter for ClaudeDesktopRewriter {
    fn scan(&self) -> Result<Vec<AgentConfig>> {
        let path = self.config_path();
        if path.exists() {
            Ok(vec![AgentConfig {
                agent_id: "claude-desktop".into(),
                path,
                should_wrap: true, // In reality, fetch from binding store
            }])
        } else {
            Ok(vec![])
        }
    }

    fn plan_rewrite(&self, config: &AgentConfig) -> Result<RewritePlan> {
        let content = fs::read_to_string(&config.path)?;
        let mut json: Value = serde_json::from_str(&content)?;

        if !config.should_wrap {
            // Policy says no wrapping
            return Ok(RewritePlan {
                agent_id: config.agent_id.clone(),
                original_path: config.path.clone(),
                backup_path: self.backup_path(),
                new_content: json,
            });
        }

        if let Some(mcp_servers) = json.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
            for (server_id, server_config) in mcp_servers.iter_mut() {
                let original_cmd = server_config
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // Avoid double wrapping
                if original_cmd.contains("dek-stdio-wrapper") {
                    continue;
                }

                let mut original_args: Vec<String> = server_config
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let mut new_args = vec![
                    "--server-id".to_string(),
                    server_id.clone(),
                    "--agent-id".to_string(),
                    config.agent_id.clone(),
                    "--".to_string(),
                    original_cmd,
                ];
                new_args.append(&mut original_args);

                server_config["command"] =
                    Value::String(self.wrapper_path.to_string_lossy().to_string());
                server_config["args"] = serde_json::to_value(new_args)?;
            }
        }

        Ok(RewritePlan {
            agent_id: config.agent_id.clone(),
            original_path: config.path.clone(),
            backup_path: self.backup_path(),
            new_content: json,
        })
    }

    fn apply_rewrite(&self, plan: RewritePlan) -> Result<RewriteReport> {
        // Create backup
        if plan.original_path.exists() {
            fs::copy(&plan.original_path, &plan.backup_path)?;
        }

        // Write new config
        let formatted = serde_json::to_string_pretty(&plan.new_content)?;
        fs::write(&plan.original_path, formatted)?;

        Ok(RewriteReport {
            agent_id: plan.agent_id,
            status: "success".into(),
            backup_path: plan.backup_path,
        })
    }

    fn restore(&self, _agent_id: &str) -> Result<()> {
        let backup = self.backup_path();
        let original = self.config_path();

        if backup.exists() {
            fs::copy(&backup, &original)?;
            Ok(())
        } else {
            anyhow::bail!("No backup found at {:?}", backup)
        }
    }
}
