use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryStatus {
    Discovered,
    PendingApproval,
    Registered,
    Ignored,
    Merged,
    Retired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredAgentCandidate {
    pub schema_version: String,
    pub candidate_id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub status: DiscoveryStatus,

    pub display_name: String,
    pub inferred_agent_type: InferredAgentType,
    pub confidence: f64,
    pub risk_score: u32,

    pub first_seen: String,
    pub last_seen: String,
    pub evidence: Vec<DiscoveryEvidence>,
    pub suggested_registration: SuggestedAgentRegistration,
    pub suggested_observation_profile: ObservationProfile,

    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InferredAgentType {
    DesktopAgent,
    IdeAgent,
    CliAgent,
    BrowserAgent,
    McpClient,
    McpServer,
    LocalModelServer,
    IdeExtension,
    CustomScriptAgent,
    UnknownAiProcess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryEvidence {
    pub evidence_id: String,
    pub source: EvidenceSource,
    pub confidence: f64,
    pub observed_at: String,
    pub privacy_class: PrivacyClass,
    pub redacted: bool,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSource {
    ProcessScan,
    InstalledAppScan,
    McpConfig,
    IdeExtension,
    BrowserExtension,
    LocalModelServer,
    NetworkEgress,
    TokenUsage,
    UserConfirmation,
    CliAgent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyClass {
    PublicMetadata,
    InternalMetadata,
    SensitiveMetadata,
    SecretRedacted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedAgentRegistration {
    pub agent_id: String,
    pub name: String,
    pub agent_type: String,
    pub runtime_name: String,
    pub process_path_hash: Option<String>,
    pub executable_signer: Option<String>,
    pub declared_tools: Vec<String>,
    pub declared_resources: Vec<String>,
    pub trust_level: String,
    pub initial_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationProfile {
    pub mode: ObservationMode,
    pub collect_process_metadata: bool,
    pub collect_network_metadata: bool,
    pub collect_mcp_tool_metadata: bool,
    pub collect_token_usage: bool,
    pub collect_file_metadata: bool,
    pub collect_raw_prompt: bool,
    pub collect_raw_response: bool,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationMode {
    Off,
    ObserveOnly,
    SuggestPolicies,
    EnforceAfterPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryEvidenceV2 {
    pub evidence_id: String,
    pub source: EvidenceSource,
    pub confidence: f64,
    pub observed_at: String,
    pub privacy_class: PrivacyClass,
    pub redacted: bool,
    pub data: serde_json::Value,
    pub merge_key: Option<String>,
    pub source_path_hash: Option<String>,
    pub source_path_redacted: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredAgentCandidateV2 {
    pub schema_version: String,
    pub candidate_id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub status: DiscoveryStatus,
    pub display_name: String,
    pub vendor: Option<String>,
    pub product: Option<String>,
    pub inferred_agent_type: InferredAgentType,
    pub confidence: f64,
    pub risk_score: u32,
    pub first_seen: String,
    pub last_seen: String,
    pub evidence: Vec<DiscoveryEvidenceV2>,
    pub discovered_configs: Vec<DiscoveredConfigRef>,
    pub discovered_endpoints: Vec<DiscoveredEndpointRef>,
    pub discovered_mcp_servers: Vec<DiscoveredMcpServerRef>,
    pub suggested_registration: SuggestedAgentRegistration,
    pub suggested_observation_profile: ObservationProfile,
    pub suggested_control_bindings: Vec<ControlBindingPlan>,
    pub telemetry_plan: TelemetryPlan,
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredConfigRef {
    pub path_hash: String,
    pub path_redacted: String,
    pub config_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredEndpointRef {
    pub url: String,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredMcpServerRef {
    pub server_name: String,
    pub transport: String,
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlBindingKind {
    McpStdioWrapper,
    McpHttpProxy,
    OpenAiCompatibleProxy,
    AnthropicCompatibleProxy,
    OllamaProxy,
    NetworkEgressPep,
    FilePep,
    ObserveOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlBindingAction {
    Wrap,
    Proxy,
    Block,
    Observe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlBindingPlan {
    pub binding_id: String,
    pub kind: ControlBindingKind,
    pub target_candidate_id: String,
    pub target_config_hash: Option<String>,
    pub action: ControlBindingAction,
    pub requires_user_approval: bool,
    pub risk: String,
    pub reversible: bool,
    pub backup_path_hash: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryPlan {
    pub events_endpoint: String,
    pub metrics_endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScanJobStatus {
    Queued,
    Running,
    Completed,
    Partial,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryScanJob {
    pub scan_id: String,
    pub tenant_id: String,
    pub status: ScanJobStatus,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub sources: Vec<String>,
    pub error: Option<String>,
    pub candidates_found: u32,
}
