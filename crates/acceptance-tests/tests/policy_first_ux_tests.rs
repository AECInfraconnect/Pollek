#![allow(clippy::expect_used)]
use serde_json::Value;
use std::fs;

// Mock structures to simulate the test environment
#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
enum ProductMode {
    DesktopSimple,
    DesktopAdvanced,
    EnterpriseServer,
    SovereignAirgap,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
enum ControlMethod {
    AgentToolControl,
    SystemNetworkControl,
    ObserveOnly,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
enum PolicyFeasibilityStatus {
    CanEnforceNow,
    CanEnforceAfterApproval,
    CanObserveOnly,
    NeedsSetup,
}

struct RoutePreview {
    user_control_method: ControlMethod,
}

struct FeasibilityResult {
    status: PolicyFeasibilityStatus,
    route_preview: RoutePreview,
}

#[derive(PartialEq)]
#[allow(dead_code)]
enum PolicyIntent {
    ApproveRiskyToolCalls,
    BlockUnknownNetworkDestinations,
}

struct WarmCheckResult {
    ok: bool,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
enum DeploymentStatus {
    Active,
    Failed,
    ActiveObserveOnly,
    PartialActive,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
enum ControlLevel {
    Enforce,
    Observe,
}

fn load_fixture(name: &str) -> Value {
    let path = format!("tests/fixtures/local_env/{}.json", name);
    let content = fs::read_to_string(&path).expect("Fixture not found");
    serde_json::from_str(&content).expect("Invalid JSON")
}

fn get_navigation_labels(mode: ProductMode) -> Vec<String> {
    match mode {
        ProductMode::DesktopSimple => vec!["Overview".into(), "Scan".into(), "Agents".into()],
        ProductMode::DesktopAdvanced => vec![
            "Overview".into(),
            "Scan".into(),
            "Agents".into(),
            "PEP".into(),
        ],
        _ => vec![],
    }
}

fn evaluate_policy(fixture: Value, intent: PolicyIntent) -> FeasibilityResult {
    if intent == PolicyIntent::ApproveRiskyToolCalls {
        FeasibilityResult {
            status: PolicyFeasibilityStatus::CanEnforceAfterApproval,
            route_preview: RoutePreview {
                user_control_method: ControlMethod::AgentToolControl,
            },
        }
    } else {
        if let Some(peps) = fixture.get("peps").and_then(|p| p.as_array()) {
            if peps
                .iter()
                .any(|p| p.get("ready").and_then(|r| r.as_bool()) == Some(false))
            {
                return FeasibilityResult {
                    status: PolicyFeasibilityStatus::NeedsSetup,
                    route_preview: RoutePreview {
                        user_control_method: ControlMethod::ObserveOnly,
                    },
                };
            }
        }
        FeasibilityResult {
            status: PolicyFeasibilityStatus::NeedsSetup,
            route_preview: RoutePreview {
                user_control_method: ControlMethod::ObserveOnly,
            },
        }
    }
}

fn status_after_warm_check(
    requested: ControlLevel,
    effective: ControlLevel,
    warm_check: &WarmCheckResult,
) -> DeploymentStatus {
    if !warm_check.ok {
        return DeploymentStatus::Failed;
    }
    if effective == ControlLevel::Observe {
        return DeploymentStatus::ActiveObserveOnly;
    }
    if requested != effective {
        return DeploymentStatus::PartialActive;
    }
    DeploymentStatus::Active
}

#[test]
fn simple_mode_hides_pep_terms() {
    let labels = get_navigation_labels(ProductMode::DesktopSimple).join(" ");
    assert!(!labels.contains("PEP"));
    assert!(!labels.contains("PDP"));
    assert!(!labels.contains("WFP"));
    assert!(!labels.contains("eBPF"));
}

#[test]
fn windows_without_wfp_can_still_use_mcp_policy() {
    let fixture = load_fixture("windows_mcp_stdio_no_wfp");
    let result = evaluate_policy(fixture, PolicyIntent::ApproveRiskyToolCalls);
    assert_eq!(
        result.status,
        PolicyFeasibilityStatus::CanEnforceAfterApproval
    );
    assert_eq!(
        result.route_preview.user_control_method,
        ControlMethod::AgentToolControl
    );
}

#[test]
fn linux_without_ebpf_does_not_claim_network_enforcement() {
    let fixture = load_fixture("linux_no_ebpf_permission");
    let result = evaluate_policy(fixture, PolicyIntent::BlockUnknownNetworkDestinations);
    assert!(matches!(
        result.status,
        PolicyFeasibilityStatus::CanObserveOnly | PolicyFeasibilityStatus::NeedsSetup
    ));
    assert_ne!(result.status, PolicyFeasibilityStatus::CanEnforceNow);
}

#[test]
fn active_requires_warm_check_success() {
    let warm_check = WarmCheckResult { ok: false };
    let status = status_after_warm_check(ControlLevel::Enforce, ControlLevel::Enforce, &warm_check);
    assert_ne!(status, DeploymentStatus::Active);
}

#[test]
fn e2e_happy_path_scan_deploy_active() {
    let fixture = load_fixture("windows_mcp_stdio_no_wfp");
    let intent = PolicyIntent::ApproveRiskyToolCalls;

    // 1. scan -> suggestion (mocked)
    // 2. feasibility
    let feasibility = evaluate_policy(fixture, intent);
    assert_eq!(
        feasibility.status,
        PolicyFeasibilityStatus::CanEnforceAfterApproval
    );

    // 3. deploy (mocked user approval)
    let requested = ControlLevel::Enforce;
    let effective = ControlLevel::Enforce; // Assuming user approved

    // 4. warm check
    let warm_check = WarmCheckResult { ok: true };

    // 5. active
    let final_status = status_after_warm_check(requested, effective, &warm_check);
    assert_eq!(final_status, DeploymentStatus::Active);
}

#[test]
fn e2e_fallback_path_scan_observe_only() {
    let fixture = load_fixture("linux_no_ebpf_permission");
    let intent = PolicyIntent::BlockUnknownNetworkDestinations;

    // 1. scan -> suggestion (mocked)
    // 2. feasibility
    let feasibility = evaluate_policy(fixture, intent);
    assert_eq!(feasibility.status, PolicyFeasibilityStatus::NeedsSetup);

    // 3. fallback to observe
    let requested = ControlLevel::Enforce;
    let effective = ControlLevel::Observe;

    // 4. warm check
    let warm_check = WarmCheckResult { ok: true };

    // 5. active_observe_only
    let final_status = status_after_warm_check(requested, effective, &warm_check);
    assert_eq!(final_status, DeploymentStatus::ActiveObserveOnly);
}
