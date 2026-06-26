// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_capability_registry::DeviceCapabilities;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EnforcePlane {
    KernelEbpf,
    KernelWfp,
    KernelNeFilter,
    McpHttp,
    McpStdio,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EnforceCapability {
    Enforce,
    ObserveOnly,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PepIntent {
    NetworkEgress,
    McpToolCall { over_http: bool },
    ResourceAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepRoute {
    pub plane: EnforcePlane,
    pub capability: EnforceCapability,
    pub transport: String,
    pub reason_code: String,
    pub friendly_th: String,
    pub user_action_th: Option<String>,
}

pub fn route_pep(intent: PepIntent, caps: &DeviceCapabilities) -> PepRoute {
    match intent {
        PepIntent::NetworkEgress => route_network(caps),
        PepIntent::McpToolCall { over_http } => route_mcp(caps, over_http),
        PepIntent::ResourceAccess => route_mcp(caps, true),
    }
}

fn route_network(caps: &DeviceCapabilities) -> PepRoute {
    if caps.kernel.linux_ebpf.is_some() {
        return PepRoute {
            plane: EnforcePlane::KernelEbpf,
            capability: EnforceCapability::Enforce,
            transport: "ebpf".into(),
            reason_code: "enforced".into(),
            friendly_th: "บังคับใช้ระดับเคอร์เนลด้วย eBPF — บล็อกการเชื่อมต่อออกได้จริง".into(),
            user_action_th: None,
        };
    }
    if caps.kernel.macos_nefilter.is_some() {
        return PepRoute {
            plane: EnforcePlane::KernelNeFilter,
            capability: EnforceCapability::Enforce,
            transport: "nefilter".into(),
            reason_code: "enforced".into(),
            friendly_th: "บังคับใช้ผ่าน macOS System Extension (NEFilter) — บล็อกได้จริง".into(),
            user_action_th: None,
        };
    }
    if caps.kernel.windows_wfp.is_some() {
        return PepRoute {
            plane: EnforcePlane::KernelWfp,
            capability: EnforceCapability::ObserveOnly,
            transport: "wfp".into(),
            reason_code: "degraded_observe".into(),
            friendly_th: "บนเครื่องนี้ Windows WFP ทำงานแบบ 'สังเกตการณ์' เท่านั้น ยังไม่บล็อกจริง"
                .into(),
            user_action_th: Some(
                "ติดตั้ง WFP callout driver (ต้องสิทธิ์ Administrator) เพื่อเปิดการบังคับใช้จริง — ดู `pollek-dek doctor`"
                    .into(),
            ),
        };
    }
    PepRoute {
        plane: EnforcePlane::None,
        capability: EnforceCapability::Unavailable,
        transport: "none".into(),
        reason_code: "no_pep".into(),
        friendly_th: "ยังไม่มีตัวบังคับใช้ระดับเครือข่ายบนเครื่องนี้".into(),
        user_action_th: Some("เปิดใช้ eBPF (Linux) หรือ System Extension (macOS) เพื่อบังคับใช้จริง".into()),
    }
}

fn route_mcp(caps: &DeviceCapabilities, over_http: bool) -> PepRoute {
    let want = if over_http { "http" } else { "stdio" };
    let pep = caps.pep.iter().find(|p| {
        p.transports.iter().any(|t| t == want)
            && p.status == dek_capability_registry::CapabilityStatus::Ready
    });

    match pep {
        Some(p) => PepRoute {
            plane: if over_http {
                EnforcePlane::McpHttp
            } else {
                EnforcePlane::McpStdio
            },
            capability: EnforceCapability::Enforce,
            transport: want.into(),
            reason_code: "enforced".into(),
            friendly_th: format!(
                "บังคับใช้ที่ชั้น MCP ({}) ผ่าน `{}` — allow/deny/redact ได้จริง",
                want, p.r#type
            ),
            user_action_th: None,
        },
        None => PepRoute {
            plane: EnforcePlane::None,
            capability: EnforceCapability::Unavailable,
            transport: want.into(),
            reason_code: "no_pep".into(),
            friendly_th: format!("ยังไม่มี MCP PEP ({}) พร้อมใช้บนเครื่องนี้", want),
            user_action_th: Some(
                "ตั้งค่า agent ให้วิ่งผ่าน Pollek MCP proxy/wrapper เพื่อบังคับใช้ — ดูคู่มือ MCP setup".into(),
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dek_capability_registry::*;

    fn dummy_caps() -> DeviceCapabilities {
        DeviceCapabilities {
            device_id: "test".into(),
            dek_version: "test".into(),
            os: OsInfo {
                r#type: "linux".into(),
                version: "1".into(),
                arch: "x86".into(),
            },
            pdp: vec![],
            pep: vec![],
            plugins: vec![],
            kernel: KernelCapabilities {
                linux_ebpf: None,
                windows_wfp: None,
                macos_nefilter: None,
            },
        }
    }

    #[test]
    fn test_linux_ebpf() {
        let mut caps = dummy_caps();
        caps.kernel.linux_ebpf = Some(serde_json::json!({}));
        let route = route_pep(PepIntent::NetworkEgress, &caps);
        assert_eq!(route.plane, EnforcePlane::KernelEbpf);
        assert_eq!(route.capability, EnforceCapability::Enforce);
    }

    #[test]
    fn test_windows_wfp() {
        let mut caps = dummy_caps();
        caps.kernel.windows_wfp = Some(serde_json::json!({}));
        let route = route_pep(PepIntent::NetworkEgress, &caps);
        assert_eq!(route.plane, EnforcePlane::KernelWfp);
        assert_eq!(route.capability, EnforceCapability::ObserveOnly);
        assert!(route.user_action_th.is_some());
    }

    #[test]
    fn test_mcp_available() {
        let mut caps = dummy_caps();
        caps.pep.push(PepCapability {
            r#type: "mcp-proxy".into(),
            transports: vec!["stdio".into()],
            control_level: dek_domain_schema::control_level::ControlLevel::Enforce,
            status: CapabilityStatus::Ready,
            status_reason: None,
        });
        let route = route_pep(PepIntent::McpToolCall { over_http: false }, &caps);
        assert_eq!(route.plane, EnforcePlane::McpStdio);
        assert_eq!(route.capability, EnforceCapability::Enforce);
    }
}
