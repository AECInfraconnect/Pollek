// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use axum::{
    extract::Path,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use dek_enforcement_api::planner::{
    negotiate, ControlDomain, ControlLevel, ControlMethodCap, LocalCapabilitySnapshot,
    MethodStatus, PolicyFeasibilityResult,
};
use serde::{Deserialize, Serialize};

use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/capability-snapshot",
            get(get_host_capabilities),
        )
        .route("/v1/tenants/:tenant/scan", post(scan_agents))
        .route("/v1/tenants/:tenant/scans/:job", get(get_scan_result))
        .route(
            "/v1/tenants/:tenant/policy-suggestions",
            post(get_policy_suggestions),
        )
        .route(
            "/v1/tenants/:tenant/policies/feasibility",
            post(evaluate_feasibility),
        )
        .route(
            "/v1/tenants/:tenant/deployment-sessions",
            post(create_deploy_session),
        )
        .route(
            "/v1/tenants/:tenant/deployment-sessions/:id/confirm",
            post(confirm_deploy_session),
        )
        .route(
            "/v1/tenants/:tenant/deployment-sessions/:id/apply",
            post(apply_deploy_session),
        )
}

fn is_elevated() -> bool {
    // Basic stub for elevation check
    #[cfg(target_os = "windows")]
    {
        // For testing/mocking, assume true or implement real check
        true
    }
    #[cfg(not(target_os = "windows"))]
    {
        true
    }
}

fn get_current_snapshot() -> LocalCapabilitySnapshot {
    if std::env::consts::OS == "windows" {
        return LocalCapabilitySnapshot {
            control_methods: vec![ControlMethodCap {
                id: "windows_wfp_um".into(),
                domains: vec![ControlDomain::Network],
                max_level: if is_elevated() {
                    ControlLevel::Enforce
                } else {
                    ControlLevel::Observe
                },
                status: if is_elevated() {
                    MethodStatus::Available
                } else {
                    MethodStatus::NeedsPermission
                },
            }],
        };
    }

    let method_id = match std::env::consts::OS {
        "macos" => "macos_netext",
        _ => "linux_ebpf",
    };
    LocalCapabilitySnapshot {
        control_methods: vec![ControlMethodCap {
            id: method_id.into(),
            domains: vec![ControlDomain::Network],
            max_level: ControlLevel::Enforce,
            status: MethodStatus::Available,
        }],
    }
}

async fn get_host_capabilities(
    Path(_tenant): Path<String>,
) -> ApiResult<(StatusCode, Json<LocalCapabilitySnapshot>)> {
    Ok((StatusCode::OK, Json(get_current_snapshot())))
}

#[derive(Serialize)]
struct ScanResponse {
    job_id: String,
}

async fn scan_agents(Path(_tenant): Path<String>) -> ApiResult<(StatusCode, Json<ScanResponse>)> {
    Ok((
        StatusCode::OK,
        Json(ScanResponse {
            job_id: "job-123".into(),
        }),
    ))
}

async fn get_scan_result(
    Path((_tenant, job_id)): Path<(String, String)>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({"job_id": job_id, "status": "completed"})),
    ))
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
}

async fn get_policy_suggestions(
    Path(_tenant): Path<String>,
    Json(_req): Json<SuggestionsRequest>,
) -> ApiResult<(StatusCode, Json<Vec<PolicySuggestion>>)> {
    Ok((
        StatusCode::OK,
        Json(vec![PolicySuggestion {
            id: "sugg-1".into(),
            title_th: "จำกัดการเข้าถึงเครือข่าย".into(),
            title_en: "Restrict Network Access".into(),
            domains: vec!["network".into()],
            recommended_level: "enforce".into(),
        }]),
    ))
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct FeasibilityRequest {
    candidate: dek_agent_discovery::model::DiscoveredAgentCandidateV2,
    requested_level: ControlLevel,
}

async fn evaluate_feasibility(
    Path(_tenant): Path<String>,
    Json(req): Json<FeasibilityRequest>,
) -> ApiResult<(StatusCode, Json<PolicyFeasibilityResult>)> {
    let snap = get_current_snapshot();
    let res = dek_enforcement_api::feasibility::assess(&req.candidate, req.requested_level, &snap);
    Ok((StatusCode::OK, Json(res)))
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct CreateDeployRequest {
    candidate: dek_agent_discovery::model::DiscoveredAgentCandidateV2,
    requested_level: ControlLevel,
}

#[derive(Serialize)]
struct DeploySession {
    id: String,
    feasibility: PolicyFeasibilityResult,
    status: String,
}

async fn create_deploy_session(
    Path(_tenant): Path<String>,
    Json(req): Json<CreateDeployRequest>,
) -> ApiResult<(StatusCode, Json<DeploySession>)> {
    let snap = get_current_snapshot();
    let res = dek_enforcement_api::feasibility::assess(&req.candidate, req.requested_level, &snap);
    Ok((
        StatusCode::OK,
        Json(DeploySession {
            id: "sess-123".into(),
            feasibility: res,
            status: "pending".into(),
        }),
    ))
}

async fn confirm_deploy_session(
    Path((_tenant, _id)): Path<(String, String)>,
) -> ApiResult<(
    StatusCode,
    Json<dek_enforcement_api::planner::ControlMethodPlan>,
)> {
    // In a real app we would load the session and candidate from DB.
    // Here we just mock it for compilation based on previous stub logic.
    let snap = get_current_snapshot();
    let pol = dek_enforcement_api::planner::Policy {
        id: "mock_pol".into(),
        requested_level: dek_enforcement_api::planner::ControlLevel::Enforce,
    };
    let res = dek_enforcement_api::planner::assess_feasibility(&pol, &snap);
    let plan = negotiate(&res);
    Ok((StatusCode::OK, Json(plan)))
}

#[derive(Serialize)]
struct DeployReport {
    status: String,
}

async fn apply_deploy_session(
    Path((_tenant, _id)): Path<(String, String)>,
) -> ApiResult<(StatusCode, Json<DeployReport>)> {
    Ok((
        StatusCode::OK,
        Json(DeployReport {
            status: "success".into(),
        }),
    ))
}
