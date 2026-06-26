// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EnforcementMode {
    Observe,
    Enforce,
    Shadow,
    Audit,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Detected,
    Registered,
    Controlled,
    Ignored,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentRecord {
    pub id: String,
    pub display_name: String,
    pub vendor: Option<String>,
    pub process_names: Vec<String>,
    pub config_paths: Vec<String>,
    pub detected_tools: Vec<crate::Tool>,
    pub detected_resources: Vec<crate::Resource>,
    pub confidence: f32,
    pub status: AgentStatus,
    #[serde(default = "default_trust_score")]
    pub trust_score: i32,
}

fn default_trust_score() -> i32 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PepPrivilegeLevel {
    None,
    User,
    Admin,
    Kernel,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PepBindingStatus {
    Active,
    Inactive,
    Misconfigured,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PepBinding {
    pub id: String,
    pub agent_id: String,
    pub pep_type: String,
    pub mode: EnforcementMode,
    pub can_observe: bool,
    pub can_enforce: bool,
    pub resources: Vec<String>,
    pub required_privileges: PepPrivilegeLevel,
    pub status: PepBindingStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PdpKind {
    OpaWasm,
    CedarLocal,
    Openfga,
    PollekCloud,
    PluginWasm,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PdpCategory {
    Local,
    Remote,
    Cloud,
    Plugin,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PdpStatus {
    Ready,
    Degraded,
    Unreachable,
    Disabled,
    Misconfigured,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SupportedLanguage {
    Rego,
    Cedar,
    Openfga,
    PluginAbiV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PdpRuntime {
    pub id: String,
    pub kind: PdpKind,
    pub category: PdpCategory,
    pub status: PdpStatus,
    pub supported_languages: Vec<SupportedLanguage>,
    pub p95_latency_ms: Option<u64>,
    pub active_bundle_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PepCandidate {
    pub pep_type: String,
    pub can_observe: bool,
    pub can_enforce: bool,
    pub setup_required: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiscoveredAgent {
    pub id: String,
    pub name: String,
    pub confidence: f32,
    pub process_names: Vec<String>,
    pub config_paths: Vec<String>,
    pub detected_mcp_servers: Vec<String>,
    pub tools: Vec<String>,
    pub resources: Vec<String>,
    pub available_peps: Vec<PepCandidate>,
    pub recommended_policies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DecisionObligation {
    Redact { json_path: String },
    MaskOutput,
    RequireApproval { approver: String },
    LimitTokens { max_tokens: u64 },
    BlockNetwork { host: String },
    LogOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DecisionEnvelope {
    pub decision_id: String,
    pub timestamp_ms: i64,
    pub agent_id: String,
    pub user_id: String,
    pub action: String,
    pub resource: String,
    pub pep_type: String,
    pub pdp_runtime_id: String,
    pub policy_bundle_id: String,
    pub route_id: String,
    pub allowed: bool,
    pub mode: String,
    pub reason: String,
    pub obligations: Vec<DecisionObligation>,
    pub latency_ms: u64,
    pub fallback_used: bool,
    pub redacted_fields: Vec<String>,
}
