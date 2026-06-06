use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcMessage<T> {
    pub version: String,
    pub payload: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub uptime_seconds: u64,
    pub ebpf_active: bool,
    pub active_bundle_version: Option<String>,
    pub update_state: String,
    pub core_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcRequest {
    HealthCheck,
    ReloadConfig,
    Status,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcResponse {
    HealthStatus {
        status: String,
        core_version: String,
    },
    ReloadStatus {
        status: String,
    },
    ServiceStatus(ServiceStatus),
    Error(String),
}
