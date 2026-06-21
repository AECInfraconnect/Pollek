use crate::error::{ApiError, ApiResult};
use crate::pdp_models::{PdpKind, PdpRuntime};
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
            "/v1/tenants/:tenant/pdp/runtimes/:id/health",
            post(probe_health),
        )
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
    let mut runtimes = vec![];
    for val in list {
        if let Ok(c) = serde_json::from_value::<PdpRuntime>(val) {
            runtimes.push(c);
        }
    }
    Ok(Json(runtimes))
}

async fn get_runtime(
    Path((tenant, id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<PdpRuntime>> {
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

async fn probe_health(
    Path((tenant, id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> Json<ProbeResult> {
    let rt = match st.pdp_store.get_runtime(&tenant, &id).await {
        Ok(Some(c)) => match serde_json::from_value::<PdpRuntime>(c) {
            Ok(c) => c,
            Err(_) => {
                return Json(ProbeResult {
                    ok: false,
                    latency_ms: 0,
                    detail: "invalid config".into(),
                })
            }
        },
        _ => {
            return Json(ProbeResult {
                ok: false,
                latency_ms: 0,
                detail: "pdp runtime not found".into(),
            })
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
        Json(ProbeResult {
            ok,
            latency_ms: start.elapsed().as_millis() as u64,
            detail: if ok {
                "reachable".into()
            } else {
                "unreachable".into()
            },
        })
    } else {
        Json(ProbeResult {
            ok: true,
            latency_ms: 0,
            detail: "built-in runtime".into(),
        })
    }
}
