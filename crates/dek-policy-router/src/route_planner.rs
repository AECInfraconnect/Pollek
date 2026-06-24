// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_domain_schema::capabilities::{
    CapabilityStatus, DeviceCapabilityReport, PdpCapabilityStatus, PepCapabilityStatus,
};
use dek_domain_schema::control_level::ControlLevel;
use dek_domain_schema::deployment_session::{
    DeploymentSession, EnforcementLayer, FallbackPlan, LocalizedText, ObservabilityPath, PdpEngine,
    PdpRouteMode, PdpSelection, PepSelection, RoutingPlan, UserAction, UserMessage,
};

#[derive(Debug)]
pub enum RouteError {
    NoValidPdp,
    NoValidPep,
}

impl std::fmt::Display for RouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RouteError::NoValidPdp => write!(f, "No valid PDP engine found"),
            RouteError::NoValidPep => write!(f, "No valid PEP layer found"),
        }
    }
}

impl std::error::Error for RouteError {}

pub struct RoutePlanner;

impl RoutePlanner {
    pub fn plan_route(
        session: &DeploymentSession,
        device_caps: &DeviceCapabilityReport,
    ) -> Result<RoutingPlan, RouteError> {
        // 1. Pick best PDP
        let pdp = Self::select_pdp(session, &device_caps.pdps)?;

        // 2. Pick best PEP
        let pep = Self::select_pep(session, &device_caps.peps)?;

        // 3. Fallback logic
        let fallback = Self::calculate_fallback(&pdp, &pep);

        // 4. effective control level
        let mut effective_control_level = session.requested_control_level;
        if pep.status != CapabilityStatus::Ready || pdp.status != CapabilityStatus::Ready {
            effective_control_level = ControlLevel::Observe;
        }

        // 5. observability path
        let observability_path = if effective_control_level == ControlLevel::Observe {
            ObservabilityPath::SecureSpoolAndOtel
        } else {
            ObservabilityPath::LocalOnly
        };

        // Gather user messages
        let mut user_messages = vec![];
        let mut required_actions = vec![];

        if pep.status != CapabilityStatus::Ready {
            user_messages.push(UserMessage {
                message_id: "pep_degraded".into(),
                text: pep.reason.clone(),
                severity: dek_domain_schema::deployment_session::EventStatus::Warning,
            });
            if let Some(pep_cap) = device_caps.peps.iter().find(|p| p.layer == pep.layer) {
                if let Some(action) = &pep_cap.next_action {
                    required_actions.push(UserAction {
                        action_id: format!("action_{:?}", action),
                        kind: action.clone(),
                        label: LocalizedText {
                            en: format!("Resolve {:?}", action),
                            th: format!("แก้ไข {:?}", action),
                        },
                        help: pep.reason.clone(),
                        safe_to_retry: true,
                    });
                }
            }
        }

        Ok(RoutingPlan {
            deployment_id: session.deployment_id.clone(),
            agent_id: match &session.target_scope {
                dek_domain_schema::deployment_session::DeploymentScope::Agent { agent_id } => {
                    agent_id.clone()
                }
                _ => "unknown".into(), // Simplified
            },
            selected_pep: pep,
            selected_pdp: pdp,
            effective_control_level,
            observability_path,
            fallback,
            user_messages,
            required_actions,
        })
    }

    fn select_pdp(
        _session: &DeploymentSession,
        pdps: &[PdpCapabilityStatus],
    ) -> Result<PdpSelection, RouteError> {
        // Prefer Cedar Native, then Wasm, then Cloud
        let engines = [
            PdpEngine::Cedar,
            PdpEngine::OpaWasm,
            PdpEngine::OpenFga,
            PdpEngine::Cloud,
        ];

        for engine in engines {
            if let Some(pdp) = pdps.iter().find(|p| p.engine == engine) {
                return Ok(PdpSelection {
                    pdp_id: format!("{:?}", engine).to_lowercase(),
                    engine: pdp.engine.clone(),
                    mode: PdpRouteMode::LocalOnly,
                    status: pdp.status.clone(),
                    reason_code: pdp.reason_code.clone(),
                    reason: pdp.user_message.clone(),
                });
            }
        }
        Err(RouteError::NoValidPdp)
    }

    fn select_pep(
        _session: &DeploymentSession,
        peps: &[PepCapabilityStatus],
    ) -> Result<PepSelection, RouteError> {
        // Priority: eBPF > Mac NE > Windows WFP > MCP Proxy > Stdio
        let layers = [
            EnforcementLayer::EbpfNetwork,
            EnforcementLayer::MacosNetworkExtension,
            EnforcementLayer::WindowsWfp,
            EnforcementLayer::McpProxy,
            EnforcementLayer::McpStdioWrapper,
        ];

        for layer in layers {
            if let Some(pep) = peps.iter().find(|p| p.layer == layer) {
                return Ok(PepSelection {
                    pep_id: format!("{:?}", layer).to_lowercase(),
                    layer: pep.layer.clone(),
                    status: pep.status.clone(),
                    reason_code: pep.reason_code.clone(),
                    reason: pep.user_message.clone(),
                });
            }
        }
        Err(RouteError::NoValidPep)
    }

    fn calculate_fallback(_pdp: &PdpSelection, _pep: &PepSelection) -> FallbackPlan {
        FallbackPlan {
            enabled: true,
            fallback_pep: Some(EnforcementLayer::ObserveOnly),
            fallback_pdp: Some(PdpEngine::Cloud),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use dek_domain_schema::capabilities::{
        CapabilityStatus, DeviceCapabilityReport, OsProfile, PdpCapabilityStatus,
        PepCapabilityStatus,
    };
    use dek_domain_schema::control_level::ControlLevel;
    use dek_domain_schema::deployment_session::{
        DeploymentScope, DeploymentSession, DeploymentSessionStatus, EnforcementLayer,
        LocalizedText, PdpEngine,
    };

    fn make_session(level: ControlLevel) -> DeploymentSession {
        DeploymentSession {
            deployment_id: "dep1".into(),
            policy_id: "pol1".into(),
            policy_version: "v1".into(),
            requested_control_level: level,
            target_scope: DeploymentScope::Agent {
                agent_id: "ag1".into(),
            },
            status: DeploymentSessionStatus::Planning,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: "test".into(),
        }
    }

    fn make_text() -> LocalizedText {
        LocalizedText {
            en: "test".into(),
            th: "ทดสอบ".into(),
        }
    }

    #[test]
    fn test_permutation_1_linux_ebpf_cedar_enforce() -> Result<(), RouteError> {
        let session = make_session(ControlLevel::Enforce);
        let caps = DeviceCapabilityReport {
            device_id: "d1".into(),
            os: OsProfile {
                r#type: "linux".into(),
                version: "1".into(),
                arch: "x86".into(),
            },
            peps: vec![PepCapabilityStatus {
                layer: EnforcementLayer::EbpfNetwork,
                status: CapabilityStatus::Ready,
                confidence: 1.0,
                detected_version: None,
                reason_code: "ok".into(),
                user_message: make_text(),
                next_action: None,
            }],
            pdps: vec![PdpCapabilityStatus {
                engine: PdpEngine::Cedar,
                status: CapabilityStatus::Ready,
                reason_code: "ok".into(),
                user_message: make_text(),
            }],
            scanned_at: Utc::now(),
        };

        let plan = RoutePlanner::plan_route(&session, &caps)?;
        assert_eq!(plan.selected_pep.layer, EnforcementLayer::EbpfNetwork);
        assert_eq!(plan.selected_pdp.engine, PdpEngine::Cedar);
        assert_eq!(plan.effective_control_level, ControlLevel::Enforce);
        Ok(())
    }

    #[test]
    fn test_permutation_2_windows_wfp_observe() -> Result<(), RouteError> {
        let session = make_session(ControlLevel::Enforce);
        let caps = DeviceCapabilityReport {
            device_id: "d2".into(),
            os: OsProfile {
                r#type: "windows".into(),
                version: "10".into(),
                arch: "x64".into(),
            },
            peps: vec![PepCapabilityStatus {
                layer: EnforcementLayer::WindowsWfp,
                status: CapabilityStatus::ReadyRequiresApproval,
                confidence: 0.8,
                detected_version: None,
                reason_code: "needs_approval".into(),
                user_message: make_text(),
                next_action: None,
            }],
            pdps: vec![PdpCapabilityStatus {
                engine: PdpEngine::Cedar,
                status: CapabilityStatus::Ready,
                reason_code: "ok".into(),
                user_message: make_text(),
            }],
            scanned_at: Utc::now(),
        };

        let plan = RoutePlanner::plan_route(&session, &caps)?;
        assert_eq!(plan.selected_pep.layer, EnforcementLayer::WindowsWfp);
        assert_eq!(plan.effective_control_level, ControlLevel::Observe);
        Ok(())
    }

    #[test]
    fn test_permutation_3_macos_nefilter_missing_driver() -> Result<(), RouteError> {
        let session = make_session(ControlLevel::Enforce);
        let caps = DeviceCapabilityReport {
            device_id: "d3".into(),
            os: OsProfile {
                r#type: "macos".into(),
                version: "14".into(),
                arch: "arm64".into(),
            },
            peps: vec![PepCapabilityStatus {
                layer: EnforcementLayer::MacosNetworkExtension,
                status: CapabilityStatus::MissingDriver,
                confidence: 1.0,
                detected_version: None,
                reason_code: "no_driver".into(),
                user_message: make_text(),
                next_action: None,
            }],
            pdps: vec![PdpCapabilityStatus {
                engine: PdpEngine::Cedar,
                status: CapabilityStatus::Ready,
                reason_code: "ok".into(),
                user_message: make_text(),
            }],
            scanned_at: Utc::now(),
        };

        let plan = RoutePlanner::plan_route(&session, &caps)?;
        assert_eq!(
            plan.selected_pep.layer,
            EnforcementLayer::MacosNetworkExtension
        );
        assert_eq!(plan.effective_control_level, ControlLevel::Observe);
        Ok(())
    }

    #[test]
    fn test_permutation_4_cloud_fallback_pdp() -> Result<(), RouteError> {
        let session = make_session(ControlLevel::Enforce);
        let caps = DeviceCapabilityReport {
            device_id: "d4".into(),
            os: OsProfile {
                r#type: "linux".into(),
                version: "1".into(),
                arch: "x86".into(),
            },
            peps: vec![PepCapabilityStatus {
                layer: EnforcementLayer::EbpfNetwork,
                status: CapabilityStatus::Ready,
                confidence: 1.0,
                detected_version: None,
                reason_code: "ok".into(),
                user_message: make_text(),
                next_action: None,
            }],
            pdps: vec![PdpCapabilityStatus {
                engine: PdpEngine::Cloud,
                status: CapabilityStatus::Ready,
                reason_code: "ok".into(),
                user_message: make_text(),
            }],
            scanned_at: Utc::now(),
        };

        let plan = RoutePlanner::plan_route(&session, &caps)?;
        assert_eq!(plan.selected_pep.layer, EnforcementLayer::EbpfNetwork);
        assert_eq!(plan.selected_pdp.engine, PdpEngine::Cloud);
        assert_eq!(plan.effective_control_level, ControlLevel::Enforce);
        Ok(())
    }

    #[test]
    fn test_permutation_5_mcp_proxy() -> Result<(), RouteError> {
        let session = make_session(ControlLevel::Enforce);
        let caps = DeviceCapabilityReport {
            device_id: "d5".into(),
            os: OsProfile {
                r#type: "windows".into(),
                version: "10".into(),
                arch: "x64".into(),
            },
            peps: vec![PepCapabilityStatus {
                layer: EnforcementLayer::McpProxy,
                status: CapabilityStatus::Ready,
                confidence: 1.0,
                detected_version: None,
                reason_code: "ok".into(),
                user_message: make_text(),
                next_action: None,
            }],
            pdps: vec![PdpCapabilityStatus {
                engine: PdpEngine::Cedar,
                status: CapabilityStatus::Ready,
                reason_code: "ok".into(),
                user_message: make_text(),
            }],
            scanned_at: Utc::now(),
        };

        let plan = RoutePlanner::plan_route(&session, &caps)?;
        assert_eq!(plan.selected_pep.layer, EnforcementLayer::McpProxy);
        assert_eq!(plan.selected_pdp.engine, PdpEngine::Cedar);
        assert_eq!(plan.effective_control_level, ControlLevel::Enforce);
        Ok(())
    }

    #[test]
    fn test_permutation_6_no_valid_pep() {
        let session = make_session(ControlLevel::Observe);
        let caps = DeviceCapabilityReport {
            device_id: "d6".into(),
            os: OsProfile {
                r#type: "linux".into(),
                version: "1".into(),
                arch: "x86".into(),
            },
            peps: vec![],
            pdps: vec![PdpCapabilityStatus {
                engine: PdpEngine::Cedar,
                status: CapabilityStatus::Ready,
                reason_code: "ok".into(),
                user_message: make_text(),
            }],
            scanned_at: Utc::now(),
        };

        let res = RoutePlanner::plan_route(&session, &caps);
        assert!(matches!(res, Err(RouteError::NoValidPep)));
    }
}
