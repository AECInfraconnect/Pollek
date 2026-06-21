// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::{error::ApiResult, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/pep-capabilities",
            get(list_capabilities),
        )
        .route(
            "/v1/tenants/:tenant/pep-capabilities/check",
            post(check_capabilities),
        )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepCapabilityCheckRequest {
    pub preset_id: String,
    pub target_os: String,
    pub requested_pep_types: Vec<String>,
}

async fn list_capabilities(
    Path(tenant): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let active_runtimes = state
        .pdp_store
        .list_runtimes(&tenant)
        .await
        .unwrap_or_default();
    let has_linux_ebpf = active_runtimes
        .iter()
        .any(|r| r.get("id").and_then(|v| v.as_str()) == Some("linux_ebpf_pep"));

    Ok(Json(serde_json::json!({
        "schema_version": "pep-capabilities-list.v2",
        "capabilities": [
            {
                "pep_type": "linux_ebpf",
                "status": if has_linux_ebpf { "available" } else { "not_installed" },
                "mode": "enforce",
                "maturity": "enforce_beta"
            },
            {
                "pep_type": "windows_wfp",
                "status": "not_available",
                "mode": "observe_only",
                "maturity": "stub",
                "reason": "not running on windows"
            },
            {
                "pep_type": "macos_nefilter",
                "status": "not_available",
                "mode": "observe_only",
                "maturity": "stub",
                "reason": "not running on macOS"
            },
            {
                "pep_type": "http_gateway",
                "status": "available", // Mock true if empty for backward compat locally
                "mode": "enforce",
                "maturity": "production"
            },
            {
                "pep_type": "mcp_proxy",
                "status": "available",
                "mode": "enforce",
                "maturity": "production"
            },
            {
                "pep_type": "stdio_wrapper",
                "status": "available",
                "mode": "enforce",
                "maturity": "production"
            },
            {
                "pep_type": "execution_sandbox",
                "status": "available",
                "mode": "observe_only",
                "maturity": "stub",
                "reason": "Currently mocks success without actual isolation"
            },
            {
                "pep_type": "a2a_mediator",
                "status": "available",
                "mode": "observe_only",
                "maturity": "stub",
                "reason": "Cryptographic signature validation is mocked"
            }
        ]
    })))
}

async fn check_capabilities(
    Path(tenant): Path<String>,
    State(state): State<AppState>,
    Json(req): Json<PepCapabilityCheckRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let active_runtimes = state
        .pdp_store
        .list_runtimes(&tenant)
        .await
        .unwrap_or_default();
    let has_linux_ebpf = active_runtimes
        .iter()
        .any(|r| r.get("id").and_then(|v| v.as_str()) == Some("linux_ebpf_pep"));

    let mut recommended = "".to_string();
    if req.requested_pep_types.contains(&"linux_ebpf".to_string())
        && req.target_os == "linux"
        && has_linux_ebpf
    {
        recommended = "linux_ebpf".to_string();
    } else if req.requested_pep_types.contains(&"mcp_proxy".to_string()) {
        recommended = "mcp_proxy".to_string();
    } else if req
        .requested_pep_types
        .contains(&"http_gateway".to_string())
    {
        recommended = "http_gateway".to_string();
    } else if let Some(first) = req.requested_pep_types.first() {
        recommended = first.clone();
    }

    Ok(Json(serde_json::json!({
        "recommended": recommended,
        "capabilities": [
            {
                "pep_type": "linux_ebpf",
                "status": if req.target_os == "linux" && has_linux_ebpf { "available" } else { "not_installed" },
                "mode": if req.target_os == "linux" && has_linux_ebpf { "enforce" } else { "observe_only" },
                "maturity": "enforce_beta",
                "reason": if req.target_os != "linux" { "not running on linux" } else if !has_linux_ebpf { "PEP is not installed or active" } else { "" }
            },
            {
                "pep_type": "windows_wfp",
                "status": if req.target_os == "windows" { "available" } else { "not_available" },
                "mode": if req.target_os == "windows" { "enforce" } else { "observe_only" },
                "maturity": "stub",
                "reason": if req.target_os != "windows" { "not running on windows" } else { "" }
            },
            {
                "pep_type": "mcp_proxy",
                "status": "available",
                "mode": "enforce",
                "maturity": "production"
            },
            {
                "pep_type": "execution_sandbox",
                "status": "available",
                "mode": "observe_only",
                "maturity": "stub",
                "reason": "Currently mocks success without actual isolation"
            },
            {
                "pep_type": "a2a_mediator",
                "status": "available",
                "mode": "observe_only",
                "maturity": "stub",
                "reason": "Cryptographic signature validation is mocked"
            }
        ]
    })))
}
