// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_capability_registry::LocalCapabilitySnapshot;
use dek_domain_schema::{
    capability_inventory::AgentCapabilityInventory,
    control_level::ControlLevel,
    deployment_session::LocalizedText,
    feasibility::{
        ControlMethod, ControlMethodPlan, Enforceability, InternalPdp, InternalPep,
        PolicyFeasibilityRequest, PolicyFeasibilityResult, PolicyFeasibilityStatus, PolicyIntent,
        ProductMode,
    },
    policy_target::PolicyTarget,
};

pub struct ControlLevelNegotiation {
    pub requested: ControlLevel,
    pub effective: ControlLevel,
    pub downgraded: bool,
    pub reason: LocalizedText,
    pub requires_user_confirmation: bool,
}

pub fn negotiate_control_level(
    requested: ControlLevel,
    enforceability: &Enforceability,
) -> ControlLevelNegotiation {
    let effective = match requested {
        ControlLevel::StrictDeny if enforceability.can_strict_deny => ControlLevel::StrictDeny,
        ControlLevel::StrictDeny if enforceability.can_enforce => ControlLevel::Enforce,
        ControlLevel::Enforce if enforceability.can_enforce => ControlLevel::Enforce,
        ControlLevel::Enforce if enforceability.can_require_approval => ControlLevel::Approval,
        ControlLevel::Approval if enforceability.can_require_approval => ControlLevel::Approval,
        ControlLevel::Warn if enforceability.can_warn => ControlLevel::Warn,
        _ if enforceability.can_observe => ControlLevel::Observe,
        _ => ControlLevel::Observe,
    };

    let downgraded = effective != requested;

    ControlLevelNegotiation {
        requested,
        effective,
        downgraded,
        reason: if downgraded {
            LocalizedText {
                en: "This device cannot fully enforce the requested level yet. POLLEK will use the strongest available safe mode.".into(),
                th: "เครื่องนี้ยัง enforce ตามระดับที่ขอได้ไม่เต็มที่ ระบบจะใช้ระดับที่ปลอดภัยและทำได้จริงที่สุด".into(),
            }
        } else {
            LocalizedText {
                en: "The requested control level is supported on this device.".into(),
                th: "เครื่องนี้รองรับ control level ที่เลือก".into(),
            }
        },
        requires_user_confirmation: downgraded,
    }
}

pub fn score_plan(req: &PolicyFeasibilityRequest, plan: &ControlMethodPlan) -> i32 {
    let mut score = 0;

    if plan.enforceability.can_enforce {
        score += 100;
    }
    if plan.enforceability.can_require_approval {
        score += 70;
    }
    if plan.enforceability.can_warn {
        score += 50;
    }
    if plan.enforceability.can_observe {
        score += 20;
    }

    if matches!(req.mode, ProductMode::DesktopSimple) {
        score += match plan.method {
            ControlMethod::AgentToolControl => 40,
            ControlMethod::AgentConfigWrapper => 30,
            ControlMethod::LocalApiControl => 25,
            ControlMethod::BrowserActivityMonitor => 15,
            ControlMethod::NetworkControl => -10,
            ControlMethod::ProcessObservation => 10,
            ControlMethod::ObserveOnly => 0,
        };
    }

    if matches!(req.mode, ProductMode::EnterpriseServer)
        && matches!(plan.method, ControlMethod::NetworkControl)
    {
        score += 40;
    }

    score
}

pub fn resolve_agent<'a>(
    target: &PolicyTarget,
    snapshot: &'a LocalCapabilitySnapshot,
) -> Option<&'a AgentCapabilityInventory> {
    snapshot
        .agents
        .iter()
        .find(|a| a.agent_id == target.target_id)
}

pub fn candidate_methods_for_intent(
    _intent: &PolicyIntent,
    _agent: Option<&AgentCapabilityInventory>,
    snapshot: &LocalCapabilitySnapshot,
) -> Vec<(ControlMethodPlan, PolicyFeasibilityStatus)> {
    let mut plans = Vec::new();

    for m in &snapshot.methods {
        use dek_capability_registry::snapshot::{capability_to_user_status, CapabilityStatus};
        if m.status == CapabilityStatus::UnsupportedOnThisOs
            || m.status == CapabilityStatus::UnsupportedForThisAgent
            || m.status == CapabilityStatus::Unknown
        {
            continue;
        }

        let status = capability_to_user_status(m);

        plans.push((
            ControlMethodPlan {
                method: m.method.clone(),
                internal_pep: m.internal_pep.clone(),
                internal_pdp: InternalPdp::Cedar,
                enforceability: Enforceability {
                    can_observe: m.can_observe,
                    can_warn: m.can_enforce,
                    can_require_approval: m.requires_user_approval || m.can_enforce,
                    can_enforce: m.can_enforce,
                    can_strict_deny: m.can_enforce,
                },
                reason_code: "mapped_from_capability".to_string(),
                explanation: m.user_message.clone(),
                diagnostics: vec![],
            },
            status,
        ));
    }

    if plans.is_empty() {
        plans.push((
            ControlMethodPlan {
                method: ControlMethod::ObserveOnly,
                internal_pep: InternalPep::None,
                internal_pdp: InternalPdp::Cloud,
                enforceability: Enforceability {
                    can_observe: true,
                    can_warn: false,
                    can_require_approval: false,
                    can_enforce: false,
                    can_strict_deny: false,
                },
                reason_code: "fallback_observe".to_string(),
                explanation: LocalizedText {
                    en: "No active control method available. Reverting to observe only.".into(),
                    th: "ไม่มีวิธีควบคุมที่ใช้งานได้ เปลี่ยนเป็นแค่สังเกตการณ์".into(),
                },
                diagnostics: vec![],
            },
            PolicyFeasibilityStatus::CanObserveOnly,
        ));
    }
    plans
}

pub fn select_best_control_method(
    req: &PolicyFeasibilityRequest,
    candidates: Vec<(ControlMethodPlan, PolicyFeasibilityStatus)>,
) -> (ControlMethodPlan, PolicyFeasibilityStatus) {
    let mut sorted = candidates;
    sorted.sort_by_key(|(plan, _)| score_plan(req, plan));
    sorted.pop().unwrap_or_else(|| {
        let observe_plan = ControlMethodPlan {
            method: ControlMethod::ObserveOnly,
            internal_pep: InternalPep::None,
            internal_pdp: InternalPdp::Cloud,
            enforceability: Enforceability {
                can_observe: true,
                can_warn: false,
                can_require_approval: false,
                can_enforce: false,
                can_strict_deny: false,
            },
            reason_code: "no_methods_found".to_string(),
            explanation: LocalizedText {
                en: "No suitable control methods found.".into(),
                th: "ไม่พบวิธีควบคุมที่เหมาะสม".into(),
            },
            diagnostics: vec![],
        };
        (observe_plan, PolicyFeasibilityStatus::Unknown)
    })
}

pub fn build_feasibility_result(
    req: &PolicyFeasibilityRequest,
    target: PolicyTarget,
    candidate: (ControlMethodPlan, PolicyFeasibilityStatus),
) -> PolicyFeasibilityResult {
    let (plan, status) = candidate;
    let negotiation = negotiate_control_level(req.requested_control_level, &plan.enforceability);

    PolicyFeasibilityResult {
        target,
        policy_intent: req.policy_intent.clone(),
        requested_control_level: req.requested_control_level,
        effective_control_level: negotiation.effective,
        status,
        user_summary: negotiation.reason,
        user_detail: plan.explanation.clone(),
        required_actions: vec![],
        technical_plan: Some(plan),
        confidence: 0.9,
    }
}

pub fn evaluate_policy_feasibility(
    req: PolicyFeasibilityRequest,
    snapshot: &LocalCapabilitySnapshot,
) -> Vec<PolicyFeasibilityResult> {
    req.targets
        .iter()
        .map(|target| {
            let agent = resolve_agent(target, snapshot);
            let candidates = candidate_methods_for_intent(&req.policy_intent, agent, snapshot);
            let best = select_best_control_method(&req, candidates);
            build_feasibility_result(&req, target.clone(), best)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use dek_capability_registry::snapshot::CapabilityStatus;
    use dek_domain_schema::policy_target::{Evaluators, MatchCriteria};

    fn create_mock_target(id: &str) -> PolicyTarget {
        PolicyTarget {
            schema_version: "1".into(),
            target_id: id.to_string(),
            tenant_id: "t1".into(),
            r#match: MatchCriteria {
                principal: None,
                agent: None,
                resource: None,
                action: None,
                network: None,
            },
            evaluators: Evaluators {
                required: vec![],
                conditional: None,
            },
            obligations: vec![],
        }
    }

    fn create_mock_snapshot() -> LocalCapabilitySnapshot {
        LocalCapabilitySnapshot {
            snapshot_id: "snap1".into(),
            device_id: "d1".into(),
            os: dek_capability_registry::OsInfo {
                r#type: "windows".into(),
                version: "11".into(),
                arch: "x86_64".into(),
            },
            agents: vec![],
            methods: vec![
                dek_capability_registry::snapshot::ControlMethodCapability {
                    method: ControlMethod::AgentToolControl,
                    internal_pep: InternalPep::McpStdioWrapper,
                    status: CapabilityStatus::ReadyAfterApproval,
                    can_observe: true,
                    can_enforce: true,
                    requires_admin: false,
                    requires_user_approval: true,
                    confidence: 1.0,
                    evidence: vec![],
                    user_message: LocalizedText {
                        en: "".into(),
                        th: "".into(),
                    },
                    next_action: None,
                },
                dek_capability_registry::snapshot::ControlMethodCapability {
                    method: ControlMethod::LocalApiControl,
                    internal_pep: InternalPep::HttpProxy,
                    status: CapabilityStatus::Ready,
                    can_observe: true,
                    can_enforce: true,
                    requires_admin: false,
                    requires_user_approval: false,
                    confidence: 1.0,
                    evidence: vec![],
                    user_message: LocalizedText {
                        en: "".into(),
                        th: "".into(),
                    },
                    next_action: None,
                },
            ],
            generated_at: Utc::now(),
        }
    }

    #[test]
    fn test_mcp_stdio_approval_required() {
        let mut snapshot = create_mock_snapshot();
        snapshot
            .methods
            .retain(|m| m.internal_pep == InternalPep::McpStdioWrapper);

        let req = PolicyFeasibilityRequest {
            policy_id: None,
            policy_intent: PolicyIntent::ApproveRiskyToolCalls,
            requested_control_level: ControlLevel::Enforce,
            targets: vec![create_mock_target("claude_desktop")],
            mode: ProductMode::DesktopSimple,
        };

        let res = evaluate_policy_feasibility(req, &snapshot);
        assert_eq!(res.len(), 1);
        assert_eq!(
            res[0].status,
            PolicyFeasibilityStatus::CanEnforceAfterApproval
        );
    }

    #[test]
    fn test_mcp_http_enforce_now() {
        let mut snapshot = create_mock_snapshot();
        snapshot
            .methods
            .retain(|m| m.internal_pep == InternalPep::HttpProxy);

        let req = PolicyFeasibilityRequest {
            policy_id: None,
            policy_intent: PolicyIntent::BlockSpecificTools,
            requested_control_level: ControlLevel::Enforce,
            targets: vec![create_mock_target("cursor")],
            mode: ProductMode::DesktopSimple,
        };

        let res = evaluate_policy_feasibility(req, &snapshot);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].status, PolicyFeasibilityStatus::CanEnforceNow);
    }

    #[test]
    fn test_no_os_network_control_fallback() {
        let mut snapshot = create_mock_snapshot();
        snapshot.methods.clear();

        let req = PolicyFeasibilityRequest {
            policy_id: None,
            policy_intent: PolicyIntent::BlockUnknownNetworkDestinations,
            requested_control_level: ControlLevel::Enforce,
            targets: vec![create_mock_target("local_ollama")],
            mode: ProductMode::DesktopSimple,
        };

        let res = evaluate_policy_feasibility(req, &snapshot);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].status, PolicyFeasibilityStatus::CanObserveOnly);
        assert_eq!(res[0].effective_control_level, ControlLevel::Observe);
    }

    #[test]
    fn test_downgrade_confirmation() {
        let mut snapshot = create_mock_snapshot();
        snapshot.methods.clear();
        snapshot
            .methods
            .push(dek_capability_registry::snapshot::ControlMethodCapability {
                method: ControlMethod::ObserveOnly,
                internal_pep: InternalPep::None,
                status: CapabilityStatus::Ready,
                can_observe: true,
                can_enforce: false,
                requires_admin: false,
                requires_user_approval: false,
                confidence: 1.0,
                evidence: vec![],
                user_message: LocalizedText {
                    en: "".into(),
                    th: "".into(),
                },
                next_action: None,
            });

        let req = PolicyFeasibilityRequest {
            policy_id: None,
            policy_intent: PolicyIntent::ApproveRiskyToolCalls,
            requested_control_level: ControlLevel::Enforce,
            targets: vec![create_mock_target("claude_desktop")],
            mode: ProductMode::DesktopSimple,
        };

        let res = evaluate_policy_feasibility(req, &snapshot);
        assert_eq!(res[0].effective_control_level, ControlLevel::Observe);
        assert!(res[0].user_summary.en.contains("cannot fully enforce"));
    }
}
