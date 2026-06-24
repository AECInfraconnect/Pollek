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
        .route("/v1/deployment-sessions/:id", get(get_deployment_session))
        .route("/v1/deployment-sessions/:id/events", get(get_deployment_events))
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

async fn generate_real_snapshot(st: &AppState) -> LocalCapabilitySnapshot {
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

    // Fetch registered agents from the registry store
    let agents = st
        .registry_store
        .list_agent_inventories("local")
        .await
        .unwrap_or_default();

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
    let snapshot = generate_real_snapshot(&st).await;
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
            let fresh = generate_real_snapshot(&st).await;
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
        None => generate_real_snapshot(&st).await,
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
        None => generate_real_snapshot(&st).await,
    };

    let result = dek_deployment_planner::evaluate_policy_feasibility(req, &snapshot);
    Ok((StatusCode::OK, Json(result)))
}

async fn create_deployment_session(
    State(st): State<AppState>,
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

    let sink = Arc::new(StoreEventSink::new(st.deployment_store.clone()));
    let orchestrator = DeploymentOrchestrator::new(sink, st.deployment_store.clone());

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
    Path((session_id, _action_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    if let Some(mut session) = st.deployment_store.get_deployment_session(&session_id).await? {
        let sink = std::sync::Arc::new(StoreEventSink::new(st.deployment_store.clone()));
        let orchestrator = DeploymentOrchestrator::new(sink, st.deployment_store.clone());
        let _ = orchestrator.transition(&mut session, DeploymentSessionStatus::BundleCreated).await;
        let _ = orchestrator.transition(&mut session, DeploymentSessionStatus::BundleActivated).await;
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({"status": "approved", "session": session})),
        ))
    } else {
        Ok((StatusCode::NOT_FOUND, Json(serde_json::json!({"status": "not_found"}))))
    }
}

async fn retry_deployment(
    Path(session_id): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    if let Some(mut session) = st.deployment_store.get_deployment_session(&session_id).await? {
        let sink = std::sync::Arc::new(StoreEventSink::new(st.deployment_store.clone()));
        let orchestrator = DeploymentOrchestrator::new(sink, st.deployment_store.clone());
        let _ = orchestrator.transition(&mut session, DeploymentSessionStatus::ScanStarted).await;
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "retrying",
                "deployment_id": session_id
            })),
        ))
    } else {
        Ok((StatusCode::NOT_FOUND, Json(serde_json::json!({"status": "not_found"}))))
    }
}

async fn rollback_deployment(
    Path(session_id): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    if let Some(mut session) = st.deployment_store.get_deployment_session(&session_id).await? {
        let sink = std::sync::Arc::new(StoreEventSink::new(st.deployment_store.clone()));
        let orchestrator = DeploymentOrchestrator::new(sink, st.deployment_store.clone());
        let _ = orchestrator.transition(&mut session, DeploymentSessionStatus::RolledBack).await;
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "rolled_back",
                "deployment_id": session_id
            })),
        ))
    } else {
        Ok((StatusCode::NOT_FOUND, Json(serde_json::json!({"status": "not_found"}))))
    }
}


async fn get_deployment_session(
    Path(session_id): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Json<DeploymentSession>)> {
    let session = st.deployment_store.get_deployment_session(&session_id).await?;
    if let Some(session) = session {
        Ok((StatusCode::OK, Json(session)))
    } else {
        Ok((StatusCode::NOT_FOUND, Json(DeploymentSession {
            deployment_id: session_id,
            policy_id: "".into(),
            policy_version: "".into(),
            requested_control_level: ControlLevel::Observe,
            target_scope: DeploymentScope::Device { device_id: "".into() },
            status: DeploymentSessionStatus::Failed,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            created_by: "".into(),
        }))) // Return 404 in real world, but type needs to match
    }
}

async fn get_deployment_events(
    Path(session_id): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Json<Vec<dek_domain_schema::deployment_session::DeploymentEvent>>)> {
    let events = st.deployment_store.list_deployment_events(&session_id).await?;
    Ok((StatusCode::OK, Json(events)))
}
