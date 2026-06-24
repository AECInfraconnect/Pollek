// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_domain_schema::deployment_session::LocalizedText;

pub struct MessageCatalog;

impl MessageCatalog {
    pub fn pep_selected_mcp_stdio(agent_name: &str) -> LocalizedText {
        LocalizedText {
            en: format!(
                "POLLEK selected MCP Stdio Wrapper for {agent_name} because this agent uses MCP over stdio. You need to approve a config update before enforcement starts."
            ),
            th: format!(
                "ระบบเลือก MCP Stdio Wrapper สำหรับ {agent_name} เพราะ Agent นี้ใช้ MCP ผ่าน stdio คุณต้องอนุมัติการแก้ไข config ก่อนเริ่ม enforcement"
            ),
        }
    }

    pub fn pep_selected_mcp_http(agent_name: &str) -> LocalizedText {
        LocalizedText {
            en: format!(
                "POLLEK selected MCP Proxy for {agent_name} because this agent uses MCP over HTTP."
            ),
            th: format!("ระบบเลือก MCP Proxy สำหรับ {agent_name} เพราะ Agent นี้ใช้ MCP ผ่าน HTTP"),
        }
    }

    pub fn pep_selected_network(agent_name: &str) -> LocalizedText {
        LocalizedText {
            en: format!("Selected the OS network enforcement layer for {agent_name}."),
            th: format!("เลือกชั้นควบคุม network ของ OS สำหรับ {agent_name}"),
        }
    }

    pub fn pep_fallback_observe_only(agent_name: &str) -> LocalizedText {
        LocalizedText {
            en: format!(
                "POLLEK cannot safely enforce {agent_name} yet. It will observe activity and show setup recommendations."
            ),
            th: format!(
                "ระบบยังไม่สามารถ enforce {agent_name} ได้อย่างปลอดภัย จึงจะ Observe ก่อนและแสดงคำแนะนำการตั้งค่า"
            ),
        }
    }

    pub fn pdp_selected_cedar() -> LocalizedText {
        LocalizedText {
            en: "Selected Cedar local engine because this policy is a standard local allow/deny rule.".into(),
            th: "เลือก Cedar local engine เพราะ policy นี้เป็นกฎ allow/deny แบบ local มาตรฐาน".into(),
        }
    }

    pub fn pdp_selected_opa() -> LocalizedText {
        LocalizedText {
            en: "Selected OPA WASM because this policy needs complex conditional logic.".into(),
            th: "เลือก OPA WASM เพราะ policy นี้ต้องใช้ logic เงื่อนไขซับซ้อน".into(),
        }
    }

    pub fn pdp_selected_openfga() -> LocalizedText {
        LocalizedText {
            en: "Selected OpenFGA because this policy depends on entity relationships.".into(),
            th: "เลือก OpenFGA เพราะ policy นี้ตรวจสิทธิ์จากความสัมพันธ์ของ entity".into(),
        }
    }

    pub fn pdp_selected_cloud() -> LocalizedText {
        LocalizedText {
            en: "Selected Cloud PDP with local fallback because this strict policy should stay aligned with central governance.".into(),
            th: "เลือก Cloud PDP พร้อม local fallback เพราะ policy ระดับ Strict ควรตรงกับ governance ส่วนกลาง".into(),
        }
    }

    pub fn pdp_observe_only() -> LocalizedText {
        LocalizedText {
            en: "Selected observe-only routing because no active enforcement layer is ready."
                .into(),
            th: "เลือก observe-only routing เพราะยังไม่มี enforcement layer ที่พร้อมใช้งาน".into(),
        }
    }

    pub fn enforcement_active(agent_name: &str, layer: &str) -> LocalizedText {
        LocalizedText {
            en: format!("Enforcement is active for {agent_name} through {layer}."),
            th: format!("Enforcement สำหรับ {agent_name} เริ่มทำงานแล้วผ่าน {layer}"),
        }
    }
}
