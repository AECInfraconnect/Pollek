// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capabilities::CapabilityStatus;
use crate::control_level::ControlLevel;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeploymentSession {
    pub deployment_id: String,
    pub policy_id: String,
    pub policy_version: String,
    pub requested_control_level: ControlLevel,
    pub target_scope: DeploymentScope,
    pub status: DeploymentSessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentSessionStatus {
    Planning,
    WaitingForUserAction,
    Deploying,
    Active,
    ActiveObserveOnly,
    PartiallyActive,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentScope {
    Agent { agent_id: String },
    Entity { entity_id: String },
    AgentGroup { group_id: String },
    Device { device_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeploymentEvent {
    pub event_id: String,
    pub deployment_id: String,
    pub agent_id: Option<String>,
    pub entity_id: Option<String>,
    pub policy_id: String,
    pub phase: DeploymentPhase,
    pub status: EventStatus,
    pub title: LocalizedText,
    pub detail: LocalizedText,
    pub technical_detail: Option<serde_json::Value>,
    pub user_action: Option<UserAction>,
    pub created_at: DateTime<Utc>,
    pub correlation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentPhase {
    AgentDiscovery,
    CapabilityCheck,
    RoutePlanning,
    PolicyCompile,
    BundleSign,
    PepDeploy,
    PdpHealthCheck,
    WarmCheck,
    Enforcement,
    Observe,
    TelemetrySync,
    Rollback,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    Info,
    Success,
    Warning,
    Error,
    ActionRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LocalizedText {
    pub en: String,
    pub th: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserAction {
    pub action_id: String,
    pub kind: UserActionKind,
    pub label: LocalizedText,
    pub help: LocalizedText,
    pub safe_to_retry: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserActionKind {
    ApproveConfigPatch,
    ApproveSystemExtension,
    InstallDriver,
    StartLocalService,
    ConnectCloud,
    RebuildPolicyBundle,
    RetryDeployment,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoutingPlan {
    pub deployment_id: String,
    pub agent_id: String,
    pub selected_pep: PepSelection,
    pub selected_pdp: PdpSelection,
    pub effective_control_level: ControlLevel,
    pub observability_path: ObservabilityPath,
    pub fallback: FallbackPlan,
    pub user_messages: Vec<UserMessage>,
    pub required_actions: Vec<UserAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PepSelection {
    pub pep_id: String,
    pub layer: EnforcementLayer,
    pub status: CapabilityStatus,
    pub reason_code: String,
    pub reason: LocalizedText,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PdpSelection {
    pub pdp_id: String,
    pub engine: PdpEngine,
    pub mode: PdpRouteMode,
    pub status: CapabilityStatus,
    pub reason_code: String,
    pub reason: LocalizedText,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EnforcementLayer {
    McpProxy,
    McpStdioWrapper,
    HttpProxy,
    EbpfNetwork,
    WindowsWfp,
    MacosNetworkExtension,
    BrowserExtension,
    ObserveOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PdpEngine {
    Cedar,
    OpaWasm,
    OpenFga,
    Cloud,
    RouterOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PdpRouteMode {
    LocalOnly,
    LocalPrimaryRemoteFallback,
    CloudPrimaryLocalFallback,
    MirrorAuditOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObservabilityPath {
    SecureSpoolAndOtel,
    LocalOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct FallbackPlan {
    pub enabled: bool,
    pub fallback_pep: Option<EnforcementLayer>,
    pub fallback_pdp: Option<PdpEngine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserMessage {
    pub message_id: String,
    pub text: LocalizedText,
    pub severity: EventStatus,
}
