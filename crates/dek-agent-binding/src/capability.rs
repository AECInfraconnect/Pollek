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
    McpStdio,
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

pub fn capabilities_from_signature(sig: &AgentSignatureV2) -> CapabilityDescriptor {
    let surfaces: Vec<Surface> = sig.control_strategies.iter().filter_map(|s| match s.as_str() {
        "mcp_stdio_wrapper" => Some(Surface::McpStdio),
        "ollama_proxy" | "network_egress_pep" => {
            Some(Surface::OpenAiCompatApi { port: sig.ports.first().copied().unwrap_or(0) })
        }
        _ => None,
    }).collect();
    
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
    match surface {
        Surface::McpHttp { url } => {
            let client = reqwest::Client::new();
            let req_body = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "tools/list",
                "id": 1
            });
            
            let res = client.post(url)
                .json(&req_body)
                .send()
                .await?;
                
            let data: serde_json::Value = res.json().await?;
            if let Some(tools) = data.get("result").and_then(|r| r.get("tools")).and_then(|t| t.as_array()) {
                let mut caps = Vec::new();
                for t in tools {
                    let name = t.get("name").and_then(|n| n.as_str()).unwrap_or("unknown").to_string();
                    let desc = t.get("description").and_then(|d| d.as_str()).unwrap_or("").to_string();
                    let schema = t.get("inputSchema").cloned().unwrap_or(serde_json::json!({}));
                    
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
        _ => {
            // Simulated probe for non-HTTP surfaces
            Ok(vec![])
        }
    }
}
