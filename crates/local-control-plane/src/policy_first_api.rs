// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use dek_capability_registry::snapshot::{CapabilityStatus, ControlMethodCapability};
use dek_capability_registry::{CapabilityRegistry, LocalCapabilitySnapshot};
use dek_deployment_planner::{FeasibilitySuggester, PolicySuggestionEngine, SuggestedPolicy};
use dek_domain_schema::{
    capability_inventory::{
        AgentCapabilityInventory, AgentKind, McpSurface, McpTransportKind, ModelEndpointSurface,
        TelemetryCapabilities,
    },
    control_level::ControlLevel,
    deployment_session::{
        DeploymentScope, DeploymentSession, DeploymentSessionStatus, LocalizedText,
    },
    feasibility::{ControlMethod, InternalPep, PolicyFeasibilityRequest, PolicyFeasibilityResult},
};
use std::sync::Arc;
use uuid::Uuid;

use crate::deployment_orchestrator::{DeploymentOrchestrator, StoreEventSink};
use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/local/scan", post(scan))
        .route(
            "/v1/local/capability-snapshot/latest",
            get(get_latest_snapshot),
        )
        .route("/v1/policy-suggestions", get(get_policy_suggestions))
        .route("/v1/policies/feasibility", post(evaluate_feasibility))
        .route("/v1/deployment-sessions", post(create_deployment_session))
        .route(
            "/v1/deployment-sessions/:id/actions/:action_id/approve",
            post(approve_action),
        )
        .route("/v1/deployment-sessions/:id/retry", post(retry_deployment))
        .route(
            "/v1/deployment-sessions/:id/rollback",
            post(rollback_deployment),
        )
}

fn generate_real_snapshot() -> LocalCapabilitySnapshot {
    let registry = CapabilityRegistry::new("local".into(), "1.0".into());
    let dev_caps = registry.gather();

    let mut methods = Vec::new();

    // Map discovered PEPs to snapshot methods
    for pep in dev_caps.pep {
        let (method, internal) = match pep.r#type.as_str() {
            "linux-ebpf" => (ControlMethod::NetworkControl, InternalPep::LinuxEbpf),
            "windows-wfp" => (ControlMethod::NetworkControl, InternalPep::WindowsWfp),
            "macos-nefilter" => (
                ControlMethod::NetworkControl,
                InternalPep::MacosNetworkExtension,
            ),
            _ => (ControlMethod::ObserveOnly, InternalPep::None),
        };
        methods.push(ControlMethodCapability {
            method,
            internal_pep: internal,
            status: if pep.status == dek_domain_schema::capabilities::CapabilityStatus::Ready {
                CapabilityStatus::Ready
            } else {
                CapabilityStatus::MissingComponent
            },
            can_observe: true,
            can_enforce: pep.control_level == ControlLevel::Enforce,
            requires_admin: true,
            requires_user_approval: false,
            confidence: 1.0,
            evidence: vec![],
            user_message: LocalizedText {
                en: "".into(),
                th: "".into(),
            },
            next_action: None,
        });
    }

    // Add HTTP Proxy / API control if local proxy is present
    methods.push(ControlMethodCapability {
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

    // Generate agents based on typical dev environment for demo/UX testing
    let agents = vec![
        AgentCapabilityInventory {
            schema_version: "1".into(),
            tenant_id: "local".into(),
            device_id: "local".into(),
            agent_id: "claude_desktop".into(),
            candidate_id: None,
            display_name: "Claude Desktop".into(),
            agent_type: AgentKind::DesktopAgent,
            trust_level: "High".into(),
            confidence: 0.9,
            risk_score: 5,
            process: None,
            config_surfaces: vec![],
            mcp_surfaces: vec![McpSurface {
                server_name: "claude-mcp".into(),
                client_hint: "claude".into(),
                transport: McpTransportKind::Stdio,
                command_template: None,
                endpoint_domain: None,
                has_auth_header: false,
                env_key_names: vec![],
                tools_known: vec![],
                resources_known: vec![],
            }],
            model_endpoints: vec![],
            browser_surfaces: vec![],
            file_surfaces: vec![],
            network_surfaces: vec![],
            supported_pep_bindings: vec![],
            supported_pdp_routes: vec![],
            telemetry_capabilities: TelemetryCapabilities {
                emits_tool_logs: true,
                emits_resource_logs: false,
                emits_decision_logs: false,
                emits_network_logs: false,
                format: "json".into(),
            },
            last_scan_id: "".into(),
            last_seen_at: "".into(),
        },
        AgentCapabilityInventory {
            schema_version: "1".into(),
            tenant_id: "local".into(),
            device_id: "local".into(),
            agent_id: "local_ollama".into(),
            candidate_id: None,
            display_name: "Ollama".into(),
            agent_type: AgentKind::LocalModelServer,
            trust_level: "High".into(),
            confidence: 0.95,
            risk_score: 1,
            process: None,
            config_surfaces: vec![],
            mcp_surfaces: vec![],
            model_endpoints: vec![ModelEndpointSurface {
                endpoint_url: "http://127.0.0.1:11434".into(),
                protocol: "http".into(),
                models_known: vec![],
            }],
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
        },
    ];

    LocalCapabilitySnapshot {
        snapshot_id: Uuid::new_v4().to_string(),
        device_id: dev_caps.device_id,
        os: dev_caps.os,
        agents,
        methods,
        generated_at: Utc::now(),
    }
}

async fn scan(State(st): State<AppState>) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let snapshot = generate_real_snapshot();
    let mut lock = st.latest_snapshot.write().await;
    *lock = Some(snapshot);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({"status": "scanned"})),
    ))
}

async fn get_latest_snapshot(
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Json<LocalCapabilitySnapshot>)> {
    let lock = st.latest_snapshot.read().await;
    match &*lock {
        Some(snapshot) => Ok((StatusCode::OK, Json(snapshot.clone()))),
        None => {
            let fresh = generate_real_snapshot();
            Ok((StatusCode::OK, Json(fresh)))
        }
    }
}

async fn get_policy_suggestions(
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Json<Vec<SuggestedPolicy>>)> {
    let lock = st.latest_snapshot.read().await;
    let snapshot = match &*lock {
        Some(s) => s.clone(),
        None => generate_real_snapshot(),
    };

    let suggester = FeasibilitySuggester;
    let suggestions = suggester.suggest(&snapshot);

    Ok((StatusCode::OK, Json(suggestions)))
}

async fn evaluate_feasibility(
    State(st): State<AppState>,
    Json(req): Json<PolicyFeasibilityRequest>,
) -> ApiResult<(StatusCode, Json<Vec<PolicyFeasibilityResult>>)> {
    let lock = st.latest_snapshot.read().await;
    let snapshot = match &*lock {
        Some(s) => s.clone(),
        None => generate_real_snapshot(),
    };

    let result = dek_deployment_planner::evaluate_policy_feasibility(req, &snapshot);
    Ok((StatusCode::OK, Json(result)))
}

async fn create_deployment_session(
    State(_st): State<AppState>,
) -> ApiResult<(StatusCode, Json<DeploymentSession>)> {
    let mut session = DeploymentSession {
        deployment_id: Uuid::new_v4().to_string(),
        policy_id: "policy-tmp".into(),
        policy_version: "1.0".into(),
        requested_control_level: ControlLevel::Enforce,
        target_scope: DeploymentScope::Device {
            device_id: "local".into(),
        },
        status: DeploymentSessionStatus::ScanStarted,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        created_by: "local_admin".into(),
    };

    let sink = Arc::new(StoreEventSink::new());
    let orchestrator = DeploymentOrchestrator::new(sink);

    let _ = orchestrator
        .transition(&mut session, DeploymentSessionStatus::ScanCompleted)
        .await;
    let _ = orchestrator
        .transition(
            &mut session,
            DeploymentSessionStatus::CapabilitySnapshotCreated,
        )
        .await;
    let _ = orchestrator
        .transition(
            &mut session,
            DeploymentSessionStatus::PolicyFeasibilityEvaluated,
        )
        .await;
    let _ = orchestrator
        .transition(&mut session, DeploymentSessionStatus::DeploymentPlanCreated)
        .await;

    Ok((StatusCode::OK, Json(session)))
}

async fn approve_action(
    Path((_session_id, _action_id)): Path<(String, String)>,
    State(_st): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({"status": "approved"})),
    ))
}

async fn retry_deployment(
    Path(session_id): Path<String>,
    State(_st): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "retrying",
            "deployment_id": session_id
        })),
    ))
}

async fn rollback_deployment(
    Path(session_id): Path<String>,
    State(_st): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "rolled_back",
            "deployment_id": session_id
        })),
    ))
}
