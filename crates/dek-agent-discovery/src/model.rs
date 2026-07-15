use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryStatus {
    Discovered,
    Unconfirmed,
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
    WebAIApp,
    McpClient,
    McpServer,
    LocalModelServer,
    IdeExtension,
    CustomScriptAgent,
    AutomationAgent,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    Container,
    PortProbe,
    PythonFramework,
    BrowserSession,
    BrowserWindow,
    BrowserHistory,
    NetworkSni,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    pub mcp_stdio_config_paths: Vec<String>,
    pub mcp_http_urls: Vec<String>,
    pub local_model_endpoints: Vec<String>,
    pub browser_extension_evidence: Vec<String>,
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

/// A single observation signal and whether it is being (or can be) collected
/// for a given agent, plus the concrete method used to collect it. This makes
/// the observability of every discovered type explicit and displayable on both
/// Pollek LCP and Pollek Cloud, instead of leaving operators to infer it from
/// the raw `ObservationProfile` booleans.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservationSignalCoverage {
    /// Stable machine identifier for the signal (e.g. `token_usage`).
    pub signal: String,
    /// Human-readable label for UIs.
    pub label: String,
    /// Whether this signal is actively collected, merely available, or does
    /// not apply to this agent type.
    pub status: ObservationSignalStatus,
    /// The concrete collection method for this signal on this agent type
    /// (e.g. `egress_llm_usage_parser`, `mcp_tool_call_metadata`).
    pub method: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObservationSignalStatus {
    /// The signal is collected for this agent under the suggested profile.
    Active,
    /// The signal could be collected but is disabled by default for this type.
    Available,
    /// The signal is not meaningful for this agent type.
    NotApplicable,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityBoundary {
    LocalDevice,
    LocalBrowserProfile,
    LocalContainer,
    LocalNetwork,
    RemoteCloudSandbox,
    RemoteWorkspace,
    RemoteModelApi,
    McpRemoteServer,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntityRole {
    LocalAgentHost,
    WebAiSurface,
    CloudAgentRuntime,
    RemoteWorkspace,
    ModelApiEndpoint,
    McpToolSurface,
    BrowserProfile,
    GeneratedAppPreview,
    IntegrationEndpoint,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuplicatePolicy {
    Standalone,
    ChildSurface,
    RelatedEndpoint,
    ProviderEndpoint,
    MergedDuplicate,
    NeedsHumanConfirmation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedSurfaceRef {
    pub service_id: String,
    pub display_name: String,
    pub entity_role: EntityRole,
    pub authority_boundary: AuthorityBoundary,
    pub evidence_sources: Vec<EvidenceSource>,
    pub confidence: f64,
    pub control_parent_id: Option<String>,
    pub grouping_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryEvidenceV3 {
    pub evidence_id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub source: EvidenceSource,
    pub observed_at: String,
    pub confidence: f64,
    pub privacy_class: PrivacyClass,
    pub redacted: bool,
    pub subject: EvidenceSubject,
    pub signals: Vec<DiscoverySignal>,
    pub raw_redacted: serde_json::Value,
    pub merge_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EvidenceSubject {
    Process {
        pid: u32,
        process_name: String,
        exe_hash: Option<String>,
    },
    McpServer {
        server_name: String,
        transport: String,
        config_hash: Option<String>,
    },
    HttpEndpoint {
        url_redacted: String,
        port: u16,
        protocol: String,
    },
    BrowserExtension {
        browser: String,
        profile_hash: String,
        extension_id: String,
    },
    Container {
        engine: String,
        container_id_hash: String,
        image: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverySignal {
    pub name: String,
    pub weight: f64,
    pub source: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredAgentCandidateV2 {
    pub schema_version: String,
    pub candidate_id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub status: DiscoveryStatus,
    pub canonical_service_id: String,
    pub surface_group_id: String,
    pub authority_boundary: AuthorityBoundary,
    pub entity_role: EntityRole,
    pub duplicate_policy: DuplicatePolicy,
    pub control_parent_id: Option<String>,
    pub grouping_reason: Option<String>,
    pub observe_scope: String,
    pub enforce_scope: String,
    pub related_surfaces: Vec<RelatedSurfaceRef>,
    #[serde(default)]
    pub instance_count: u32,
    #[serde(default)]
    pub matched_signature_id: Option<String>,
    pub display_name: String,
    pub vendor: Option<String>,
    pub product: Option<String>,
    pub inferred_agent_type: InferredAgentType,
    pub confidence: f64,
    pub risk_score: u32,
    #[serde(default)]
    pub capability_tags: Vec<String>,
    #[serde(default)]
    pub matched_signals: Vec<MatchedSignal>,
    pub first_seen: String,
    pub last_seen: String,
    #[serde(default)]
    pub scan_ids: Vec<String>,
    #[serde(default)]
    pub last_scan_id: Option<String>,
    pub evidence: Vec<DiscoveryEvidenceV2>,
    pub discovered_configs: Vec<DiscoveredConfigRef>,
    pub discovered_endpoints: Vec<DiscoveredEndpointRef>,
    pub discovered_mcp_servers: Vec<DiscoveredMcpServerRef>,
    pub suggested_registration: SuggestedAgentRegistration,
    pub suggested_observation_profile: ObservationProfile,
    /// Per-signal observability derived from the agent type and profile: what
    /// Pollek can actually observe for this specific agent, and how. Surfaced
    /// on Pollek LCP (Activity tab) and forwarded to Pollek Cloud.
    #[serde(default)]
    pub observation_coverage: Vec<ObservationSignalCoverage>,
    pub suggested_control_bindings: Vec<ControlBindingPlan>,
    pub telemetry_plan: TelemetryPlan,
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchedSignal {
    pub kind: String,
    pub detail: String,
    pub weight: f64,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryEntityKind {
    Agent,
    AgenticHost,
    SubAgent,
    McpServer,
    Tool,
    Resource,
    ModelProvider,
    Model,
    EmbeddingModel,
    Reranker,
    SafetyModel,
    VisionModel,
    MultimodalModel,
    WorkflowBlueprint,
    InferenceEndpoint,
    Container,
    Framework,
    IdeExtension,
    BrowserExtension,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalCapability {
    pub capability_id: String,
    pub candidate_id: String,
    pub capability_kind: String,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
    pub output_schema: Option<serde_json::Value>,
    pub modality: Vec<String>,
    pub actions: Vec<String>,
    pub source: String,
    pub confidence: f64,
    pub risk_tags: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub privacy_class: PrivacyClass,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryEntityCandidate {
    pub schema_version: String,
    pub candidate_id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub entity_kind: DiscoveryEntityKind,
    pub display_name: String,
    pub vendor: Option<String>,
    pub product: Option<String>,
    pub confidence: f64,
    pub risk_score: u32,
    pub status: DiscoveryStatus,
    pub capabilities: Vec<CanonicalCapability>,
    pub evidence: Vec<DiscoveryEvidenceV2>,
    pub relationships: Vec<DiscoveredRelationship>,
    pub suggested_registration: serde_json::Value,
    pub suggested_control_bindings: Vec<ControlBindingPlan>,
    /// Per-signal observability for this entity (metadata-only, safe to sync to
    /// Pollek Cloud), so the Cloud console can show what Pollek observes for
    /// each discovered agent and how — mirroring the LCP Activity view.
    #[serde(default)]
    pub observation_coverage: Vec<ObservationSignalCoverage>,
    pub privacy_profile: String,
    pub performance_cost_class: String,
    pub first_seen: String,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredRelationship {
    pub relationship_id: String,
    pub subject_candidate_id: String,
    pub relation: String,
    pub object_candidate_id: String,
    pub confidence: f64,
    pub evidence_ids: Vec<String>,
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
    pub capture_tool_calls: bool,
    pub capture_arguments: bool,
    pub redact_env_keys: Vec<String>,
    pub risk_signals: Vec<String>,
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
