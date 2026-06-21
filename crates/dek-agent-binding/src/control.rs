use serde::{Deserialize, Serialize};
use crate::capability::{CapabilityDescriptor, Surface};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlBindingSpec {
    pub surface_selector: String,
    pub strategy: ControlStrategy,
    pub reversible: bool,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlStrategy {
    StdioWrapperInjection { wrapper_path: String },
    HttpProxyRedirect { local_proxy: String },
    NetworkEgressInterception,
    ObserveOnly,
}

pub fn derive_control(cap: &CapabilityDescriptor) -> Vec<ControlBindingSpec> {
    cap.interaction_surfaces.iter().map(|s| match s {
        Surface::McpStdio => ControlBindingSpec {
            surface_selector: "mcp_stdio".into(),
            strategy: ControlStrategy::StdioWrapperInjection {
                wrapper_path: "dek-mcp-stdio-wrapper".into(),
            },
            reversible: true,
            requires_approval: false,
        },
        Surface::McpHttp { url } | Surface::McpSse { url } => ControlBindingSpec {
            surface_selector: "mcp_http".into(),
            strategy: ControlStrategy::HttpProxyRedirect {
                local_proxy: format!("http://127.0.0.1:8787/proxy?upstream={url}"),
            },
            reversible: true,
            requires_approval: false,
        },
        Surface::OpenAiCompatApi { .. } => ControlBindingSpec {
            surface_selector: "openai_api".into(),
            strategy: ControlStrategy::NetworkEgressInterception,
            reversible: true,
            requires_approval: true,
        },
        _ => ControlBindingSpec {
            surface_selector: "native".into(),
            strategy: ControlStrategy::ObserveOnly,
            reversible: true,
            requires_approval: false,
        },
    }).collect()
}
