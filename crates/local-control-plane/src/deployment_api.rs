// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use dek_domain_schema::deployment_session::{
    DeploymentEvent, DeploymentSession, DeploymentSessionStatus, RoutingPlan,
};
use dek_policy_router::route_planner::RoutePlanner;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    deployment_orchestrator::{DeploymentEventSink, DeploymentOrchestrator},
    error::{ApiError, ApiResult},
    state::AppState,
};

pub struct StoreEventSink {
    store: std::sync::Arc<dyn crate::store::TelemetryStore>,
    tenant_id: String,
}

impl DeploymentEventSink for StoreEventSink {
    async fn emit(&self, event: DeploymentEvent) -> anyhow::Result<()> {
        let store = self.store.clone();
        let tenant_id = self.tenant_id.clone();
        let payload = serde_json::to_value(&event).unwrap_or_default();
        let _ = store
            .put_telemetry(&tenant_id, "deployment_event", &event.event_id, &payload)
            .await;
        Ok(())
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/policies/:policy_id/deploy-plan",
            post(create_deploy_plan),
        )
        .route("/v1/tenants/:tenant/deployments", post(create_deployment))
        .route(
            "/v1/tenants/:tenant/deployments/:deployment_id",
            get(get_deployment),
        )
        .route(
            "/v1/tenants/:tenant/deployments/:deployment_id/events",
            get(get_deployment_events),
        )
        .route(
            "/v1/tenants/:tenant/deployments/:deployment_id/actions/:action_id/approve",
            post(approve_action),
        )
        .route(
            "/v1/tenants/:tenant/deployments/:deployment_id/retry",
            post(retry_deployment),
        )
        .route(
            "/v1/tenants/:tenant/deployments/:deployment_id/rollback",
            post(rollback_deployment),
        )
        .route("/v1/agents/:agent_id/timeline", get(get_agent_timeline))
        .route("/v1/capabilities/local", get(get_local_capabilities))
        .route(
            "/v1/enforcement-layers/status",
            get(get_enforcement_layers_status),
        )
        .route("/v1/system/profile", get(get_system_profile))
}

async fn create_deploy_plan(
    Path((_tenant, _policy_id)): Path<(String, String)>,
    State(_st): State<AppState>,
    Json(session): Json<DeploymentSession>,
) -> ApiResult<Json<RoutingPlan>> {
    // Mock device capabilities for now (in a real system we fetch from capability registry)
    let device_caps = dek_domain_schema::capabilities::DeviceCapabilityReport {
        device_id: "local".into(),
        os: dek_domain_schema::capabilities::OsProfile {
            r#type: "windows".into(),
            version: "11".into(),
            arch: "x86_64".into(),
        },
        peps: vec![],
        pdps: vec![],
        scanned_at: chrono::Utc::now(),
    };

    let plan = RoutePlanner::plan_route(&session, &device_caps)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(plan))
}

async fn get_system_profile() -> ApiResult<Json<serde_json::Value>> {
    let path_str = dek_config::paths::get_bootstrap_path()
        .to_string_lossy()
        .into_owned();
    let cfg = dek_config::BootstrapConfig::load_or_default(&path_str).unwrap_or_else(|_| {
        dek_config::BootstrapConfig {
            device_id: "unknown".into(),
            mtls: dek_config::MtlsConfig {
                client_cert_path: "".into(),
                client_key_path: "".into(),
                root_ca_path: "".into(),
            },
            pinned_bundle_public_key: "".into(),
            cloud_url: "http://127.0.0.1:3000".into(),
            spiffe_id: None,
            tenant_id: Some("local".into()),
            local_api_token: None,
        }
    });

    let mode = match cfg.tenant_id.as_deref() {
        Some("sovereign") => "sovereign",
        Some("local") => "local",
        _ if cfg.cloud_url.contains("127.0.0.1") || cfg.cloud_url.contains("localhost") => "local",
        _ => "cloud",
    };

    Ok(Json(serde_json::json!({
        "mode": mode,
        "cloud_url": cfg.cloud_url,
        "tenant_id": cfg.tenant_id,
        "device_id": cfg.device_id,
        "is_sovereign": mode == "sovereign",
        "is_local": mode == "local" || mode == "sovereign",
    })))
}

async fn create_deployment(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(mut session): Json<DeploymentSession>,
) -> ApiResult<Json<Value>> {
    session.deployment_id = Uuid::new_v4().to_string();
    session.status = DeploymentSessionStatus::ScanStarted;

    let sink = std::sync::Arc::new(StoreEventSink {
        store: st.telemetry_store.clone(),
        tenant_id: tenant.clone(),
    });
    let orchestrator = DeploymentOrchestrator::new(sink.clone(), st.deployment_store.clone());

    // Perform warm check
    let device_caps = dek_capability_registry::detect::detect_pep_capabilities();
    let caps_report = dek_domain_schema::capabilities::DeviceCapabilityReport {
        device_id: "local".into(),
        os: dek_domain_schema::capabilities::OsProfile {
            r#type: "windows".into(),
            version: "11".into(),
            arch: "x86_64".into(),
        },
        peps: device_caps.into_iter().map(|c| dek_domain_schema::capabilities::PepCapabilityStatus {
            layer: match c.r#type.as_str() {
                "linux-ebpf" => dek_domain_schema::deployment_session::EnforcementLayer::EbpfNetwork,
                "windows-wfp" => dek_domain_schema::deployment_session::EnforcementLayer::WindowsWfp,
                "macos-nefilter" => dek_domain_schema::deployment_session::EnforcementLayer::MacosNetworkExtension,
                "mcp-stdio" => dek_domain_schema::deployment_session::EnforcementLayer::McpStdioWrapper,
                "mcp-http" => dek_domain_schema::deployment_session::EnforcementLayer::McpProxy,
                _ => dek_domain_schema::deployment_session::EnforcementLayer::ObserveOnly,
            },
            status: c.status,
            confidence: 1.0,
            detected_version: None,
            reason_code: "ok".into(),
            user_message: c.status_reason.unwrap_or_else(|| dek_domain_schema::deployment_session::LocalizedText {
                en: "OK".into(),
                th: "OK".into(),
            }),
            next_action: None,
        }).collect(),
        pdps: vec![],
        scanned_at: chrono::Utc::now(),
    };

    if let Ok(_plan) =
        dek_policy_router::route_planner::RoutePlanner::plan_route(&session, &caps_report)
    {
        sink.emit(DeploymentEvent {
            event_id: Uuid::new_v4().to_string(),
            deployment_id: session.deployment_id.clone(),
            agent_id: Some("local".to_string()),
            entity_id: None,
            policy_id: session.policy_id.clone(),
            phase: dek_domain_schema::deployment_session::DeploymentPhase::CapabilityCheck,
            status: dek_domain_schema::deployment_session::EventStatus::Info,
            title: dek_domain_schema::deployment_session::LocalizedText {
                en: "Capability Warm Check".to_string(),
                th: "ตรวจสอบสถานะการทำงาน".to_string(),
            },
            detail: dek_domain_schema::deployment_session::LocalizedText {
                en: "Performing capability warm check...".to_string(),
                th: "กำลังตรวจสอบความพร้อมการบังคับใช้...".to_string(),
            },
            technical_detail: None,
            user_action: None,
            created_at: chrono::Utc::now(),
            correlation_id: session.deployment_id.clone(),
        })
        .await
        .unwrap_or_default();

        // Simulate warm check
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        let warm_check_ok = true;

        if !warm_check_ok {
            orchestrator
                .transition(&mut session, DeploymentSessionStatus::Failed)
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
            return Err(ApiError::Internal(anyhow::anyhow!("Warm check failed")));
        }
    }

    orchestrator
        .transition(&mut session, DeploymentSessionStatus::Active)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(
        serde_json::json!({ "deployment_id": session.deployment_id, "status": "Active" }),
    ))
}

async fn get_deployment(
    Path((_tenant, deployment_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<DeploymentSession>> {
    let session = st
        .deployment_store
        .get_deployment_session(&deployment_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?
        .ok_or_else(|| ApiError::NotFound("Deployment not found".into()))?;
    Ok(Json(session))
}

async fn get_deployment_events(
    Path((_tenant, deployment_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<Vec<DeploymentEvent>>> {
    let events = st
        .deployment_store
        .list_deployment_events(&deployment_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    Ok(Json(events))
}

async fn approve_action(
    Path((tenant, deployment_id, _action_id)): Path<(String, String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<DeploymentSession>> {
    let mut session = st
        .deployment_store
        .get_deployment_session(&deployment_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?
        .ok_or_else(|| ApiError::NotFound("Deployment not found".into()))?;

    let sink = std::sync::Arc::new(StoreEventSink {
        store: st.telemetry_store.clone(),
        tenant_id: tenant,
    });
    let orchestrator = DeploymentOrchestrator::new(sink, st.deployment_store.clone());

    orchestrator
        .transition(&mut session, DeploymentSessionStatus::Active)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(session))
}

async fn retry_deployment(
    Path((tenant, deployment_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<DeploymentSession>> {
    let mut session = st
        .deployment_store
        .get_deployment_session(&deployment_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?
        .ok_or_else(|| ApiError::NotFound("Deployment not found".into()))?;

    let sink = std::sync::Arc::new(StoreEventSink {
        store: st.telemetry_store.clone(),
        tenant_id: tenant,
    });
    let orchestrator = DeploymentOrchestrator::new(sink, st.deployment_store.clone());

    orchestrator
        .transition(&mut session, DeploymentSessionStatus::ScanStarted)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(session))
}

async fn rollback_deployment(
    Path((tenant, deployment_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<DeploymentSession>> {
    let mut session = st
        .deployment_store
        .get_deployment_session(&deployment_id)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?
        .ok_or_else(|| ApiError::NotFound("Deployment not found".into()))?;

    let sink = std::sync::Arc::new(StoreEventSink {
        store: st.telemetry_store.clone(),
        tenant_id: tenant,
    });
    let orchestrator = DeploymentOrchestrator::new(sink, st.deployment_store.clone());

    orchestrator
        .transition(&mut session, DeploymentSessionStatus::RolledBack)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(session))
}

async fn get_agent_timeline(
    Path(agent_id): Path<String>,
    State(_st): State<AppState>,
) -> ApiResult<Json<Value>> {
    Ok(Json(serde_json::json!({
        "agent_id": agent_id,
        "events": []
    })))
}
async fn get_local_capabilities(State(_st): State<AppState>) -> ApiResult<Json<Value>> {
    let caps = dek_capability_registry::detect::detect_pep_capabilities();
    Ok(Json(serde_json::json!({
        "peps": caps
    })))
}

async fn get_enforcement_layers_status(State(_st): State<AppState>) -> ApiResult<Json<Value>> {
    let caps = dek_capability_registry::detect::detect_pep_capabilities();
    Ok(Json(serde_json::json!({
        "layers": caps
    })))
}
