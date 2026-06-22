// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ControlMode {
    Observe,
    Warn,
    Approval,
    Enforce,
    StrictDeny,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PresetCategory {
    ContentGuard,
    PiiAndSecrets,
    FileSystem,
    PersonalResources,
    McpTools,
    NetworkAndProviders,
    CostAndTokens,
    AuditAndCompliance,
    ApprovalWorkflow,
    // Keep legacy categories for migration if needed, but the plan only lists these 9.
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskTag {
    PromptInjection,
    SensitiveInfoDisclosure,
    InsecurePluginDesign,
    ExcessiveAgency,
    ModelDosCostSpike,
    ShadowAi,
    DataExfiltration,
    SecretLeakage,
    UnsafeFileAccess,
    UnsafeNetworkEgress,
    ToolPoisoning,
    UnauthorizedAccess,
    FinancialRisk,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PepType {
    McpProxy,
    StdioWrapper,
    HttpGateway,
    LinuxEbpf,
    WindowsWfp,
    MacosNetworkExtension,
    FileSystemPep,
    BrowserExtension,
    LocalModelProxy,
    CloudConnectorProxy,
    EmbeddedSdk,
    TelemetryOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyOutputKind {
    Rego,
    Cedar,
    OpenFgaModel,
    PepConfig,
    RouterRule,
    RedactionPipeline,
    ApprovalWorkflow,
    TelemetryRule,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    PolicyDraft,
    SignedBundle,
    PepBinding,
    PdpRouteRule,
    ResourceScope,
    ApprovalRule,
    TelemetrySubscription,
    RollbackSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PresetParameter {
    pub key: String,
    pub label: String,
    pub description: String,
    pub value_type: PresetValueType,
    pub required: bool,
    pub default_value: serde_json::Value,
    pub examples: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PresetValueType {
    String,
    Integer,
    Float,
    Boolean,
    StringList,
    PathList,
    GlobList,
    ProviderList,
    AgentSelector,
    ToolSelector,
    ResourceSelector,
    Duration,
    Money,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TelemetryRequirement {
    pub event_type: String,
    pub required_fields: Vec<String>,
    pub pii_handling: PiiHandling,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PiiHandling {
    None,
    Hash,
    Redact,
    LocalOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum SimulationWindow {
    Last24Hours,
    Last7Days,
    Last30Days,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyPresetV2 {
    pub id: String,
    pub version: String,
    pub title: String,
    pub short_description: String,
    pub long_description: String,
    pub category: PresetCategory,
    pub risk_tags: Vec<RiskTag>,
    pub supported_pep_types: Vec<PepType>,
    pub recommended_pep_types: Vec<PepType>,
    pub supported_control_modes: Vec<ControlMode>,
    pub default_control_mode: ControlMode,
    pub supported_policy_outputs: Vec<PolicyOutputKind>,
    pub parameters: Vec<PresetParameter>,
    pub generated_artifacts: Vec<ArtifactKind>,
    pub telemetry_requirements: Vec<TelemetryRequirement>,
    pub default_simulation_window: SimulationWindow,
    pub safety_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeployPresetRequest {
    pub preset_id: String,
    pub preset_version: Option<String>,
    pub control_mode: ControlMode,
    pub selected_pep_types: Vec<PepType>,
    #[serde(default)]
    pub targets: PresetTargets,
    #[serde(default)]
    pub params: BTreeMap<String, serde_json::Value>,
    #[serde(default = "default_true")]
    pub dry_run_first: bool,
    pub pdp_route: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct PresetTargets {
    #[serde(default)]
    pub agent_ids: Vec<String>,
    #[serde(default)]
    pub tool_ids: Vec<String>,
    #[serde(default)]
    pub resource_ids: Vec<String>,
    #[serde(default)]
    pub provider_ids: Vec<String>,
    #[serde(default)]
    pub path_scopes: Vec<PathScope>,
    #[serde(default)]
    pub account_scopes: Vec<AccountScope>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PathScope {
    pub root_path: String,
    #[serde(default)]
    pub include_globs: Vec<String>,
    #[serde(default)]
    pub exclude_globs: Vec<String>,
    pub operations: Vec<FileOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileOperation {
    Read,
    Write,
    Create,
    Delete,
    Rename,
    Execute,
    List,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AccountScope {
    pub provider: String,
    pub account_id: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedArtifact {
    pub language: String,
    pub content: String,
    pub warnings: Vec<String>,
}

impl RenderedArtifact {
    pub fn rego(name: &str, content: String) -> Self {
        Self {
            language: "rego".into(),
            content: format!("# {}\n{}", name, content),
            warnings: vec![],
        }
    }

    pub fn cedar(name: &str, content: String) -> Self {
        Self {
            language: "cedar".into(),
            content: format!("// {}\n{}", name, content),
            warnings: vec![],
        }
    }

    pub fn openfga(name: &str, content: String) -> Self {
        Self {
            language: "openfga".into(),
            content: format!("// {}\n{}", name, content),
            warnings: vec![],
        }
    }

    pub fn pep_config(content: String) -> Self {
        Self {
            language: "json".into(),
            content,
            warnings: vec![],
        }
    }

    pub fn router_rule(content: String) -> Self {
        Self {
            language: "json".into(),
            content,
            warnings: vec![],
        }
    }

    pub fn telemetry(content: String) -> Self {
        Self {
            language: "json".into(),
            content,
            warnings: vec![],
        }
    }
}
