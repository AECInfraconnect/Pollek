use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

mod rules;
mod sensors;

use rules::{evaluate_events, get_coverage, list_rules};
use sensors::{list_sensors, preflight_sensor, record_sensor_consent, request_sensor_install};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tenants/:tenant/detections/coverage", get(get_coverage))
        .route("/v1/tenants/:tenant/detections/rules", get(list_rules))
        .route(
            "/v1/tenants/:tenant/detections/evaluate",
            post(evaluate_events),
        )
        .route("/v1/tenants/:tenant/detections/sensors", get(list_sensors))
        .route(
            "/v1/tenants/:tenant/detections/sensors/:sensor_id/preflight",
            post(preflight_sensor),
        )
        .route(
            "/v1/tenants/:tenant/detections/sensors/:sensor_id/consent",
            post(record_sensor_consent),
        )
        .route(
            "/v1/tenants/:tenant/detections/sensors/:sensor_id/install",
            post(request_sensor_install),
        )
}
