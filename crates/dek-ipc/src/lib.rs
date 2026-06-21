// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

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
    RotateIdentity,
    FingerprintAction { action: String, payload: Option<Vec<u8>>, sig: Option<String> },
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
    RotateStatus {
        status: String,
    },
    ServiceStatus(ServiceStatus),
    FingerprintStatus { version: u64, message: String },
    Error(String),
}
