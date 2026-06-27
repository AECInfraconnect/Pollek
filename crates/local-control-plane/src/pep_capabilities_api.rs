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
        .route("/v1/capabilities", get(get_global_capabilities))
        .route(
            "/v1/tenants/:tenant/pep-capabilities",
            get(list_capabilities),
        )
        .route(
            "/v1/tenants/:tenant/pep-capabilities/check",
            post(check_capabilities),
        )
        .route("/v1/tenants/:tenant/peps/:id/probe", post(probe_pep))
        .route("/v1/tenants/:tenant/peps/:id/bind", post(bind_pep))
}

async fn get_global_capabilities() -> ApiResult<Json<serde_json::Value>> {
    let _local_caps = dek_capability_registry::detect::detect_pep_capabilities();
    let reg = dek_capability_registry::CapabilityRegistry::new("local".into(), "1.0.0".into());
    let full_caps = reg.gather();
    Ok(Json(serde_json::json!({
        "status": "success",
        "capabilities": full_caps,
    })))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepCapabilityCheckRequest {
    pub preset_id: String,
    pub requested_pep_types: Vec<String>,
}

async fn list_capabilities(
    Path(_tenant): Path<String>,
    State(_state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let host = crate::enforcement_plan_api::detect_host();

    Ok(Json(serde_json::json!({
        "schema_version": "pep-capabilities-list.v2",
        "capabilities": [
            {
                "pep_type": "linux_ebpf",
                "status": if host.os == "linux" && host.linux_ebpf { "available" } else { "not_installed" },
                "mode": if host.linux_ebpf { "enforce" } else { "observe_only" },
                "maturity": "enforce_beta",
                "reason": if host.os != "linux" { "not running on linux" } else if !host.linux_ebpf { "eBPF requires BTF" } else { "" }
            },
            {
                "pep_type": "windows_wfp",
                "status": if host.os == "windows" && host.windows_wfp { "available" } else if host.os == "windows" { "not_active" } else { "not_available" },
                "mode": if host.windows_wfp { "enforce" } else { "observe_only" },
                "maturity": "driver_probe_beta",
                "reason": if host.os != "windows" { "not running on windows" } else if !host.windows_wfp { "WFP driver not active" } else { "" }
            },
            {
                "pep_type": "macos_nefilter",
                "status": if host.os == "macos" && host.macos_nefilter { "available" } else if host.os == "macos" { "not_active" } else { "not_available" },
                "mode": if host.macos_nefilter { "enforce" } else { "observe_only" },
                "maturity": "system_extension_probe_beta",
                "reason": if host.os != "macos" { "not running on macOS" } else if !host.macos_nefilter { "NEFilter extension missing" } else { "" }
            },
            {
                "pep_type": "http_gateway",
                "status": "available",
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
            }
        ]
    })))
}

async fn check_capabilities(
    Path(_tenant): Path<String>,
    State(_state): State<AppState>,
    Json(req): Json<PepCapabilityCheckRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    // DEPRECATED: Please use /v1/host/capabilities and /v1/enforcement/auto-plan instead
    let host = crate::enforcement_plan_api::detect_host();

    let mut recommended = "".to_string();
    if req.requested_pep_types.contains(&"linux_ebpf".to_string())
        && host.os == "linux"
        && host.linux_ebpf
    {
        recommended = "linux_ebpf".to_string();
    } else if req.requested_pep_types.contains(&"windows_wfp".to_string())
        && host.os == "windows"
        && host.windows_wfp
    {
        recommended = "windows_wfp".to_string();
    } else if req
        .requested_pep_types
        .contains(&"macos_nefilter".to_string())
        && host.os == "macos"
        && host.macos_nefilter
    {
        recommended = "macos_nefilter".to_string();
    } else if req.requested_pep_types.contains(&"mcp_proxy".to_string()) {
        recommended = "mcp_proxy".to_string();
    } else if let Some(first) = req.requested_pep_types.first() {
        recommended = first.clone();
    }

    Ok(Json(serde_json::json!({
        "deprecated": true,
        "warning": "This endpoint is deprecated. Use GET /v1/host/capabilities and POST /v1/enforcement/auto-plan.",
        "recommended": recommended,
        "capabilities": [
            {
                "pep_type": "linux_ebpf",
                "status": if host.os == "linux" && host.linux_ebpf { "available" } else { "not_installed" },
                "mode": if host.linux_ebpf { "enforce" } else { "observe_only" },
            },
            {
                "pep_type": "windows_wfp",
                "status": if host.os == "windows" && host.windows_wfp { "available" } else { "not_available" },
                "mode": if host.windows_wfp { "enforce" } else { "observe_only" },
            },
            {
                "pep_type": "macos_nefilter",
                "status": if host.os == "macos" && host.macos_nefilter { "available" } else { "not_available" },
                "mode": if host.macos_nefilter { "enforce" } else { "observe_only" },
            },
            {
                "pep_type": "mcp_proxy",
                "status": "available",
                "mode": "enforce",
            }
        ]
    })))
}

async fn probe_pep(
    Path((_tenant, pep_id)): Path<(String, String)>,
    State(_state): State<AppState>,
    Json(_req): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "pep_id": pep_id,
        "status": "ready",
        "latency_ms": 12,
        "can_observe": true,
        "can_enforce": true,
    })))
}

async fn bind_pep(
    Path((tenant, _pep_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let binding_id = format!("bind_{}", uuid::Uuid::new_v4());
    let mut binding = req.clone();
    binding["id"] = serde_json::Value::String(binding_id.clone());
    binding["status"] = serde_json::Value::String("active".to_string());

    state
        .registry_store
        .upsert_raw(&tenant, "pep_binding", &binding_id, &binding)
        .await
        .map_err(crate::error::ApiError::Internal)?;

    Ok(Json(serde_json::json!({
        "binding_id": binding_id,
        "status": "active"
    })))
}
