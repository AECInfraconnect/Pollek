use serde::{Deserialize, Serialize};

pub mod contract;
pub use contract::{
    dek_version, evaluate_compatibility, CompatibilityStatus, CompatibilityVerdict, DekContract,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PollekPolicyBundle {
    pub api_version: String,
    pub kind: String,
    pub metadata: BundleMetadata,
    pub compatibility: BundleCompatibility,
    pub artifacts: Vec<BundleArtifact>,
    pub activation: ActivationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BundleMetadata {
    pub bundle_id: String,
    pub tenant: String,
    pub version: String,
    pub created_at: String,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BundleCompatibility {
    pub min_dek_version: String,
    pub required_crates: Vec<String>,
    pub required_pep_types: Vec<String>,
    pub required_os_modules: OsModulesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OsModulesConfig {
    #[serde(default)]
    pub linux: Vec<String>,
    #[serde(default)]
    pub windows: Vec<String>,
    #[serde(default)]
    pub macos: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BundleArtifact {
    pub r#type: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivationConfig {
    pub strategy: String,
    pub rollback_on_failure: bool,
    pub health_check_timeout_ms: u64,
    pub shadow_before_enforce_seconds: u64,
}
