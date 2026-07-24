// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use dek_domain_schema::{
    capability_snapshot_v2::{
        ContractCompatibilityStatus, ControlDomainV2, ControlLevelV2, ControlMethodCapabilityV2,
        InstallState, LocalCapabilitySnapshotV2, MethodMaturity, MethodReadiness,
        ObservationSourceCapability, OsInfoV2, RuntimeMode, SetupAction, WarmCheckStatus,
    },
    deployment_session::{
        DeploymentEvent, DeploymentPhase, DeploymentScope, DeploymentSession,
        DeploymentSessionStatus, EventStatus, LocalizedText, UserAction,
    },
    scan_session::{
        DiscoverySourceKind, DiscoverySourceResult, ScanSessionV2, ScanSourceState, ScanStatus,
    },
};
use dek_enforcement_api::{
    planner::{
        negotiate, ControlDomain, ControlLevel, ControlMethodCap, LocalCapabilitySnapshot,
        MethodStatus, Policy, PolicyFeasibilityResult,
    },
    security_coverage::{assess_policy_coverage, CoverageRequest},
};
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

mod capability;
mod deploy;
mod scan;
use capability::*;
use deploy::*;
use scan::*;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/capability-snapshot",
            get(get_host_capabilities),
        )
        .route("/v1/host/capabilities", get(get_host_capabilities_root))
        .route(
            "/v1/tenants/:tenant/devices/:device/capability-snapshot-v2",
            get(get_host_capabilities_v2),
        )
        .route(
            "/v1/tenants/:tenant/devices/:device/capability-refresh",
            post(refresh_host_capabilities_v2),
        )
        .route("/v1/tenants/:tenant/scan", post(scan_agents))
        .route("/v1/tenants/:tenant/scans/:job", get(get_scan_result))
        .route(
            "/v1/tenants/:tenant/scan-sessions",
            post(create_scan_session),
        )
        .route(
            "/v1/tenants/:tenant/scan-sessions/:scan_id",
            get(get_scan_session),
        )
        .route(
            "/v1/tenants/:tenant/scan-sessions/:scan_id/events",
            get(get_scan_session_events),
        )
        .route(
            "/v1/tenants/:tenant/policy-suggestions",
            post(get_policy_suggestions),
        )
        .route(
            "/v1/tenants/:tenant/policies/feasibility",
            post(evaluate_feasibility),
        )
        .route(
            "/v1/tenants/:tenant/policy-first/security-coverage",
            post(evaluate_security_coverage),
        )
        .route(
            "/v1/tenants/:tenant/policy-first/protection-preview",
            post(evaluate_security_coverage),
        )
        .route(
            "/v1/tenants/:tenant/deployment-sessions",
            post(create_deploy_session),
        )
        .route(
            "/v1/tenants/:tenant/deployment-sessions/:id",
            get(get_deploy_session),
        )
        .route(
            "/v1/tenants/:tenant/deployment-sessions/:id/timeline",
            get(get_deploy_timeline),
        )
        .route(
            "/v1/tenants/:tenant/deployment-sessions/:id/confirm",
            post(confirm_deploy_session),
        )
        .route(
            "/v1/tenants/:tenant/deployment-sessions/:id/approve",
            post(confirm_deploy_session),
        )
        .route(
            "/v1/tenants/:tenant/deployment-sessions/:id/actions/:action_id/approve",
            post(approve_deploy_session_action),
        )
        .route(
            "/v1/tenants/:tenant/deployment-sessions/:id/apply",
            post(apply_deploy_session),
        )
        .route(
            "/v1/tenants/:tenant/deployment-sessions/:id/rollback",
            post(rollback_deploy_session),
        )
}

async fn get_host_capabilities(
    Path(tenant): Path<String>,
    State(_state): State<AppState>,
) -> ApiResult<(StatusCode, Json<LocalCapabilitySnapshot>)> {
    let device_id = local_device_id();
    let snapshot = build_capability_snapshot_v2(&tenant, &device_id, RuntimeMode::DesktopSimple);
    Ok((StatusCode::OK, Json(legacy_snapshot_from_v2(&snapshot))))
}

/// Root-level alias documented by the deprecated pep-capabilities endpoint.
async fn get_host_capabilities_root(
    State(_state): State<AppState>,
) -> ApiResult<(StatusCode, Json<LocalCapabilitySnapshot>)> {
    let device_id = local_device_id();
    let snapshot = build_capability_snapshot_v2("local", &device_id, RuntimeMode::DesktopSimple);
    Ok((StatusCode::OK, Json(legacy_snapshot_from_v2(&snapshot))))
}

async fn get_host_capabilities_v2(
    Path((tenant, device)): Path<(String, String)>,
    Query(query): Query<ModeQuery>,
    State(state): State<AppState>,
) -> ApiResult<(StatusCode, Json<LocalCapabilitySnapshotV2>)> {
    let demo_profile = demo_profile_from_query(&query);
    let device_id = if let Some(profile) = &demo_profile {
        profile.device_id()
    } else if device == "local" {
        local_device_id()
    } else {
        device
    };
    let snapshot = build_capability_snapshot_v2_for(
        &tenant,
        &device_id,
        parse_mode(query.mode.as_deref()),
        demo_profile.as_ref(),
    );
    if demo_profile.is_none() {
        let mut guard = state.latest_snapshot.write().await;
        *guard = Some(dek_capability_registry::LocalCapabilitySnapshot {
            snapshot_id: format!("snap_{}", uuid::Uuid::new_v4()),
            device_id: snapshot.device_id.clone(),
            os: dek_capability_registry::OsInfo {
                r#type: snapshot.os.family.clone(),
                version: snapshot.os.version.clone(),
                arch: snapshot.os.arch.clone(),
            },
            agents: Vec::new(),
            methods: Vec::new(),
            generated_at: snapshot.generated_at,
        });
    }
    Ok((StatusCode::OK, Json(snapshot)))
}

async fn refresh_host_capabilities_v2(
    Path((tenant, device)): Path<(String, String)>,
    Query(query): Query<ModeQuery>,
    State(state): State<AppState>,
) -> ApiResult<(StatusCode, Json<LocalCapabilitySnapshotV2>)> {
    get_host_capabilities_v2(Path((tenant, device)), Query(query), State(state)).await
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct SuggestionsRequest {
    agents: Vec<String>,
}

#[derive(Serialize)]
struct PolicySuggestion {
    id: String,
    title_th: String,
    title_en: String,
    domains: Vec<String>,
    recommended_level: String,
    reason_code: String,
}

async fn get_policy_suggestions(
    Path(tenant): Path<String>,
    State(_state): State<AppState>,
    Json(req): Json<SuggestionsRequest>,
) -> ApiResult<(StatusCode, Json<Vec<PolicySuggestion>>)> {
    let has_agents = !req.agents.is_empty();
    let id = if has_agents {
        "pii.redact_before_external_llm"
    } else {
        "observe-only-baseline"
    };
    Ok((
        StatusCode::OK,
        Json(vec![PolicySuggestion {
            id: id.into(),
            title_th: "ป้องกันข้อมูลอ่อนไหวก่อนออกไปยัง AI ภายนอก".into(),
            title_en: "Protect sensitive data before external AI egress".into(),
            domains: vec![
                "mcp_tool_call".into(),
                "prompt_content".into(),
                "network_egress".into(),
            ],
            recommended_level: if tenant == "local" { "warn" } else { "ask" }.into(),
            reason_code: "policy_first_sensitive_data_default".into(),
        }]),
    ))
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct FeasibilityRequest {
    /// Full discovery candidate (rich flow, e.g. AutoDiscovery detail).
    candidate: Option<dek_agent_discovery::model::DiscoveredAgentCandidateV2>,
    /// Lightweight policy reference (simple wizard sends the picked suggestion here).
    policy: Option<serde_json::Value>,
    requested_level: ControlLevel,
    policy_id: Option<String>,
}

fn policy_id_from_value(policy: Option<&serde_json::Value>) -> Option<String> {
    policy
        .and_then(|p| p.get("id").or_else(|| p.get("policy_id")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

async fn evaluate_feasibility(
    Path(tenant): Path<String>,
    State(_state): State<AppState>,
    Json(req): Json<FeasibilityRequest>,
) -> ApiResult<(StatusCode, Json<PolicyFeasibilityResult>)> {
    let device_id = local_device_id();
    let snap_v2 = build_capability_snapshot_v2(&tenant, &device_id, RuntimeMode::DesktopSimple);
    let snap = legacy_snapshot_from_v2(&snap_v2);
    let policy_id = req
        .policy_id
        .clone()
        .or_else(|| policy_id_from_value(req.policy.as_ref()));
    let mut res = if let Some(candidate) = &req.candidate {
        dek_enforcement_api::feasibility::assess(candidate, req.requested_level, &snap)
    } else {
        // Policy-only flow: derive required control domains from the policy id.
        let pol = dek_enforcement_api::planner::Policy {
            id: policy_id
                .clone()
                .unwrap_or_else(|| "observe-only-baseline".into()),
            requested_level: req.requested_level,
        };
        dek_enforcement_api::planner::assess_feasibility(&pol, &snap)
    };
    if let Some(policy_id) = policy_id {
        res.policy_id = policy_id;
    }
    Ok((StatusCode::OK, Json(res)))
}

#[derive(Deserialize)]
struct SecurityCoverageRequest {
    #[serde(default)]
    agent_ids: Vec<String>,
    #[serde(default)]
    entity_ids: Vec<String>,
    #[serde(default)]
    policy_ids: Vec<String>,
    #[serde(default)]
    requested_level: Option<ControlLevelV2>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    local_cloud_profile: Option<String>,
    #[serde(default)]
    demo_os: Option<String>,
    #[serde(default)]
    demo_profile: Option<String>,
}

async fn evaluate_security_coverage(
    Path(tenant): Path<String>,
    State(_state): State<AppState>,
    Json(req): Json<SecurityCoverageRequest>,
) -> ApiResult<(StatusCode, Json<dek_domain_schema::PolicyCoverageReport>)> {
    let device_id = local_device_id();
    let mode = parse_mode(req.mode.as_deref());
    let demo_profile = demo_profile_from_parts(req.demo_os.as_deref(), req.demo_profile.as_deref());
    let device_id = demo_profile
        .as_ref()
        .map(DemoProfile::device_id)
        .unwrap_or(device_id);
    let snapshot =
        build_capability_snapshot_v2_for(&tenant, &device_id, mode.clone(), demo_profile.as_ref());
    let policy_id = req
        .policy_ids
        .first()
        .cloned()
        .unwrap_or_else(|| "pii.redact_before_external_llm".into());
    let report = assess_policy_coverage(
        CoverageRequest {
            tenant_id: tenant,
            device_id,
            agent_id: req.agent_ids.first().cloned(),
            entity_id: req.entity_ids.first().cloned(),
            policy_id,
            requested_level: req.requested_level.unwrap_or(ControlLevelV2::Enforce),
            mode,
            local_cloud_profile: req
                .local_cloud_profile
                .unwrap_or_else(|| "local_only".into()),
            evidence_ids: Vec::new(),
        },
        &snapshot,
    );
    Ok((StatusCode::OK, Json(report)))
}
