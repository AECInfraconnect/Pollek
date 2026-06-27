// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use axum::{extract::State, routing::get, Json, Router};
use serde_json::Value;

use crate::{error::ApiResult, state::AppState};
use dek_recommend::Recommender;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/recommendations", get(get_recommendations))
}

async fn get_recommendations(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    // Determine local device capabilities
    let caps =
        dek_capability_registry::CapabilityRegistry::new("local".into(), "1.0".into()).gather();

    let events = state
        .observability_store
        .list_observation_events("local")
        .await
        .map_err(crate::error::ApiError::Internal)?;
    let items = dek_agent_observer::activity::activity_items_from_observations(&events);
    let recent_stats = dek_agent_observer::activity::activity_counts(&items);

    let recs = Recommender::recommend(&caps, &recent_stats);

    Ok(Json(serde_json::json!({
        "status": "success",
        "recommendations": recs,
    })))
}
