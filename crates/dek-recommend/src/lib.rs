// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_capability_registry::{CapabilityStatus, DeviceCapabilities};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedAction {
    pub severity: Severity,
    pub title: String,
    pub description_th: String,
    pub cli_command: Option<String>,
}

pub struct Recommender;

impl Recommender {
    pub fn recommend(
        caps: &DeviceCapabilities,
        recent_stats: &dek_agent_observer::activity::ActivityCounts,
    ) -> Vec<RecommendedAction> {
        let mut recs = Vec::new();

        // 1. Missing OS PEP
        if caps.os.r#type == "linux" && caps.kernel.linux_ebpf.is_none() {
            recs.push(RecommendedAction {
                severity: Severity::Warning,
                title: "eBPF is missing".into(),
                description_th: "เครื่อง Linux นี้น่าจะยังไม่ได้ติดตั้งหรือเปิดใช้ eBPF ทำให้บล็อกเครือข่ายไม่ได้จริง"
                    .into(),
                cli_command: Some("pollek-dek doctor --fix ebpf".into()),
            });
        }

        // 2. High MCP traffic but no MCP proxy
        if recent_stats.mcp_invocations > 50 {
            let has_mcp_pep = caps
                .pep
                .iter()
                .any(|p| p.r#type.contains("mcp") && p.status == CapabilityStatus::Ready);
            if !has_mcp_pep {
                recs.push(RecommendedAction {
                    severity: Severity::Warning,
                    title: "MCP PEP not configured".into(),
                    description_th: "พบการเรียกใช้งาน AI Tools สูงมาก แต่คุณยังไม่ได้เปิดใช้ MCP PEP ทำให้ AI อาจเข้าถึงข้อมูลอันตรายได้".into(),
                    cli_command: Some("pollek-dek config set mcp.proxy.enabled true".into()),
                });
            }
        }

        recs
    }
}
