use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceCapabilities {
    pub device_id: String,
    pub dek_version: String,
    pub os: OsInfo,
    pub pdp: Vec<PdpCapability>,
    pub pep: Vec<PepCapability>,
    pub plugins: Vec<PluginCapability>,
    pub kernel: KernelCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OsInfo {
    pub r#type: String,
    pub version: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PdpCapability {
    pub r#type: String,
    pub version: Option<String>,
    pub mode: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PepCapability {
    pub r#type: String,
    #[serde(default)]
    pub transports: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginCapability {
    pub id: String,
    pub abi: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelCapabilities {
    pub linux_ebpf: Option<serde_json::Value>,
    pub windows_wfp: Option<serde_json::Value>,
    pub macos_nefilter: Option<serde_json::Value>,
}

impl DeviceCapabilities {
    pub fn has_os_l4_ready(&self) -> bool {
        self.kernel.linux_ebpf.is_some()
            || self.kernel.windows_wfp.is_some()
            || self.kernel.macos_nefilter.is_some()
    }
}
