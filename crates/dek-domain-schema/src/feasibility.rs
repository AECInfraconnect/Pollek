// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProductMode {
    DesktopSimple,
    DesktopAdvanced,
    EnterpriseServer,
    SovereignAirgap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PolicyIntent {
    ObserveAgentActivity,
    ApproveRiskyToolCalls,
    BlockSpecificTools,
    RedactSensitiveParameters,
    BlockSensitiveFileUpload,
    BlockUnknownNetworkDestinations,
    RestrictLocalModelUsage,
    LimitTokenOrCostUsage,
    RequireEntityRelationship,
    DetectPromptInjection,
    KillSwitchOnAnomaly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PolicyFeasibilityStatus {
    CanEnforceNow,
    CanEnforceAfterApproval,
    CanPartiallyEnforce,
    CanObserveOnly,
    NeedsSetup,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyFeasibilityRequest {
    pub policy_id: Option<String>,
    pub policy_intent: PolicyIntent,
    pub requested_control_level: ControlLevel,
    pub targets: Vec<PolicyTarget>,
    pub mode: ProductMode,
}

use crate::control_level::ControlLevel;
use crate::deployment_session::LocalizedText;
use crate::policy_target::PolicyTarget;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FallbackBehavior {
    DowngradeToObserve,
    WarnThenObserve,
    RequireUserSetup,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoutePreview {
    pub user_control_method: ControlMethod,
    pub advanced_pep: Option<InternalPep>,
    pub advanced_pdp: Option<InternalPdp>,
    pub fallback: FallbackBehavior,
    pub warm_check_required: bool,
    pub explanation: LocalizedText,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ControlMethod {
    AgentToolControl,
    AgentConfigWrapper,
    LocalApiControl,
    BrowserActivityMonitor,
    SystemNetworkControl,
    ProcessObservation,
    ObserveOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InternalPep {
    McpProxy,
    McpStdioWrapper,
    HttpProxy,
    BrowserExtension,
    LinuxEbpf,
    WindowsWfp,
    MacosNetworkExtension,
    SecureSpoolObserver,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InternalPdp {
    Cedar,
    OpaWasm,
    OpenFga,
    Cloud,
    RouterOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct Enforceability {
    pub can_observe: bool,
    pub can_warn: bool,
    pub can_require_approval: bool,
    pub can_enforce: bool,
    pub can_strict_deny: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiagnosticFinding {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ControlMethodPlan {
    pub method: ControlMethod,
    pub internal_pep: InternalPep,
    pub internal_pdp: InternalPdp,
    pub enforceability: Enforceability,
    pub reason_code: String,
    pub explanation: LocalizedText,
    pub diagnostics: Vec<DiagnosticFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequiredUserAction {
    pub kind: String,
    pub label: LocalizedText,
}

impl RequiredUserAction {
    pub fn install_system_component(component: &str) -> Self {
        Self {
            kind: format!("install_{}", component),
            label: LocalizedText {
                en: format!("Install {}", component),
                th: format!("ติดตั้ง {}", component),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyFeasibilityResult {
    pub feasibility_id: String,
    pub target: PolicyTarget,
    pub policy_intent: PolicyIntent,
    pub requested_control_level: ControlLevel,
    pub effective_control_level: ControlLevel,
    pub status: PolicyFeasibilityStatus,
    pub summary: LocalizedText,
    pub detail: LocalizedText,
    pub required_actions: Vec<RequiredUserAction>,
    pub route_preview: RoutePreview,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RequiredCapabilityLevel {
    Observe,
    Warn,
    Approval,
    Enforce,
    StrictDeny,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CapabilityRequirement {
    pub method: ControlMethod,
    pub minimum: RequiredCapabilityLevel,
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PolicyPresetTemplate {
    pub template_id: String,
    pub display_name: LocalizedText,
    pub description: LocalizedText,
    pub intent: PolicyIntent,
    pub supported_control_levels: Vec<ControlLevel>,
    pub required_capabilities: Vec<CapabilityRequirement>,
    pub preferred_methods: Vec<ControlMethod>,
    pub fallback_allowed: bool,
    pub default_for_desktop: bool,
    pub default_for_enterprise: bool,
}
