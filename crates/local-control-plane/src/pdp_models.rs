// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PdpRuntimeCategory {
    LocalEngine,
    ExternalConnector,
    PollenCloud,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PdpKind {
    OpaWasm,
    CedarLocal,
    WasmPlugin,
    OpaServer,
    OpenfgaServer,
    CedarHttp,
    AwsVerifiedPermissions,
    CustomHttp,
    CustomGrpc,
    PollenCloudPdp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PdpStatus {
    NotInstalled,
    Installed,
    Loading,
    Ready,
    Reachable,
    Unreachable,
    Degraded,
    Error,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PdpRouteMode {
    LocalOnly,
    LocalPrimaryRemoteFallback,
    RemotePrimaryLocalFallback,
    CloudPrimaryLocalFallback,
    ShadowRemote,
    MirrorAuditOnly,
    StrictRemote,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PdpFailureBehavior {
    Deny,
    Allow,
    Fallback,
    LastKnownGood,
    NotApplicable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdpCapability {
    pub name: String,
    pub version: String,
    pub supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdpHealthSnapshot {
    pub health: String,
    pub readiness: String,
    pub last_checked_at: Option<String>,
    pub last_decision_probe_at: Option<String>,
    pub latency_ms: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdpRuntime {
    pub id: String,
    pub name: String,
    pub category: PdpRuntimeCategory,
    pub kind: PdpKind,
    pub enabled: bool,
    pub status: PdpStatus,
    pub endpoint: Option<String>,
    pub auth_ref: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<PdpCapability>,
    pub health: Option<PdpHealthSnapshot>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMatch {
    pub agent_ids: Option<Vec<String>>,
    pub resource_ids: Option<Vec<String>>,
    pub protocols: Option<Vec<String>>,
    pub policy_tags: Option<Vec<String>>,
    pub sensitivity: Option<Vec<String>>,
    pub environment: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdpRouteRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    #[serde(rename = "match")]
    pub match_cond: RouteMatch,
    pub mode: PdpRouteMode,
    pub primary_pdp_id: String,
    #[serde(default)]
    pub fallback_pdp_ids: Vec<String>,
    #[serde(default)]
    pub shadow_pdp_ids: Vec<String>,
    pub merge_strategy: String,
    pub failure_behavior: PdpFailureBehavior,
    pub timeout_ms: u64,
    pub max_retries: u32,
}

pub fn normalize_pdp_kind(input: &str) -> Result<PdpKind, String> {
    let value = input.trim().to_lowercase().replace("-", "_");
    match value.as_str() {
        "opa" | "opa_server" => Ok(PdpKind::OpaServer),
        "opa_wasm" => Ok(PdpKind::OpaWasm),
        "openfga" | "open_fga" | "openfga_server" => Ok(PdpKind::OpenfgaServer),
        "cedar" | "cedar_http" => Ok(PdpKind::CedarHttp),
        "cedar_local" => Ok(PdpKind::CedarLocal),
        "aws_avp" | "aws_verified_permissions" => Ok(PdpKind::AwsVerifiedPermissions),
        "custom_http" => Ok(PdpKind::CustomHttp),
        "custom_grpc" => Ok(PdpKind::CustomGrpc),
        "pollen_cloud" | "pollen_cloud_pdp" => Ok(PdpKind::PollenCloudPdp),
        "wasm_plugin" => Ok(PdpKind::WasmPlugin),
        _ => Err(format!("Unsupported PDP kind: {}", input)),
    }
}
