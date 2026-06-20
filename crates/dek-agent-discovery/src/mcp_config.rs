use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfigEvidence {
    pub config_path_hash: String,
    pub config_path_redacted: String,
    pub client_hint: String,
    pub server_name: String,
    pub transport: String,
    pub command_template: Option<Vec<String>>,
    pub endpoint_domain: Option<String>,
    pub env_key_names: Vec<String>,
}

pub fn discover_mcp_configs(paths: &[PathBuf]) -> Result<Vec<McpServerConfigEvidence>> {
    let mut out = vec![];
    for p in paths {
        if !p.exists() || !p.is_file() { continue; }
        let text = match std::fs::read_to_string(p) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let json: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(servers) = json.get("mcpServers").and_then(|v| v.as_object()) {
            for (name, cfg) in servers {
                let command = cfg.get("command").and_then(|v| v.as_str()).map(|s| s.to_string());
                let args = cfg.get("args")
                    .and_then(|v| v.as_array())
                    .map(|xs| xs.iter()
                        .filter_map(|x| x.as_str())
                        .map(crate::redaction::redact_arg)
                        .collect::<Vec<_>>())
                    .unwrap_or_default();
                let env_key_names = cfg.get("env")
                    .and_then(|v| v.as_object())
                    .map(|m| m.keys().cloned().collect())
                    .unwrap_or_default();
                let url = cfg.get("url").and_then(|v| v.as_str()).or_else(|| cfg.get("endpoint").and_then(|v| v.as_str()));

                out.push(McpServerConfigEvidence {
                    config_path_hash: crate::redaction::sha256_string(&p.to_string_lossy()),
                    config_path_redacted: crate::redaction::redact_path_for_ui(&p.to_string_lossy()),
                    client_hint: infer_client_from_path(p),
                    server_name: name.clone(),
                    transport: if command.is_some() { "stdio".into() } else { "http".into() },
                    command_template: command.map(|c| std::iter::once(crate::redaction::redact_arg(&c)).chain(args).collect()),
                    endpoint_domain: url.and_then(|u| url::Url::parse(u).ok()).and_then(|u| u.host_str().map(|s| s.to_string())),
                    env_key_names,
                });
            }
        }
    }
    Ok(out)
}

fn infer_client_from_path(path: &std::path::Path) -> String {
    let s = path.to_string_lossy().to_ascii_lowercase();
    if s.contains("claude") { "claude-desktop".into() }
    else if s.contains("cursor") { "cursor".into() }
    else if s.contains("windsurf") { "windsurf".into() }
    else if s.contains("code") || s.contains("vscode") { "vscode".into() }
    else { "unknown".into() }
}
