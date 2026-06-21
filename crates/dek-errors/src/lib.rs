use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorDomain {
    Enrollment,
    Identity,
    Mtls,
    Config,
    Bundle,
    Activation,
    Policy,
    Pdp,
    Pep,
    Wasm,
    Telemetry,
    Storage,
    Update,
    Ebpf,
    Platform,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RetryClass {
    NoRetry,
    RetryImmediate,
    RetryWithBackoff,
    RetryAfterReauth,
    RetryAfterAdminAction,
    FatalRequiresReinstall,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SafetyAction {
    DenyRequest,
    KeepLastKnownGood,
    EnterObserveOnly,
    EnterDegradedMode,
    StopService,
    QuarantineArtifact,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ErrorEnvelope {
    pub error_id: String,
    pub domain: ErrorDomain,
    pub code: String,
    pub message: String,
    pub safe_message: String,
    pub retry_class: RetryClass,
    pub safety_action: SafetyAction,
    pub tenant_id: Option<String>,
    pub device_id: Option<String>,
    pub bundle_version: Option<String>,
    pub request_id: Option<String>,
    pub timestamp: String,
    pub remediation: Option<String>,
}

impl std::fmt::Display for ErrorEnvelope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?}] {}: {}", self.domain, self.code, self.message)
    }
}

impl std::error::Error for ErrorEnvelope {}
