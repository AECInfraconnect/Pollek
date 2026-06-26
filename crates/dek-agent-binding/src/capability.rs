use dek_fingerprint_defs::model::AgentSignatureV2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub agent_signature_id: String,
    pub interaction_surfaces: Vec<Surface>,
    pub data_reach: Vec<DataReach>,
    pub tool_capabilities: Vec<ToolCapability>,
    pub network_egress: Vec<EgressTarget>,
    pub model_providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    McpStdio { command: String, args: Vec<String> },
    McpHttp { url: String },
    McpSse { url: String },
    OpenAiCompatApi { port: u16 },
    NativeProcess,
    BrowserExtension,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataReach {
    pub kind: String,
    pub path_pattern: Option<String>,
    pub sensitivity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCapability {
    pub tool_name: String,
    pub description: String,
    pub parameters_schema: serde_json::Value,
    pub risk_class: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EgressTarget {
    pub host: String,
    pub purpose: String,
}

pub fn capabilities_from_discovery(
    sig: &AgentSignatureV2,
    discovered_surfaces: Vec<Surface>,
) -> CapabilityDescriptor {
    let mut surfaces: Vec<Surface> = sig
        .control_strategies
        .iter()
        .filter_map(|s| match s.as_str() {
            "mcp_stdio_wrapper" => Some(Surface::McpStdio {
                command: "unknown".into(),
                args: vec![],
            }),
            "ollama_proxy" | "network_egress_pep" => Some(Surface::OpenAiCompatApi {
                port: sig.ports.first().copied().unwrap_or(0),
            }),
            _ => None,
        })
        .collect();

    // Preserve discovered capabilities over the generic signature defaults
    if !discovered_surfaces.is_empty() {
        surfaces = discovered_surfaces;
    }

    CapabilityDescriptor {
        agent_signature_id: sig.id.clone(),
        interaction_surfaces: surfaces,
        data_reach: vec![],
        tool_capabilities: vec![],
        network_egress: vec![],
        model_providers: vec![],
    }
}

pub async fn probe_mcp_tools(surface: &Surface) -> anyhow::Result<Vec<ToolCapability>> {
    let req_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "tools/list",
        "id": 1
    });

    let tools_json: serde_json::Value = match surface {
        Surface::McpHttp { url } => {
            let client = reqwest::Client::new();
            let res = client.post(url).json(&req_body).send().await?;
            res.json().await?
        }
        Surface::McpSse { url } => {
            // For SSE, we would normally connect, get the POST endpoint from the event, and then POST.
            // Simplified for demonstration: we assume the POST endpoint is url + "/message"
            let post_url = if url.ends_with("/sse") {
                url.replace("/sse", "/message")
            } else {
                format!("{}/message", url)
            };
            let client = reqwest::Client::new();
            let res = client.post(&post_url).json(&req_body).send().await?;
            res.json().await?
        }
        Surface::McpStdio { command, args } => {
            // Spawn the process
            use std::process::Stdio;
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            use tokio::process::Command;
            use tokio::time::{timeout, Duration};

            let mut child = Command::new(command)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;

            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to open stdin"))?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to open stdout"))?;

            // Send initialize request
            let init_req = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "initialize",
                "id": 0,
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {
                        "name": "PollekDEK",
                        "version": "1.0.0"
                    }
                }
            });
            let init_msg = format!("{}\n", serde_json::to_string(&init_req)?);
            stdin.write_all(init_msg.as_bytes()).await?;
            stdin.flush().await?;

            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            // Wait for initialize response
            let mut init_received = false;
            let _ = timeout(Duration::from_secs(5), async {
                loop {
                    line.clear();
                    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                        break;
                    }
                    if line.contains("\"jsonrpc\"") && line.contains("\"id\":0") {
                        init_received = true;
                        break;
                    }
                }
            })
            .await;

            if !init_received {
                let _ = child.kill().await;
                anyhow::bail!("Timeout waiting for initialize response");
            }

            // Send initialized notification
            let initialized_notify = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            });
            let notif_msg = format!("{}\n", serde_json::to_string(&initialized_notify)?);
            stdin.write_all(notif_msg.as_bytes()).await?;
            stdin.flush().await?;

            // Send tools/list request
            let msg = format!("{}\n", serde_json::to_string(&req_body)?);
            stdin.write_all(msg.as_bytes()).await?;
            stdin.flush().await?;

            // Read response
            let mut tools_received = false;
            let mut parsed_response = serde_json::json!({});
            let _ = timeout(Duration::from_secs(5), async {
                loop {
                    line.clear();
                    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                        break;
                    }
                    if line.contains("\"jsonrpc\"") && line.contains("\"id\":1") {
                        if let Ok(json) = serde_json::from_str(&line) {
                            parsed_response = json;
                            tools_received = true;
                        }
                        break;
                    }
                }
            })
            .await;

            // Kill child so it doesn't hang around
            let _ = child.kill().await;

            if !tools_received {
                anyhow::bail!("No valid tools/list response from stdio");
            }

            parsed_response
        }
        _ => return Ok(vec![]),
    };

    if let Some(tools) = tools_json
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
    {
        let mut caps = Vec::new();
        for t in tools {
            let name = t
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("unknown")
                .to_string();
            let desc = t
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            let schema = t
                .get("inputSchema")
                .cloned()
                .unwrap_or(serde_json::json!({}));

            let risk_class = if name.contains("delete") || name.contains("remove") {
                "delete".to_string()
            } else if name.contains("exec") || name.contains("run") {
                "exec".to_string()
            } else if name.contains("write") || name.contains("update") {
                "write".to_string()
            } else {
                "read".to_string()
            };

            caps.push(ToolCapability {
                tool_name: name,
                description: desc,
                parameters_schema: schema,
                risk_class,
            });
        }
        Ok(caps)
    } else {
        anyhow::bail!("Invalid or missing tools array in response");
    }
}
