use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PepInfo {
    pub mode: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PrincipalContext {
    pub entity_id: String,
    pub r#type: String,
    pub attributes: HashMap<String, String>,
    pub identity_assurance: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentContext {
    pub agent_id: String,
    pub r#type: String,
    pub runtime: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResourceContext {
    pub resource_id: String,
    pub r#type: String,
    pub uri: String,
    pub classification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequestContext {
    pub mcp: HashMap<String, Value>,
    pub session: HashMap<String, Value>,
    pub runtime: HashMap<String, Value>,
    pub network: HashMap<String, Value>,
    pub risk: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DecisionRequest {
    pub schema_version: String,
    pub request_id: String,
    pub trace_id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub spiffe_id: String,
    pub pep: PepInfo,
    pub principal: PrincipalContext,
    pub agent: AgentContext,
    pub action: String,
    pub resource: ResourceContext,
    pub context: RequestContext,
    pub input_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EvaluatorResult {
    pub evaluator: String,
    pub decision: String,
    pub reason: String,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DecisionResult {
    pub schema_version: String,
    pub request_id: String,
    pub final_decision: String,
    pub combined_reason: String,
    pub evaluator_results: Vec<EvaluatorResult>,
    pub obligations: Vec<String>,
}
