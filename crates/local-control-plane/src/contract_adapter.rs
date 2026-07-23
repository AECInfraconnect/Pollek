//! WASM contract-adapter — the version-skew bridge (roadmap 2C).
//!
//! A Cloud-authored bundle may be shaped for a different contract generation
//! than this DEK runs. Rather than ship a new binary, the DEK runs a
//! hot-reloadable **WASM component** that migrates the bundle into the shape it
//! expects. The adapter runs in the real [`dek_wasm_host::WasmPluginHost`]
//! (fuel-bounded, pooled), and the migrated bundle is then re-checked against
//! the Contract Hub so the UI can show it becoming activatable.
//!
//! The adapter module is shipped with the DEK (embedded, integrity-hashed) and
//! loaded once on first use.

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dek_bundle_format::{evaluate_compatibility, BundleCompatibility};
use dek_wasm_host::{
    config::WasmHostConfig,
    host::WasmPluginHost,
    plugin_key::{sha256_hex, PluginKey},
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::OnceCell;

/// The adapter WASM, built from `examples/plugins/contract-adapter` and shipped
/// with the DEK.
const ADAPTER_WASM: &[u8] = include_bytes!("../assets/contract-adapter.wasm");
const ADAPTER_ID: &str = "contract-adapter";
const ADAPTER_VERSION: &str = "0.1.0";
/// CPU-instruction budget for one adaptation (bounded; a bundle migration is
/// cheap).
const ADAPTER_FUEL: u64 = 200_000_000;

struct AdapterRuntime {
    host: WasmPluginHost,
    pool_key: String,
    sha256: String,
}

static RUNTIME: OnceCell<Arc<AdapterRuntime>> = OnceCell::const_new();

async fn runtime() -> anyhow::Result<Arc<AdapterRuntime>> {
    RUNTIME
        .get_or_try_init(|| async {
            let host = WasmPluginHost::new(WasmHostConfig::default())?;
            let sha = sha256_hex(ADAPTER_WASM);
            let key = PluginKey {
                tenant_id: "local".into(),
                plugin_id: ADAPTER_ID.into(),
                version: ADAPTER_VERSION.into(),
                wasm_sha256: sha.clone(),
                abi_version: "1".into(),
            };
            host.load_plugin(key, ADAPTER_WASM).await?;
            let pool_key = format!("local:{ADAPTER_ID}:{ADAPTER_VERSION}:{sha}");
            Ok::<_, anyhow::Error>(Arc::new(AdapterRuntime {
                host,
                pool_key,
                sha256: sha,
            }))
        })
        .await
        .cloned()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tenants/:tenant/contract/adapter", get(adapter_info))
        .route("/v1/tenants/:tenant/contract/adapt", post(adapt_bundle))
}

async fn adapter_info(Path(_tenant): Path<String>) -> impl IntoResponse {
    match runtime().await {
        Ok(rt) => (
            StatusCode::OK,
            Json(json!({
                "schema_version": "contract-adapter-info.v1",
                "plugin_id": ADAPTER_ID,
                "version": ADAPTER_VERSION,
                "wasm_sha256": rt.sha256,
                "wasm_bytes": ADAPTER_WASM.len(),
                "runtime": "wasmtime (fuel-bounded, pooled)",
                "loaded": true,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"loaded": false, "error": e.to_string()})),
        ),
    }
}

#[derive(serde::Deserialize)]
struct AdaptRequest {
    bundle: Value,
    #[serde(default)]
    to_contract: Option<String>,
}

async fn adapt_bundle(
    State(_state): State<AppState>,
    Path(_tenant): Path<String>,
    Json(req): Json<AdaptRequest>,
) -> impl IntoResponse {
    let rt = match runtime().await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("adapter unavailable: {e}")})),
            )
        }
    };

    let to_contract = req
        .to_contract
        .unwrap_or_else(|| crate::contract_api::CONTRACT_VERSION.to_string());

    // Verdict BEFORE adaptation (if the bundle even carries a compatibility).
    let before = verdict_for(&_state, &req.bundle).await;

    let input = match serde_json::to_vec(&json!({
        "bundle": req.bundle,
        "to_contract": to_contract,
    })) {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": e.to_string()})),
            )
        }
    };

    let request_id = format!("adapt_{}", uuid::Uuid::new_v4());
    let out = match rt
        .host
        .invoke(&rt.pool_key, request_id, &input, ADAPTER_FUEL)
        .await
    {
        Ok(o) => o,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("adapter invoke failed: {e}")})),
            )
        }
    };

    let result: Value = match serde_json::from_slice(&out) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("adapter returned invalid json: {e}")})),
            )
        }
    };

    // Verdict AFTER adaptation on the migrated bundle.
    let after = match result.get("bundle") {
        Some(b) => verdict_for(&_state, b).await,
        None => None,
    };

    (
        StatusCode::OK,
        Json(json!({
            "schema_version": "contract-adaptation.v1",
            "adapter": { "plugin_id": ADAPTER_ID, "version": ADAPTER_VERSION, "wasm_sha256": rt.sha256 },
            "to_contract": to_contract,
            "adapted": result.get("adapted").cloned().unwrap_or(json!(false)),
            "changes": result.get("changes").cloned().unwrap_or(json!([])),
            "migrated_bundle": result.get("bundle").cloned().unwrap_or(json!(null)),
            "verdict_before": before,
            "verdict_after": after,
        })),
    )
}

/// Evaluate a bundle's `compatibility` against this DEK, if present.
async fn verdict_for(state: &AppState, bundle: &Value) -> Option<Value> {
    let compat = extract_compat(bundle)?;
    let contract = crate::contract_api::build_dek_contract(state).await;
    serde_json::to_value(evaluate_compatibility(&contract, &compat)).ok()
}

fn extract_compat(bundle: &Value) -> Option<BundleCompatibility> {
    let compat = bundle.get("compatibility")?;
    serde_json::from_value::<BundleCompatibility>(compat.clone()).ok()
}
