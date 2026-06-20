use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObjectMeta {
    pub schema_version: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub environment_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: String,
    pub updated_by: String,
    pub source: RegistrationSource,
    pub status: RegistryStatus,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationSource {
    Manual,
    Discovery,
    Import,
    CloudSync,
    AgentSelfRegistration,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RegistryStatus {
    Discovered,
    PendingApproval,
    Registered,
    Active,
    Suspended,
    Deleted,
    Draft,
    Compiled,
    Published,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AiAgent {
    pub meta: ObjectMeta,
    pub agent_id: String,
    pub name: String,
    pub agent_type: AgentType,
    pub vendor: Option<String>,
    pub runtime: AgentRuntime,
    pub entrypoints: Vec<AgentEntrypoint>,
    pub declared_tools: Vec<String>,
    pub declared_resources: Vec<String>,
    pub identity: AgentIdentity,
    pub trust_level: TrustLevel,
    pub capabilities: Vec<String>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    ClaudeDesktop,
    OpenAIAgent,
    LangChainAgent,
    LlamaIndexAgent,
    CustomMcpClient,
    BrowserAgent,
    CliAgent,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentRuntime {
    pub runtime_name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentEntrypoint {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AgentIdentity {
    pub spiffe_id: Option<String>,
    pub process_path: Option<String>,
    pub user_subject: Option<String>,
    pub signing_key_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Untrusted,
    Low,
    Medium,
    High,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct McpServer {
    pub meta: ObjectMeta,
    pub server_id: String,
    pub name: String,
    pub transport: McpTransport,
    pub endpoint: String,
    pub owner_agent_id: Option<String>,
    pub tools: Vec<String>,
    pub resources: Vec<String>,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    Stdio,
    Http,
    Sse,
    WebSocket,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Tool {
    pub meta: ObjectMeta,
    pub tool_id: String,
    pub mcp_server_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    pub output_schema: Option<serde_json::Value>,
    pub side_effect_level: SideEffectLevel,
    pub data_access_level: DataAccessLevel,
    pub risk_level: RiskLevel,
    pub category: ToolCategory,
    pub source: ToolSource,
    pub declared_by_agent_ids: Vec<String>,
    pub observed_by_agent_ids: Vec<String>,
    pub reachable_resource_ids: Vec<String>,
    pub required_entitlements: Vec<String>,
    pub schema_fingerprint: SchemaFingerprint,
    pub policy_coverage: PolicyCoverageSummary,
    pub pep_bindings: Vec<PepBinding>,
    pub last_seen_at: Option<String>,
    pub first_seen_at: Option<String>,
    pub observation_count_24h: u64,
    pub deny_count_24h: u64,
    pub allow_count_24h: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectLevel {
    None,
    Local,
    Network,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DataAccessLevel {
    None,
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Resource {
    pub meta: ObjectMeta,
    pub resource_id: String,
    pub resource_type: ResourceType,
    pub name: String,
    pub uri: String,
    pub classification: DataClassification,
    pub owner_entity_id: Option<String>,
    pub attributes: HashMap<String, serde_json::Value>,
    pub data_boundary: ResourceDataBoundary,
    pub data_tags: Vec<String>,
    pub pii_types: Vec<String>,
    pub secret_types: Vec<String>,
    pub allowed_actions: Vec<ResourceAction>,
    pub observed_actions: Vec<ResourceAction>,
    pub observed_by_agent_ids: Vec<String>,
    pub reachable_via_tool_ids: Vec<String>,
    pub policy_coverage: PolicyCoverageSummary,
    pub pep_bindings: Vec<PepBinding>,
    pub first_seen_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub access_count_24h: u64,
    pub violation_count_24h: u64,
    pub cost_relevant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    File,
    Database,
    ApiEndpoint,
    McpResource,
    VectorStore,
    Topic,
    Queue,
    Device,
    Secret,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DataClassification {
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Entity {
    pub meta: ObjectMeta,
    pub entity_id: String,
    pub entity_type: EntityType,
    pub display_name: String,
    pub external_ids: Vec<ExternalId>,
    pub roles: Vec<String>,
    pub attributes: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    HumanUser,
    ServiceAccount,
    Workload,
    AiAgent,
    Organization,
    Tenant,
    Device,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExternalId {
    pub provider: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Relationship {
    pub meta: ObjectMeta,
    pub relationship_id: String,
    pub subject: RelationshipRef,
    pub relation: String,
    pub object: RelationshipRef,
    pub conditions: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RelationshipRef {
    pub object_type: String,
    pub object_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlackboxProviderType {
    OpenAiCompatible,
    Ollama,
    HuggingFaceEndpoint,
    AzureOpenAi,
    AnthropicCompatible,
    LocalModelServer,
    CustomHttp,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BlackboxModelRef {
    pub model_id: String,
    pub display_name: String,
    pub context_window: Option<u32>,
    pub pii_allowed: bool,
    pub max_latency_ms: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DataBoundary {
    LocalOnly,
    PrivateNetwork,
    ExternalCloud,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BlackboxAiProvider {
    pub meta: ObjectMeta,
    pub provider_id: String,
    pub name: String,
    pub provider_type: BlackboxProviderType,
    pub endpoint: Option<String>,
    pub model_catalog: Vec<BlackboxModelRef>,
    pub supported_tasks: Vec<String>,
    pub data_boundary: DataBoundary,
    pub auth_ref: Option<String>,
    pub risk_level: RiskLevel,
    pub labels: HashMap<String, String>,
}

// New Enums and Structs from Deep Research Implementation Plan

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    Mcp,
    Cli,
    Browser,
    Database,
    FileSystem,
    Network,
    SecretManager,
    CodeExecution,
    BusinessApi,
    LocalModel,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolSource {
    Registry,
    McpDiscovery,
    ProcessDiscovery,
    ConfigDiscovery,
    NetworkObservation,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SchemaFingerprint {
    pub input_schema_hash: String,
    pub output_schema_hash: Option<String>,
    pub descriptor_hash: Option<String>,
    pub previous_descriptor_hash: Option<String>,
    pub drift_status: DriftStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DriftStatus {
    Stable,
    New,
    Changed,
    Suspicious,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResourceDataBoundary {
    LocalOnly,
    PrivateNetwork,
    TenantCloud,
    ExternalCloud,
    PublicInternet,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResourceAction {
    Read,
    Write,
    Delete,
    Execute,
    Connect,
    Query,
    Publish,
    Subscribe,
    Upload,
    Download,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyCoverageSummary {
    pub status: CoverageStatus,
    pub policy_ids: Vec<String>,
    pub missing_controls: Vec<MissingControl>,
    pub last_simulated_at: Option<String>,
    pub last_enforced_at: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Uncovered,
    ObserveOnly,
    Partial,
    Enforced,
    Broken,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MissingControl {
    pub control_type: String,
    pub reason: String,
    pub recommended_policy_type: String,
    pub recommended_pep_type: PepType,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PepBinding {
    pub pep_type: PepType,
    pub deployment_status: DeploymentStatus,
    pub rule_ids: Vec<String>,
    pub capabilities: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PepType {
    McpProxy,
    StdioWrapper,
    HttpGateway,
    Envoy,
    Istio,
    EmbeddedSdk,
    LinuxEbpf,
    WindowsWfp,
    MacosNeFilter,
    BrowserExtension,
    FileSystemWatcher,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentStatus {
    NotAvailable,
    Available,
    ObserveOnly,
    Enforcing,
    Failed,
}
