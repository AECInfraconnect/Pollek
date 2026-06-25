// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_capability_registry::{snapshot::CapabilityStatus, LocalCapabilitySnapshot};
use dek_domain_schema::{
    capability_inventory::AgentCapabilityInventory,
    control_level::ControlLevel,
    deployment_session::LocalizedText,
    feasibility::{ControlMethod, PolicyFeasibilityStatus, RequiredUserAction},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedPolicy {
    pub suggestion_id: String,
    pub policy_template_id: String,
    pub display_name: LocalizedText,
    pub description: LocalizedText,
    pub target_agent_ids: Vec<String>,
    pub recommended_control_level: ControlLevel,
    pub feasibility: PolicyFeasibilityStatus,
    pub confidence: f32,
    pub reason_codes: Vec<String>,
    pub setup_required: Vec<RequiredUserAction>,
}

pub trait PolicySuggestionEngine {
    fn suggest(&self, snapshot: &LocalCapabilitySnapshot) -> Vec<SuggestedPolicy>;
}

pub struct FeasibilitySuggester;

impl PolicySuggestionEngine for FeasibilitySuggester {
    fn suggest(&self, snapshot: &LocalCapabilitySnapshot) -> Vec<SuggestedPolicy> {
        let mut suggestions = Vec::new();
        for agent in &snapshot.agents {
            suggestions.extend(suggest_for_agent(agent, &snapshot.methods));
        }
        suggestions
    }
}

pub fn suggest_for_agent(
    agent: &AgentCapabilityInventory,
    caps: &[dek_capability_registry::snapshot::ControlMethodCapability],
) -> Vec<SuggestedPolicy> {
    let mut suggestions = Vec::new();

    let has_mcp = !agent.mcp_surfaces.is_empty();
    // In a real app we'd check if the endpoint actually matches openai
    let has_local_api = !agent.model_endpoints.is_empty();

    let network_ready = caps.iter().any(|c| {
        c.method == ControlMethod::SystemNetworkControl && c.status == CapabilityStatus::Ready
    });

    if has_mcp {
        suggestions.push(SuggestedPolicy {
            suggestion_id: format!("sugg-mcp-risky-{}", agent.agent_id),
            policy_template_id: "approve_risky_tool_calls".into(),
            display_name: LocalizedText {
                en: "Approve risky tool calls".into(),
                th: "อนุมัติการเรียกใช้ Tool ที่มีความเสี่ยง".into(),
            },
            description: LocalizedText {
                en: "Agent exposes MCP tools, so POLLEK can review tool calls before execution."
                    .into(),
                th: "Agent นี้มี MCP tools ระบบจึงสามารถให้ตรวจและอนุมัติ tool call ก่อนทำงานได้".into(),
            },
            target_agent_ids: vec![agent.agent_id.clone()],
            recommended_control_level: ControlLevel::Approval,
            feasibility: PolicyFeasibilityStatus::CanEnforceAfterApproval,
            confidence: 0.9,
            reason_codes: vec!["mcp_surface_detected".into()],
            setup_required: vec![],
        });

        suggestions.push(SuggestedPolicy {
            suggestion_id: format!("sugg-mcp-redact-{}", agent.agent_id),
            policy_template_id: "redact_sensitive_parameters".into(),
            display_name: LocalizedText {
                en: "Redact sensitive parameters".into(),
                th: "ซ่อนข้อมูลอ่อนไหวในพารามิเตอร์".into(),
            },
            description: LocalizedText {
                en: "Agent tool parameters can be inspected and redacted before execution.".into(),
                th: "ระบบสามารถตรวจและ redact parameter ของ tool ก่อน execution ได้".into(),
            },
            target_agent_ids: vec![agent.agent_id.clone()],
            recommended_control_level: ControlLevel::Enforce,
            feasibility: PolicyFeasibilityStatus::CanEnforceAfterApproval,
            confidence: 0.9,
            reason_codes: vec!["mcp_surface_detected".into()],
            setup_required: vec![],
        });
    }

    if has_local_api {
        suggestions.push(SuggestedPolicy {
            suggestion_id: format!("sugg-api-limit-{}", agent.agent_id),
            policy_template_id: "limit_token_or_cost_usage".into(),
            display_name: LocalizedText {
                en: "Limit token or cost usage".into(),
                th: "จำกัดปริมาณ Token หรือค่าใช้จ่าย".into(),
            },
            description: LocalizedText {
                en: "Local API traffic can be measured and rate-limited.".into(),
                th: "ระบบสามารถวัดและจำกัดการใช้งานผ่าน local API ได้".into(),
            },
            target_agent_ids: vec![agent.agent_id.clone()],
            recommended_control_level: ControlLevel::Warn,
            feasibility: PolicyFeasibilityStatus::CanEnforceNow,
            confidence: 0.9,
            reason_codes: vec!["local_api_detected".into()],
            setup_required: vec![],
        });
    }

    if network_ready {
        suggestions.push(SuggestedPolicy {
            suggestion_id: format!("sugg-net-block-{}", agent.agent_id),
            policy_template_id: "block_unknown_network_destinations".into(),
            display_name: LocalizedText {
                en: "Block unknown network destinations".into(),
                th: "บล็อกการเข้าถึง Network ที่ไม่รู้จัก".into(),
            },
            description: LocalizedText {
                en: "System network control is ready on this device.".into(),
                th: "เครื่องนี้พร้อมใช้ system network control".into(),
            },
            target_agent_ids: vec![agent.agent_id.clone()],
            recommended_control_level: ControlLevel::Enforce,
            feasibility: PolicyFeasibilityStatus::CanEnforceNow,
            confidence: 0.95,
            reason_codes: vec!["network_control_ready".into()],
            setup_required: vec![],
        });
    } else {
        suggestions.push(SuggestedPolicy {
            suggestion_id: format!("sugg-net-obs-{}", agent.agent_id),
            policy_template_id: "observe_unknown_network_destinations".into(),
            display_name: LocalizedText {
                en: "Observe network destinations".into(),
                th: "ตรวจสอบการใช้งาน Network".into(),
            },
            description: LocalizedText {
                en: "System network control is not ready, but POLLEK can still observe known agent activity.".into(),
                th: "system network control ยังไม่พร้อม แต่ระบบยัง Observe activity ของ Agent ที่รู้จักได้".into(),
            },
            target_agent_ids: vec![agent.agent_id.clone()],
            recommended_control_level: ControlLevel::Observe,
            feasibility: PolicyFeasibilityStatus::CanObserveOnly,
            confidence: 0.8,
            reason_codes: vec!["network_control_not_ready".into()],
            setup_required: vec![],
        });
    }

    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    use dek_capability_registry::snapshot::*;
    use dek_domain_schema::capability_inventory::*;
    use dek_domain_schema::feasibility::InternalPep;

    fn make_agent(id: &str, has_mcp: bool, has_api: bool) -> AgentCapabilityInventory {
        AgentCapabilityInventory {
            schema_version: "1".into(),
            tenant_id: "t1".into(),
            device_id: "d1".into(),
            agent_id: id.into(),
            candidate_id: None,
            display_name: id.into(),
            agent_type: AgentKind::DesktopAgent,
            trust_level: "High".into(),
            confidence: 1.0,
            risk_score: 0,
            process: None,
            config_surfaces: vec![],
            mcp_surfaces: if has_mcp {
                vec![McpSurface {
                    server_name: "test".into(),
                    client_hint: "test".into(),
                    transport: McpTransportKind::Stdio,
                    command_template: None,
                    endpoint_domain: None,
                    has_auth_header: false,
                    env_key_names: vec![],
                    tools_known: vec![],
                    resources_known: vec![],
                }]
            } else {
                vec![]
            },
            model_endpoints: if has_api {
                vec![ModelEndpointSurface {
                    endpoint_url: "http://localhost:11434".into(),
                    protocol: "http".into(),
                    models_known: vec![],
                }]
            } else {
                vec![]
            },
            browser_surfaces: vec![],
            file_surfaces: vec![],
            network_surfaces: vec![],
            supported_pep_bindings: vec![],
            supported_pdp_routes: vec![],
            telemetry_capabilities: TelemetryCapabilities {
                emits_tool_logs: false,
                emits_resource_logs: false,
                emits_decision_logs: false,
                emits_network_logs: false,
                format: "json".into(),
            },
            last_scan_id: "".into(),
            last_seen_at: "".into(),
        }
    }

    #[test]
    fn test_suggestions_mcp_and_network_ready() {
        let agent = make_agent("test1", true, false);
        let caps = vec![ControlMethodCapability {
            method: ControlMethod::SystemNetworkControl,
            internal_pep: InternalPep::WindowsWfp,
            status: CapabilityStatus::Ready,
            can_observe: true,
            can_enforce: true,
            requires_admin: true,
            requires_user_approval: false,
            confidence: 1.0,
            evidence: vec![],
            user_message: LocalizedText {
                en: "".into(),
                th: "".into(),
            },
            next_action: None,
        }];

        let suggs = suggest_for_agent(&agent, &caps);
        assert_eq!(suggs.len(), 3); // 2 for MCP, 1 for network
        let template_ids: Vec<_> = suggs
            .iter()
            .map(|s| s.policy_template_id.as_str())
            .collect();
        assert!(template_ids.contains(&"approve_risky_tool_calls"));
        assert!(template_ids.contains(&"redact_sensitive_parameters"));
        assert!(template_ids.contains(&"block_unknown_network_destinations"));
    }
}
