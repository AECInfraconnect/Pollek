// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use std::convert::Infallible;

use crate::{error::ApiResult, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/activity", get(get_activity))
        .route("/v1/activity/stream", get(stream_activity))
        .route("/v1/tenants/:tenant/activity", get(get_activity_tenant))
}

async fn get_activity(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    get_activity_for_tenant(&state, "local").await
}

async fn get_activity_tenant(
    Path(tenant): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<Value>> {
    get_activity_for_tenant(&state, &tenant).await
}

async fn get_activity_for_tenant(state: &AppState, tenant: &str) -> ApiResult<Json<Value>> {
    let events = state
        .observability_store
        .list_observation_events(tenant)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    let items = dek_agent_observer::activity::activity_items_from_observations(&events);
    let counts = dek_agent_observer::activity::activity_counts(&items);
    let sets = dek_agent_observer::activity::group_into_sets(items, 300);

    Ok(Json(json!({
        "status": "success",
        "source": "observation_events",
        "counts": counts,
        "activity_sets": sets,
    })))
}

async fn stream_activity(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.telemetry_tx.subscribe();
    let stream = async_stream::stream! {
        loop {
            let envelope = match rx.recv().await {
                Ok(envelope) => envelope,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            };
            let data = serde_json::to_string(&envelope).unwrap_or_else(|_| "{}".to_string());
            yield Ok(Event::default().data(data));
        }
    };

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new())
}
