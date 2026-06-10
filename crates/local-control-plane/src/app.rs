use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
    Json, Router,
};
use tower_http::services::{ServeDir, ServeFile};

use crate::{
    auth, bundle, policy, push, registry, state::AppState, telemetry,
};

pub async fn local_tenant_guard(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    if state.identity.tenant_id == "local" {
        let path = req.uri().path();
        if path.starts_with("/v1/tenants/") {
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() > 3 && parts[3] != "local" {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(
                        serde_json::json!({"error": "Local Admin Dashboard only supports tenant_id=local"}),
                    ),
                ));
            }
        }
    }
    Ok(next.run(req).await)
}

pub fn create_app(state: AppState, static_dir: &str) -> Router {
    let api_routes = Router::new()
        .merge(registry::router())
        .merge(policy::router())
        .merge(telemetry::router())
        .merge(bundle::router())
        .route("/v1/push", axum::routing::get(push::sse_handler))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            local_tenant_guard,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_token,
        ));

    Router::new()
        .route("/health", axum::routing::get(|| async { "ok" }))
        .merge(api_routes)
        .fallback_service(
            ServeDir::new(static_dir)
                .not_found_service(ServeFile::new(format!("{}/index.html", static_dir))),
        )
        .with_state(state)
}
