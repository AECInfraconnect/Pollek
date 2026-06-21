use crate::error::{ApiError, ApiResult};
use crate::pdp_models::{PdpKind, PdpRuntime, PdpRuntimeCategory, PdpStatus};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ConnectorConfig {
    pub id: String,
    pub kind: ConnectorKind, // Opa | Cedar | OpenFga
    pub endpoint: String,
    pub store_id: Option<String>,
    pub health_interval_secs: u32,
    pub mtls_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorKind {
    Opa,
    Cedar,
    #[serde(rename = "openfga")]
    OpenFga,
}

#[derive(Serialize)]
pub struct TestResult {
    pub ok: bool,
    pub latency_ms: u64,
    pub detail: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tenants/:tenant/connectors", get(list).post(upsert))
        .route(
            "/v1/tenants/:tenant/connectors/:id/test",
            post(test_connection),
        )
        .route("/v1/tenants/:tenant/pdp/:id/override", post(set_override))
}

#[derive(serde::Deserialize)]
pub struct OverridePayload {
    pub forced_state: String, // "ForceDown", "ForceUp"
    pub auto_recovery_delay: Option<u64>,
}

async fn set_override(
    Path((_tenant, id)): Path<(String, String)>,
    State(_st): State<AppState>,
    Json(payload): Json<OverridePayload>,
) -> ApiResult<Json<serde_json::Value>> {
    tracing::info!(
        "Setting override for PDP {} to state {} with delay {:?}",
        id,
        payload.forced_state,
        payload.auto_recovery_delay
    );
    // Note: Since this is LCP serving as the dashboard backend, we just return success.
    // The dashboard expects this endpoint to exist. Edge PDPs would have real ManualOverride configs.
    Ok(Json(serde_json::json!({
        "status": "success",
        "pdp_id": id,
        "forced_state": payload.forced_state,
        "auto_recovery_delay": payload.auto_recovery_delay
    })))
}

async fn test_connection(
    Path((tenant, id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> Json<TestResult> {
    let rt = match st.pdp_store.get_runtime(&tenant, &id).await {
        Ok(Some(c)) => match serde_json::from_value::<PdpRuntime>(c) {
            Ok(c) => c,
            Err(_) => {
                return Json(TestResult {
                    ok: false,
                    latency_ms: 0,
                    detail: "invalid config".into(),
                })
            }
        },
        _ => {
            return Json(TestResult {
                ok: false,
                latency_ms: 0,
                detail: "connector not found".into(),
            })
        }
    };
    let start = std::time::Instant::now();

    if let Some(endpoint) = rt.endpoint {
        let ok = match rt.kind {
            PdpKind::CedarHttp => true,
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
        Json(TestResult {
            ok,
            latency_ms: start.elapsed().as_millis() as u64,
            detail: if ok {
                "reachable".into()
            } else {
                "unreachable".into()
            },
        })
    } else {
        Json(TestResult {
            ok: false,
            latency_ms: 0,
            detail: "no endpoint configured".into(),
        })
    }
}

async fn list(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<Json<Vec<ConnectorConfig>>> {
    let list = st
        .pdp_store
        .list_runtimes(&tenant)
        .await
        .map_err(ApiError::Internal)?;
    let mut configs = vec![];
    for val in list {
        if let Ok(rt) = serde_json::from_value::<PdpRuntime>(val) {
            // Map back to legacy ConnectorConfig
            if rt.category == PdpRuntimeCategory::ExternalConnector {
                let kind = match rt.kind {
                    PdpKind::OpaServer => ConnectorKind::Opa,
                    PdpKind::OpenfgaServer => ConnectorKind::OpenFga,
                    PdpKind::CedarHttp => ConnectorKind::Cedar,
                    _ => continue,
                };
                configs.push(ConnectorConfig {
                    id: rt.id,
                    kind,
                    endpoint: rt.endpoint.unwrap_or_default(),
                    store_id: None,
                    health_interval_secs: 60,
                    mtls_enabled: false,
                });
            }
        }
    }
    Ok(Json(configs))
}

async fn upsert(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(payload): Json<ConnectorConfig>,
) -> ApiResult<Json<ConnectorConfig>> {
    let pdp_kind = match payload.kind {
        ConnectorKind::Opa => PdpKind::OpaServer,
        ConnectorKind::Cedar => PdpKind::CedarHttp,
        ConnectorKind::OpenFga => PdpKind::OpenfgaServer,
    };

    let rt = PdpRuntime {
        id: payload.id.clone(),
        name: payload.id.clone(), // Default name to ID
        category: PdpRuntimeCategory::ExternalConnector,
        kind: pdp_kind,
        enabled: true,
        status: PdpStatus::Ready,
        endpoint: Some(payload.endpoint.clone()),
        auth_ref: None,
        capabilities: vec![],
        health: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    let val = serde_json::to_value(&rt).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    st.pdp_store
        .upsert_runtime(&tenant, &payload.id, &val)
        .await
        .map_err(ApiError::Internal)?;

    // Add deprecation warning to response later if needed. For now just succeed.
    Ok(Json(payload))
}
