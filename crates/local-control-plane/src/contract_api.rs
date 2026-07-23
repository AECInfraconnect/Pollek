//! Contract Hub API — the DEK/LCP side of version negotiation with Pollek Cloud.
//!
//! `GET  /v1/tenants/:tenant/contract`          → this DEK's self-reported contract.
//! `POST /v1/tenants/:tenant/contract/evaluate` → verdict for a given bundle /
//!                                                 compatibility block.
//!
//! The DEK never activates a bundle it cannot run. Cloud (and this endpoint)
//! evaluate the DEK's real, runtime-derived [`DekContract`] against a bundle's
//! `compatibility` using the shared, pure [`dek_bundle_format::evaluate_compatibility`],
//! so a fleet of version-skewed DEKs each gets an explicit, reasoned verdict.

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dek_bundle_format::{
    evaluate_compatibility, BundleCompatibility, DekContract, OsModulesConfig, PollekPolicyBundle,
};
use dek_capability_registry::snapshot::CapabilityStatus;
use serde_json::json;

/// Contract generation this DEK speaks. Single source shared with the
/// `.well-known/pollek-contract` discovery document.
pub const CONTRACT_VERSION: &str = "2026.06.29";
/// Floor DEK version this contract generation still supports.
pub const MIN_SUPPORTED_DEK_VERSION: &str = "1.0.0-beta.6";
/// Bundle-envelope api versions this DEK understands.
pub const SUPPORTED_BUNDLE_API_VERSIONS: &[&str] = &["v1"];

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tenants/:tenant/contract", get(get_contract))
        .route(
            "/v1/tenants/:tenant/contract/evaluate",
            post(evaluate_contract),
        )
}

fn current_platform() -> String {
    match std::env::consts::OS {
        "linux" => "linux",
        "windows" => "windows",
        "macos" => "macos",
        other => other,
    }
    .to_string()
}

/// User-space PEPs this LCP binary always hosts (compiled-in routers), used when
/// no live capability snapshot has been taken yet.
fn baseline_pep_types() -> Vec<String> {
    vec![
        "mcp_proxy".to_string(),
        "http_proxy".to_string(),
        "browser_extension".to_string(),
        "secure_spool_observer".to_string(),
    ]
}

fn capability_present(status: &CapabilityStatus) -> bool {
    matches!(
        status,
        CapabilityStatus::Ready
            | CapabilityStatus::ReadyAfterApproval
            | CapabilityStatus::InstalledInactive
    )
}

/// Build the DEK's contract from real runtime sources: the compiled product
/// version, and the live capability snapshot (when present) for available PEP
/// types and OS enforcement modules.
pub async fn build_dek_contract(state: &AppState) -> DekContract {
    let platform = current_platform();
    let snapshot = state.latest_snapshot.read().await;

    let mut available_pep_types: Vec<String> = Vec::new();
    let mut os_modules = OsModulesConfig::default();

    if let Some(snap) = snapshot.as_ref() {
        for m in &snap.methods {
            if !capability_present(&m.status) {
                continue;
            }
            let pep = serde_json::to_value(&m.internal_pep)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            if pep.is_empty() || pep == "none" {
                continue;
            }
            if !available_pep_types.contains(&pep) {
                available_pep_types.push(pep.clone());
            }
            // OS-module PEPs contribute a concrete module id for the platform.
            match pep.as_str() {
                "linux_ebpf" => push_unique(&mut os_modules.linux, "ebpfd.v1"),
                "windows_wfp" => push_unique(&mut os_modules.windows, "wfp.v1"),
                "macos_network_extension" => push_unique(&mut os_modules.macos, "nefilter.v1"),
                _ => {}
            }
        }
    }

    if available_pep_types.is_empty() {
        available_pep_types = baseline_pep_types();
    } else {
        for base in baseline_pep_types() {
            if !available_pep_types.contains(&base) {
                available_pep_types.push(base);
            }
        }
    }

    DekContract {
        dek_version: dek_bundle_format::dek_version().to_string(),
        contract_version: CONTRACT_VERSION.to_string(),
        supported_bundle_api_versions: SUPPORTED_BUNDLE_API_VERSIONS
            .iter()
            .map(|s| s.to_string())
            .collect(),
        available_pep_types,
        os_modules,
        platform,
    }
}

fn push_unique(v: &mut Vec<String>, item: &str) {
    if !v.iter().any(|x| x == item) {
        v.push(item.to_string());
    }
}

async fn get_contract(
    State(state): State<AppState>,
    Path(_tenant): Path<String>,
) -> impl IntoResponse {
    let contract = build_dek_contract(&state).await;
    (
        StatusCode::OK,
        Json(json!({
            "schema_version": "dek-contract.v1",
            "contract": contract,
        })),
    )
}

/// Accepts either a full [`PollekPolicyBundle`] or a bare [`BundleCompatibility`]
/// and returns the compatibility verdict against this DEK.
async fn evaluate_contract(
    State(state): State<AppState>,
    Path(_tenant): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let compat = extract_compatibility(&body);
    let compat = match compat {
        Some(c) => c,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "body must be a policy bundle or a compatibility block",
                })),
            )
        }
    };

    let contract = build_dek_contract(&state).await;
    let verdict = evaluate_compatibility(&contract, &compat);

    (
        StatusCode::OK,
        Json(json!({
            "schema_version": "contract-evaluation.v1",
            "contract": contract,
            "compatibility": compat,
            "verdict": verdict,
        })),
    )
}

/// Pull a `BundleCompatibility` from a full bundle, a `{ "compatibility": … }`
/// wrapper, or a bare compatibility object.
fn extract_compatibility(body: &serde_json::Value) -> Option<BundleCompatibility> {
    if let Ok(bundle) = serde_json::from_value::<PollekPolicyBundle>(body.clone()) {
        return Some(bundle.compatibility);
    }
    if let Some(inner) = body.get("compatibility") {
        if let Ok(c) = serde_json::from_value::<BundleCompatibility>(inner.clone()) {
            return Some(c);
        }
    }
    serde_json::from_value::<BundleCompatibility>(body.clone()).ok()
}
