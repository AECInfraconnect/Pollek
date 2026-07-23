use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
    routing::any,
    Json, Router,
};
use metrics_exporter_prometheus::PrometheusHandle;
use tower_http::services::{ServeDir, ServeFile};

use crate::{
    activity_api, agent_discovery_api, agent_inventory_api, auth, browser_extension_api, bundle,
    connectors, consent_api, contract_adapter, contract_api, correlation, deployment_api,
    detection_api, discovery, enforcement_plan_api, entity_graph, hotreload_api, inventory_api,
    local_observe, observation_api, observe_accuracy, pdp_cloud_api, pdp_routing_api,
    pdp_runtime_api, pep_capabilities_api, plugin_api, policy, policy_deploy_api, policy_first_api,
    policy_presets_api, policy_suggestions_api, preset_deploy_api, preset_deploy_wizard_api,
    prompt_guard_api, push, recommendation_api, registry, state::AppState, telemetry, usage_api,
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

async fn api_not_found() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "api route not found",
            "message": "The Local Control Plane API route is not available in this running backend. Restart local-control-plane after updating the repository, or verify that the dashboard is pointing at the Local Control Plane API port.",
        })),
    )
}

pub fn create_app(state: AppState, static_dir: &str, metrics_handle: PrometheusHandle) -> Router {
    let public_routes = Router::new()
        .route("/health", axum::routing::get(|| async { "ok" }))
        .route(
            "/metrics",
            axum::routing::get({
                let handle = metrics_handle.clone();
                move || async move { handle.render() }
            }),
        )
        .merge(discovery::router());

    let api_routes = Router::new()
        .merge(registry::router())
        .merge(agent_discovery_api::router())
        .merge(agent_inventory_api::router())
        .merge(policy_presets_api::router())
        .merge(preset_deploy_api::router())
        .merge(preset_deploy_wizard_api::router())
        .merge(activity_api::router())
        .merge(pep_capabilities_api::router())
        .merge(enforcement_plan_api::router())
        .merge(recommendation_api::router())
        .merge(policy_suggestions_api::router())
        .merge(observation_api::router())
        .merge(correlation::router())
        .merge(contract_api::router())
        .merge(contract_adapter::router())
        .merge(hotreload_api::router())
        .merge(usage_api::router())
        .merge(local_observe::router())
        .merge(observe_accuracy::router())
        .merge(entity_graph::router())
        .merge(inventory_api::router())
        .merge(policy::router())
        .merge(telemetry::router())
        .merge(bundle::router())
        .merge(connectors::router())
        .merge(pdp_runtime_api::router())
        .merge(pdp_routing_api::router())
        .merge(pdp_cloud_api::router())
        .merge(plugin_api::router())
        .merge(browser_extension_api::router())
        .merge(prompt_guard_api::router())
        .merge(policy_deploy_api::router())
        .merge(deployment_api::router())
        .merge(detection_api::router())
        .merge(consent_api::router())
        .merge(policy_first_api::router())
        .route(
            "/v1/tenants/:tenant/devices/:device/events",
            axum::routing::get(push::sse_handler),
        )
        .route("/v1/*path", any(api_not_found))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            local_tenant_guard,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::require_token,
        ));

    Router::new()
        .merge(public_routes)
        .merge(api_routes)
        .fallback_service(
            ServeDir::new(static_dir)
                .not_found_service(ServeFile::new(format!("{}/index.html", static_dir))),
        )
        .with_state(state)
}
