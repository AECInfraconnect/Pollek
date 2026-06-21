use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnforcementCapabilities {
    pub mcp_http_pep: bool,
    pub mcp_stdio_pep: bool,
    pub network_filter_user_mode: bool,
    pub network_filter_kernel: bool,
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
            network_filter_kernel: true, // WFP
            #[cfg(target_os = "macos")]
            network_filter_user_mode: true, // NetworkExtension
            #[cfg(target_os = "macos")]
            network_filter_kernel: false,
            #[cfg(target_os = "linux")]
            network_filter_user_mode: false,
            #[cfg(target_os = "linux")]
            network_filter_kernel: true, // eBPF
            #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
            network_filter_user_mode: false,
            #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
            network_filter_kernel: false,
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
