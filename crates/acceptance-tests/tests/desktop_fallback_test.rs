// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_capability_registry::{
    snapshot::CapabilityStatus, ControlMethodCapability, LocalCapabilitySnapshot, OsInfo,
};
use dek_deployment_planner::evaluate_policy_feasibility;
use dek_domain_schema::deployment_session::LocalizedText;
use dek_domain_schema::{
    control_level::ControlLevel,
    feasibility::{
        ControlMethod, InternalPep, PolicyFeasibilityRequest, PolicyIntent, ProductMode,
    },
    policy_target::PolicyTarget,
};

#[test]
fn test_claude_desktop_mcp_stdio_fallback() {
    let snapshot = LocalCapabilitySnapshot {
        snapshot_id: "snap_1".into(),
        device_id: "dev_1".into(),
        os: OsInfo {
            r#type: "windows".into(),
            version: "11".into(),
            arch: "x86_64".into(),
        },
        agents: vec![],
        methods: vec![ControlMethodCapability {
            method: ControlMethod::AgentConfigWrapper,
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
        }],
        generated_at: chrono::Utc::now(),
    };

    let req = PolicyFeasibilityRequest {
        policy_id: None,
        policy_intent: PolicyIntent::ApproveRiskyToolCalls,
        requested_control_level: ControlLevel::Enforce,
        targets: vec![PolicyTarget {
            schema_version: "1.0".into(),
            target_id: "claude_desktop".into(),
            tenant_id: "local".into(),
            r#match: dek_domain_schema::policy_target::MatchCriteria {
                principal: None,
                agent: Some(dek_domain_schema::policy_target::AgentMatch {
                    agent_types: Some(vec!["desktop".into()]),
                    risk_max: None,
                }),
                resource: None,
                action: None,
                network: None,
            },
            evaluators: dek_domain_schema::policy_target::Evaluators {
                required: vec![],
                conditional: None,
            },
            obligations: vec![],
        }],
        mode: ProductMode::DesktopSimple,
    };

    let results = evaluate_policy_feasibility(req, &snapshot);
    assert!(!results.is_empty());
    assert_eq!(
        results[0].status,
        dek_domain_schema::feasibility::PolicyFeasibilityStatus::CanEnforceAfterApproval
    );
}

fn create_mock_snapshot() -> LocalCapabilitySnapshot {
    LocalCapabilitySnapshot {
        snapshot_id: "snap_1".into(),
        device_id: "dev_1".into(),
        os: OsInfo {
            r#type: "windows".into(),
            version: "11".into(),
            arch: "x86_64".into(),
        },
        agents: vec![],
        methods: vec![],
        generated_at: chrono::Utc::now(),
    }
}

fn create_mock_target(target_id: &str) -> PolicyTarget {
    PolicyTarget {
        schema_version: "1.0".into(),
        target_id: target_id.into(),
        tenant_id: "local".into(),
        r#match: dek_domain_schema::policy_target::MatchCriteria {
            principal: None,
            agent: Some(dek_domain_schema::policy_target::AgentMatch {
                agent_types: Some(vec!["desktop".into()]),
                risk_max: None,
            }),
            resource: None,
            action: None,
            network: None,
        },
        evaluators: dek_domain_schema::policy_target::Evaluators {
            required: vec![],
            conditional: None,
        },
        obligations: vec![],
    }
}

#[test]
fn test_cursor_mcp_http_fallback() {
    let mut snapshot = create_mock_snapshot();
    snapshot.methods.push(ControlMethodCapability {
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
    });

    let req = PolicyFeasibilityRequest {
        policy_id: None,
        policy_intent: PolicyIntent::ApproveRiskyToolCalls,
        requested_control_level: ControlLevel::Enforce,
        targets: vec![create_mock_target("cursor")],
        mode: ProductMode::DesktopSimple,
    };
    let results = evaluate_policy_feasibility(req, &snapshot);
    assert_eq!(
        results[0].status,
        dek_domain_schema::feasibility::PolicyFeasibilityStatus::CanEnforceNow
    );
}

#[test]
fn test_local_ollama_http() {
    let snapshot = create_mock_snapshot();
    // No enforcing methods available
    let req = PolicyFeasibilityRequest {
        policy_id: None,
        policy_intent: PolicyIntent::LimitTokenOrCostUsage,
        requested_control_level: ControlLevel::Enforce,
        targets: vec![create_mock_target("local_ollama")],
        mode: ProductMode::DesktopSimple,
    };
    let results = evaluate_policy_feasibility(req, &snapshot);
    assert_eq!(
        results[0].status,
        dek_domain_schema::feasibility::PolicyFeasibilityStatus::CanObserveOnly
    );
}

#[test]
fn test_browser_ai_no_extension() {
    let mut snapshot = create_mock_snapshot();
    snapshot.methods.push(ControlMethodCapability {
        method: ControlMethod::BrowserActivityMonitor,
        internal_pep: InternalPep::None,
        status: CapabilityStatus::MissingComponent,
        can_observe: false,
        can_enforce: false,
        requires_admin: false,
        requires_user_approval: false,
        confidence: 0.0,
        evidence: vec![],
        user_message: LocalizedText {
            en: "".into(),
            th: "".into(),
        },
        next_action: None,
    });

    let req = PolicyFeasibilityRequest {
        policy_id: None,
        policy_intent: PolicyIntent::DetectPromptInjection,
        requested_control_level: ControlLevel::Enforce,
        targets: vec![create_mock_target("browser_ai")],
        mode: ProductMode::DesktopSimple,
    };
    let results = evaluate_policy_feasibility(req, &snapshot);
    assert_eq!(
        results[0].status,
        dek_domain_schema::feasibility::PolicyFeasibilityStatus::NeedsSetup
    );
}

#[test]
fn test_windows_no_wfp() {
    let mut snapshot = create_mock_snapshot();
    snapshot.methods.push(ControlMethodCapability {
        method: ControlMethod::SystemNetworkControl,
        internal_pep: InternalPep::WindowsWfp,
        status: CapabilityStatus::MissingPermission, // Needs admin approval to install
        can_observe: false,
        can_enforce: false,
        requires_admin: true,
        requires_user_approval: true,
        confidence: 0.0,
        evidence: vec![],
        user_message: LocalizedText {
            en: "".into(),
            th: "".into(),
        },
        next_action: None,
    });

    let req = PolicyFeasibilityRequest {
        policy_id: None,
        policy_intent: PolicyIntent::BlockUnknownNetworkDestinations,
        requested_control_level: ControlLevel::Enforce,
        targets: vec![create_mock_target("windows_os")],
        mode: ProductMode::DesktopSimple,
    };
    let results = evaluate_policy_feasibility(req, &snapshot);
    assert_eq!(
        results[0].status,
        dek_domain_schema::feasibility::PolicyFeasibilityStatus::NeedsSetup
    );
}

#[test]
fn test_macos_network_extension_inactive() {
    let mut snapshot = create_mock_snapshot();
    snapshot.methods.push(ControlMethodCapability {
        method: ControlMethod::SystemNetworkControl,
        internal_pep: InternalPep::MacosNetworkExtension,
        status: CapabilityStatus::MissingPermission,
        can_observe: false,
        can_enforce: false,
        requires_admin: true,
        requires_user_approval: true,
        confidence: 0.0,
        evidence: vec![],
        user_message: LocalizedText {
            en: "".into(),
            th: "".into(),
        },
        next_action: None,
    });

    let req = PolicyFeasibilityRequest {
        policy_id: None,
        policy_intent: PolicyIntent::BlockUnknownNetworkDestinations,
        requested_control_level: ControlLevel::Enforce,
        targets: vec![create_mock_target("macos_network_extension")],
        mode: ProductMode::DesktopSimple,
    };
    let results = evaluate_policy_feasibility(req, &snapshot);
    assert_eq!(
        results[0].status,
        dek_domain_schema::feasibility::PolicyFeasibilityStatus::NeedsSetup
    );
}

#[test]
fn test_linux_ebpf_permission_denied() {
    let snapshot = create_mock_snapshot();
    let req = PolicyFeasibilityRequest {
        policy_id: None,
        policy_intent: PolicyIntent::BlockUnknownNetworkDestinations,
        requested_control_level: ControlLevel::Enforce,
        targets: vec![create_mock_target("linux_ebpf")],
        mode: ProductMode::DesktopSimple,
    };
    let results = evaluate_policy_feasibility(req, &snapshot);
    assert_eq!(
        results[0].status,
        dek_domain_schema::feasibility::PolicyFeasibilityStatus::CanObserveOnly
    );
    assert_eq!(results[0].effective_control_level, ControlLevel::Observe);
}
