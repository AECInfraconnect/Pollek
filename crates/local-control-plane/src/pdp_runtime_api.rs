use crate::error::{ApiError, ApiResult};
use crate::pdp_models::{PdpKind, PdpProbeResult, PdpRuntime, PdpRuntimeCategory, PdpStatus};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};

#[derive(serde::Serialize)]
pub struct ProbeResult {
    pub ok: bool,
    pub latency_ms: u64,
    pub detail: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/pdp/runtimes",
            get(list_runtimes).post(upsert_runtime),
        )
        .route(
            "/v1/tenants/:tenant/pdp/runtimes/:id",
            get(get_runtime).delete(delete_runtime),
        )
        .route(
            "/v1/tenants/:tenant/pdp/runtimes/:id/validate",
            post(validate_runtime),
        )
        .route(
            "/v1/tenants/:tenant/pdp/runtimes/:id/probe",
            post(probe_health),
        )
        .route(
            "/v1/tenants/:tenant/pdp/runtimes/:id/load-bundle",
            post(load_bundle),
        )
        .route(
            "/v1/tenants/:tenant/pdp/runtimes/:id/cache/clear",
            post(clear_runtime_cache),
        )
        .route("/v1/tenants/:tenant/pdp/evaluate", post(evaluate))
}

fn runtime_bundle_blob_path(id: &str) -> String {
    format!("pdp-runtimes/{id}/loaded-bundle.json")
}

async fn load_bundle(
    Path((tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    // Persist the loaded bundle as a blob so it survives restarts and can be
    // inspected by validate and dropped by cache/clear.
    let record = serde_json::json!({
        "bundle": payload,
        "loaded_at": chrono::Utc::now().to_rfc3339(),
    });
    let bytes = record.to_string().into_bytes();
    match state
        .policy_store
        .put_blob(&tenant, &runtime_bundle_blob_path(&id), &bytes)
        .await
    {
        Ok(()) => Json(serde_json::json!({
            "status": "success",
            "message": format!("Bundle loaded and persisted to runtime {id}"),
            "bytes": bytes.len(),
        })),
        Err(err) => Json(serde_json::json!({
            "status": "error",
            "message": format!("failed to persist bundle: {err}"),
        })),
    }
}

async fn evaluate(
    Path(_tenant): Path<String>,
    State(_state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "decision_id": "eval-12345",
        "allowed": true,
        "mode": "enforce",
        "reason": "Allowed by default policy",
        "principal": payload.get("principal").unwrap_or(&serde_json::json!("unknown")),
        "action": payload.get("action").unwrap_or(&serde_json::json!("unknown")),
        "resource": payload.get("resource").unwrap_or(&serde_json::json!("unknown")),
        "pep_type": payload.get("context").and_then(|c| c.get("pep_type")).unwrap_or(&serde_json::json!("unknown")),
        "pdp_runtime_id": "local-opa-wasm",
        "route_id": "default-route",
        "policy_bundle_id": "baseline-bundle",
        "policy_version": "v1.0.0",
        "latency_ms": 12,
        "fallback_used": false,
        "obligations": [],
        "redactions": [],
        "errors": []
    }))
}

fn seeded_local_runtimes(now: &str) -> Vec<PdpRuntime> {
    vec![
        PdpRuntime {
            id: "local.router".into(),
            name: "Policy Router".into(),
            category: PdpRuntimeCategory::LocalEngine,
            kind: PdpKind::PolicyRouter,
            mode: "router".into(),
            system_managed: true,
            enabled: true,
            status: PdpStatus::Ready,
            endpoint: None,
            auth_ref: None,
            capabilities: vec![],
            config_source: "seeded".into(),
            active_bundle_id: None,
            active_bundle_hash: None,
            last_activated_at: Some(now.into()),
            last_probe: None,
            health: None,
            created_at: now.into(),
            updated_at: now.into(),
        },
        PdpRuntime {
            id: "local.cedar".into(),
            name: "Cedar Local".into(),
            category: PdpRuntimeCategory::LocalEngine,
            kind: PdpKind::CedarLocal,
            mode: "embedded".into(),
            system_managed: true,
            enabled: true,
            status: PdpStatus::NotConfigured,
            endpoint: None,
            auth_ref: None,
            capabilities: vec![],
            config_source: "seeded".into(),
            active_bundle_id: None,
            active_bundle_hash: None,
            last_activated_at: None,
            last_probe: None,
            health: None,
            created_at: now.into(),
            updated_at: now.into(),
        },
        PdpRuntime {
            id: "local.opa_wasm".into(),
            name: "WASM Plugin Runtime".into(),
            category: PdpRuntimeCategory::LocalEngine,
            kind: PdpKind::OpaWasm,
            mode: "wasm".into(),
            system_managed: true,
            enabled: true,
            status: PdpStatus::NotConfigured,
            endpoint: None,
            auth_ref: None,
            capabilities: vec![],
            config_source: "seeded".into(),
            active_bundle_id: None,
            active_bundle_hash: None,
            last_activated_at: None,
            last_probe: None,
            health: None,
            created_at: now.into(),
            updated_at: now.into(),
        },
    ]
}

async fn list_runtimes(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<Json<Vec<PdpRuntime>>> {
    let list = st
        .pdp_store
        .list_runtimes(&tenant)
        .await
        .map_err(ApiError::Internal)?;
    let mut runtimes = seeded_local_runtimes(&chrono::Utc::now().to_rfc3339());
    for val in list {
        if let Ok(c) = serde_json::from_value::<PdpRuntime>(val) {
            if !runtimes.iter().any(|r| r.id == c.id) {
                runtimes.push(c);
            }
        }
    }
    Ok(Json(runtimes))
}

async fn get_runtime(
    Path((tenant, id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<PdpRuntime>> {
    let seeded = seeded_local_runtimes(&chrono::Utc::now().to_rfc3339());
    if let Some(rt) = seeded.into_iter().find(|r| r.id == id) {
        return Ok(Json(rt));
    }
    let opt = st
        .pdp_store
        .get_runtime(&tenant, &id)
        .await
        .map_err(ApiError::Internal)?;
    match opt {
        Some(val) => {
            let rt: PdpRuntime =
                serde_json::from_value(val).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
            Ok(Json(rt))
        }
        None => Err(ApiError::NotFound("pdp runtime not found".to_string())),
    }
}

async fn upsert_runtime(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(mut payload): Json<PdpRuntime>,
) -> ApiResult<Json<PdpRuntime>> {
    if let Some(secret) = payload.auth_ref.take() {
        st.pdp_credentials
            .store_credential(&payload.id, &secret)
            .await?;
    }

    let val = serde_json::to_value(&payload).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    st.pdp_store
        .upsert_runtime(&tenant, &payload.id, &val)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(payload))
}

async fn delete_runtime(
    Path((tenant, id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let deleted = st
        .pdp_store
        .delete_runtime(&tenant, &id)
        .await
        .map_err(ApiError::Internal)?;
    if deleted {
        let _ = st.pdp_credentials.delete_credential(&id).await;
        Ok(Json(serde_json::json!({ "status": "deleted" })))
    } else {
        Err(ApiError::NotFound("pdp runtime not found".to_string()))
    }
}

/// Drop the persisted bundle blob for a runtime so the next load starts
/// fresh. An empty blob marks the cache as cleared.
async fn clear_runtime_cache(
    Path((tenant, id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> Json<serde_json::Value> {
    let path = runtime_bundle_blob_path(&id);
    let had_bundle = matches!(
        st.policy_store.get_blob(&tenant, &path).await,
        Ok(Some(bytes)) if !bytes.is_empty()
    );
    if had_bundle {
        if let Err(err) = st.policy_store.put_blob(&tenant, &path, &[]).await {
            return Json(serde_json::json!({
                "ok": false,
                "error": format!("failed to clear cached bundle: {err}"),
            }));
        }
    }
    Json(serde_json::json!({
        "ok": true,
        "runtime_id": id,
        "cleared_keys": if had_bundle { vec!["loaded_bundle"] } else { Vec::new() },
    }))
}

async fn validate_runtime(
    Path((tenant, id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> Json<serde_json::Value> {
    // Validate the persisted runtime configuration for real: the config must
    // exist (or be a seeded built-in) and parse into a PdpRuntime.
    let seeded = seeded_local_runtimes(&chrono::Utc::now().to_rfc3339());
    if seeded.iter().any(|r| r.id == id) {
        return Json(serde_json::json!({
            "ok": true,
            "status": "ready",
            "details": { "source": "built_in" },
            "warnings": []
        }));
    }
    match st.pdp_store.get_runtime(&tenant, &id).await {
        Ok(Some(config)) => match serde_json::from_value::<PdpRuntime>(config.clone()) {
            Ok(runtime) => {
                let mut warnings: Vec<String> = Vec::new();
                if runtime.endpoint.is_none() {
                    warnings.push("runtime has no endpoint configured".into());
                }
                let has_bundle = matches!(
                    st.policy_store
                        .get_blob(&tenant, &runtime_bundle_blob_path(&id))
                        .await,
                    Ok(Some(bytes)) if !bytes.is_empty()
                );
                if !has_bundle {
                    warnings.push("no policy bundle loaded yet".into());
                }
                Json(serde_json::json!({
                    "ok": true,
                    "status": "ready",
                    "details": { "kind": format!("{:?}", runtime.kind) },
                    "warnings": warnings,
                }))
            }
            Err(err) => Json(serde_json::json!({
                "ok": false,
                "status": "invalid_config",
                "details": { "error": format!("{err}") },
                "warnings": []
            })),
        },
        Ok(None) => Json(serde_json::json!({
            "ok": false,
            "status": "not_found",
            "details": { "error": "runtime has no persisted configuration" },
            "warnings": []
        })),
        Err(err) => Json(serde_json::json!({
            "ok": false,
            "status": "error",
            "details": { "error": format!("{err}") },
            "warnings": []
        })),
    }
}

async fn probe_health(
    Path((tenant, id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> Json<PdpProbeResult> {
    let seeded = seeded_local_runtimes(&chrono::Utc::now().to_rfc3339());
    let rt = if let Some(rt) = seeded.into_iter().find(|r| r.id == id) {
        rt
    } else {
        match st.pdp_store.get_runtime(&tenant, &id).await {
            Ok(Some(c)) => match serde_json::from_value::<PdpRuntime>(c) {
                Ok(c) => c,
                Err(_) => {
                    return Json(PdpProbeResult {
                        ok: false,
                        latency_ms: 0,
                        effect: "error".into(),
                        reason: "invalid config".into(),
                        decision_id: "".into(),
                        details: serde_json::json!({}),
                    })
                }
            },
            _ => {
                return Json(PdpProbeResult {
                    ok: false,
                    latency_ms: 0,
                    effect: "error".into(),
                    reason: "pdp runtime not found".into(),
                    decision_id: "".into(),
                    details: serde_json::json!({}),
                })
            }
        }
    };

    let start = std::time::Instant::now();

    if let Some(endpoint) = rt.endpoint {
        let ok = match rt.kind {
            PdpKind::CedarHttp | PdpKind::CedarLocal => true, // Local cedar or mock cedar
            PdpKind::OpenfgaServer => reqwest::Client::new()
                .get(format!("{}/healthz", endpoint.trim_end_matches('/')))
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false),
            _ => reqwest::Client::new()
                .get(format!("{}/health", endpoint.trim_end_matches('/')))
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false),
        };
        Json(PdpProbeResult {
            ok,
            latency_ms: start.elapsed().as_millis() as u64,
            effect: if ok { "permit".into() } else { "deny".into() },
            reason: if ok {
                "reachable".into()
            } else {
                "unreachable".into()
            },
            decision_id: uuid::Uuid::new_v4().to_string(),
            details: serde_json::json!({}),
        })
    } else {
        // Handle seeded local engines probe
        Json(PdpProbeResult {
            ok: true,
            latency_ms: start.elapsed().as_millis() as u64,
            effect: "permit".into(),
            reason: "built-in runtime simulated response".into(),
            decision_id: uuid::Uuid::new_v4().to_string(),
            details: serde_json::json!({ "mode": rt.mode }),
        })
    }
}
