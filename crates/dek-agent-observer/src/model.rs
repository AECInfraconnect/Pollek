use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentObservationEvent {
    pub event_id: String,
    pub tenant_id: String,
    pub trace_id: String,
    pub agent_id: Option<String>,
    pub shadow_candidate_id: Option<String>,
    pub tool_id: Option<String>,
    pub resource_id: Option<String>,
    pub surface: String,
    pub action: String,
    pub pep_type: Option<String>,
    pub risk_level: Option<String>,
    pub timestamp: String,
    pub payload_json: String,
    pub token_usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostLedgerEntry {
    pub event_id: String,
    pub agent_id: String,
    pub provider: String,
    pub model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
    pub currency: String,
    pub estimated: bool,
    pub timestamp: String,
}
