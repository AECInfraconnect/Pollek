use crate::error::{ApiError, ApiResult};
use crate::pdp_models::PdpRouteRule;
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/pdp/routes",
            get(list_routes).post(upsert_route),
        )
        .route(
            "/v1/tenants/:tenant/pdp/routes/:id",
            get(get_route).delete(delete_route),
        )
        .route(
            "/v1/tenants/:tenant/pdp/routes/simulate",
            axum::routing::post(simulate_route),
        )
        .route(
            "/v1/tenants/:tenant/pdp/routes/execute",
            axum::routing::post(execute_route),
        )
}

#[derive(serde::Deserialize)]
pub struct RouteEvalRequest {
    pub agent_id: Option<String>,
    pub resource_id: Option<String>,
    pub protocol: Option<String>,
    pub payload: Option<serde_json::Value>,
}

async fn simulate_route(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(req): Json<RouteEvalRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let svc = crate::pdp_router::PdpRouterService::new(st);
    let res = svc
        .simulate_route(
            &tenant,
            req.agent_id.as_deref(),
            req.resource_id.as_deref(),
            req.protocol.as_deref(),
        )
        .await?;
    Ok(Json(res))
}

async fn execute_route(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(req): Json<RouteEvalRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let svc = crate::pdp_router::PdpRouterService::new(st);
    let payload = req.payload.unwrap_or_else(|| serde_json::json!({}));
    let res = svc
        .execute_route(
            &tenant,
            req.agent_id.as_deref(),
            req.resource_id.as_deref(),
            req.protocol.as_deref(),
            &payload,
        )
        .await?;
    Ok(Json(res))
}

async fn list_routes(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<Json<Vec<PdpRouteRule>>> {
    let list = st
        .pdp_store
        .list_routes(&tenant)
        .await
        .map_err(ApiError::Internal)?;
    let mut routes = vec![];
    for val in list {
        if let Ok(c) = serde_json::from_value::<PdpRouteRule>(val) {
            routes.push(c);
        }
    }
    // Sort by priority descending (higher number means higher priority)
    routes.sort_by_key(|b| std::cmp::Reverse(b.priority));
    Ok(Json(routes))
}

async fn get_route(
    Path((tenant, id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<PdpRouteRule>> {
    let opt = st
        .pdp_store
        .get_route(&tenant, &id)
        .await
        .map_err(ApiError::Internal)?;
    match opt {
        Some(val) => {
            let rt: PdpRouteRule =
                serde_json::from_value(val).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
            Ok(Json(rt))
        }
        None => Err(ApiError::NotFound("pdp route not found".to_string())),
    }
}

async fn upsert_route(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(payload): Json<PdpRouteRule>,
) -> ApiResult<Json<PdpRouteRule>> {
    let val = serde_json::to_value(&payload).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    st.pdp_store
        .upsert_route(&tenant, &payload.id, &val)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(payload))
}

async fn delete_route(
    Path((tenant, id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let deleted = st
        .pdp_store
        .delete_route(&tenant, &id)
        .await
        .map_err(ApiError::Internal)?;
    if deleted {
        Ok(Json(serde_json::json!({ "status": "deleted" })))
    } else {
        Err(ApiError::NotFound("pdp route not found".to_string()))
    }
}
