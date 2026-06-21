use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintDefinition {
    pub schema_version: String,
    pub definition_version: u64,
    pub released_at: String,
    pub min_engine_version: String,
    pub kind: DefinitionKind,
    pub base_version: Option<u64>,
    pub signatures: Vec<AgentSignatureV2>,
    pub removed_ids: Vec<String>,
    pub catalog_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionKind {
    Full,
    Delta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSignatureV2 {
    pub id: String,
    pub display_name: String,
    pub agent_type: String,
    pub revision: u32,
    pub meta: SignatureMeta,

    pub process_names: Vec<String>,
    pub binary_hashes: Vec<String>,
    pub config_paths: HashMap<String, Vec<String>>,
    pub config_parsers: Vec<String>,
    pub ports: Vec<u16>,
    pub port_probe: Option<PortProbeSpec>,
    pub detection_logic: DetectionLogic,

    pub control_strategies: Vec<String>,
    pub risk_weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureMeta {
    pub author: String,
    pub description: String,
    pub references: Vec<String>,
    pub added_in: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortProbeSpec {
    pub kind: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DetectionLogic {
    AnyOf,
    ProcessAndConfig,
    ProcessOrConfigWithPort,
    HashMatch,
}
