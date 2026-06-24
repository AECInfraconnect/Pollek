// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::deployment_session::{
    EnforcementLayer, LocalizedText, PdpEngine, RoutingPlan, UserActionKind,
};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub trait PepWarmCheck {
    async fn warm_check(&self, plan: &RoutingPlan) -> Result<(), String>;
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Ready,
    ReadyRequiresApproval,
    InstalledInactive,
    MissingPermission,
    MissingDriver,
    MissingBinary,
    UnsupportedOs,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeviceCapabilityReport {
    pub device_id: String,
    pub os: OsProfile,
    pub peps: Vec<PepCapabilityStatus>,
    pub pdps: Vec<PdpCapabilityStatus>,
    pub scanned_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OsProfile {
    pub r#type: String,
    pub version: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PepCapabilityStatus {
    pub layer: EnforcementLayer,
    pub status: CapabilityStatus,
    pub confidence: f32,
    pub detected_version: Option<String>,
    pub reason_code: String,
    pub user_message: LocalizedText,
    pub next_action: Option<UserActionKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PdpCapabilityStatus {
    pub engine: PdpEngine,
    pub status: CapabilityStatus,
    pub reason_code: String,
    pub user_message: LocalizedText,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EnforcementLevel {
    KernelEnforced,
    RedirectAdvisory,
    ObserveOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnforcementCapabilities {
    pub mcp_http_pep: bool,
    pub mcp_stdio_pep: bool,
    pub network_filter_user_mode: bool,
    pub network_filter_kernel: bool,
    pub network_enforcement_level: EnforcementLevel,
    pub dns_filter: bool,
    pub process_attribution: bool,
    pub ebpf_guardrail: bool,
    pub hot_reload_network_rules: bool,
    pub fail_closed_high_risk: bool,
}

impl EnforcementCapabilities {
    pub fn detect() -> Self {
        Self {
            mcp_http_pep: true,
            mcp_stdio_pep: true,
            #[cfg(windows)]
            network_filter_user_mode: false,
            #[cfg(windows)]
            network_filter_kernel: false,
            #[cfg(windows)]
            network_enforcement_level: EnforcementLevel::ObserveOnly,
            #[cfg(target_os = "macos")]
            network_filter_user_mode: false,
            #[cfg(target_os = "macos")]
            network_filter_kernel: false,
            #[cfg(target_os = "macos")]
            network_enforcement_level: EnforcementLevel::ObserveOnly,
            #[cfg(target_os = "linux")]
            network_filter_user_mode: false,
            #[cfg(target_os = "linux")]
            network_filter_kernel: true, // eBPF
            #[cfg(target_os = "linux")]
            network_enforcement_level: EnforcementLevel::KernelEnforced,
            #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
            network_filter_user_mode: false,
            #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
            network_filter_kernel: false,
            #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
            network_enforcement_level: EnforcementLevel::ObserveOnly,
            dns_filter: true,
            process_attribution: true,
            #[cfg(target_os = "linux")]
            ebpf_guardrail: true,
            #[cfg(not(target_os = "linux"))]
            ebpf_guardrail: false,
            hot_reload_network_rules: true,
            fail_closed_high_risk: true,
        }
    }
}

impl Default for EnforcementCapabilities {
    fn default() -> Self {
        Self::detect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceRegistrationRequest {
    pub device_id: String,
    pub os: String,
    pub dek_version: String,
    pub capabilities: EnforcementCapabilities,
}
