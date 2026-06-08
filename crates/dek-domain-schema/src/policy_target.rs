use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PrincipalMatch {
    pub entity_types: Option<Vec<String>>,
    pub attributes: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentMatch {
    pub agent_types: Option<Vec<String>>,
    pub risk_max: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResourceMatch {
    pub classification_in: Option<Vec<String>>,
    pub data_categories_any: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActionMatch {
    pub r#in: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NetworkMatch {
    pub dest_cidr_in: Option<Vec<String>>,
    pub dest_port_in: Option<Vec<u16>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MatchCriteria {
    pub principal: Option<PrincipalMatch>,
    pub agent: Option<AgentMatch>,
    pub resource: Option<ResourceMatch>,
    pub action: Option<ActionMatch>,
    pub network: Option<NetworkMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConditionalEvaluator {
    pub evaluator: String,
    pub when: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Evaluators {
    pub required: Vec<String>,
    pub conditional: Option<Vec<ConditionalEvaluator>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyTarget {
    pub schema_version: String,
    pub target_id: String,
    pub tenant_id: String,
    pub r#match: MatchCriteria,
    pub evaluators: Evaluators,
    pub obligations: Vec<String>,
}
