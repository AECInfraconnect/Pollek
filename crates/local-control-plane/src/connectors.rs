use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct ConnectorConfig {
    pub id: String,
    pub kind: ConnectorKind, // Opa | Cedar | OpenFga
    pub endpoint: String,
    pub health_interval_secs: u32,
    pub mtls_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorKind {
    Opa,
    Cedar,
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
    axum::extract::Path((_tenant, id)): axum::extract::Path<(String, String)>,
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
    axum::extract::Path((_tenant, id)): axum::extract::Path<(String, String)>,
    State(st): State<AppState>,
) -> Json<TestResult> {
    let cfg = match st.connector_store.get(&_tenant, &id).await {
        Ok(Some(c)) => match serde_json::from_value::<ConnectorConfig>(c) {
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
    let ok = match cfg.kind {
        ConnectorKind::Cedar => true,
        _ => reqwest::Client::new()
            .get(format!("{}/health", cfg.endpoint.trim_end_matches('/')))
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
}

async fn list(
    axum::extract::Path(tenant): axum::extract::Path<String>,
    State(st): State<AppState>,
) -> ApiResult<Json<Vec<ConnectorConfig>>> {
    let list = st
        .connector_store
        .list(&tenant)
        .await
        .map_err(ApiError::Internal)?;
    let mut configs = vec![];
    for val in list {
        if let Ok(c) = serde_json::from_value::<ConnectorConfig>(val) {
            configs.push(c);
        }
    }
    Ok(Json(configs))
}

async fn upsert(
    axum::extract::Path(tenant): axum::extract::Path<String>,
    State(st): State<AppState>,
    Json(payload): Json<ConnectorConfig>,
) -> ApiResult<Json<ConnectorConfig>> {
    let val = serde_json::to_value(&payload).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    st.connector_store
        .upsert(&tenant, &payload.id, &val)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(payload))
}
