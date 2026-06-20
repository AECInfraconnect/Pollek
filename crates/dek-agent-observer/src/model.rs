use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentObservationEvent {
    pub event_id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub agent_id: Option<String>,
    pub shadow_candidate_id: Option<String>,
    pub timestamp: String,
    
    pub resources: Option<Vec<ResourceObservation>>,
    pub token_usage: Option<TokenUsageObservation>,
    pub cost: Option<CostObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceObservation {
    pub resource_type: String, // "file", "network", "mcp_tool"
    pub uri: String,
    pub action: String, // "read", "write", "execute"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageObservation {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub estimated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostObservation {
    pub currency: String,
    pub input_cost: Option<f64>,
    pub output_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub price_catalog_version: Option<String>,
    pub estimated: bool,
}
