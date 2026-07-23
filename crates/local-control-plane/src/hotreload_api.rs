//! Hot-reload activation for definition artifacts (agent signatures + agent
//! definitions) — the missing lane of the unified bundle.
//!
//! Policy already hot-reloads Cloud→local (`pdp_cloud_api`). This adds the
//! same live-activation capability for the *definition* artifact type: agent
//! signatures and web-AI / browser definitions. Activation goes through the
//! existing [`dek_fingerprint_defs::loader::DefinitionStore`] `ArcSwap`, so a
//! new definition takes effect immediately — discovery uses it on the next scan
//! with no restart. The previous definition is snapshotted before every apply,
//! giving a real one-step rollback.
//!
//! Compatibility is enforced the way definitions are actually versioned:
//! `apply_update` requires a matching `schema_version` and a strictly newer
//! `definition_version`, and reports a clear reason on rejection — never a
//! silent partial apply.

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dek_fingerprint_defs::model::{DefinitionKind, FingerprintDefinition};
use serde_json::json;

/// Registry object type holding the last-known-good definition snapshot.
const SNAPSHOT_TYPE: &str = "definition_last_known_good";
/// Registry object type holding the most recent activation event.
const ACTIVATION_TYPE: &str = "definition_activation";
const LATEST_ID: &str = "latest";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tenants/:tenant/definitions", get(get_state))
        .route("/v1/tenants/:tenant/definitions/activate", post(activate))
        .route("/v1/tenants/:tenant/definitions/rollback", post(rollback))
}

fn def_summary(def: &FingerprintDefinition) -> serde_json::Value {
    json!({
        "schema_version": def.schema_version,
        "definition_version": def.definition_version,
        "counts": {
            "signatures": def.signatures.len(),
            "web_ai_signatures": def.web_ai_signatures.len(),
            "browser_processes": def.browser_processes.len(),
            "installed_app_signatures": def.installed_app_signatures.len(),
        }
    })
}

async fn get_state(State(state): State<AppState>, Path(tenant): Path<String>) -> impl IntoResponse {
    let current = state.def_store.get();
    let last_activation = state
        .registry_store
        .get_raw(&tenant, ACTIVATION_TYPE, LATEST_ID)
        .await
        .ok()
        .flatten();
    let rollback_available = state
        .registry_store
        .get_raw(&tenant, SNAPSHOT_TYPE, LATEST_ID)
        .await
        .ok()
        .flatten()
        .is_some();

    (
        StatusCode::OK,
        Json(json!({
            "schema_version": "definition-state.v1",
            "tenant_id": tenant,
            "current": def_summary(&current),
            "last_activation": last_activation,
            "rollback_available": rollback_available,
        })),
    )
}

async fn activate(
    State(state): State<AppState>,
    Path(tenant): Path<String>,
    Json(incoming): Json<FingerprintDefinition>,
) -> impl IntoResponse {
    let previous = state.def_store.get();
    let from_version = previous.definition_version;
    let kind = incoming.kind.clone();

    // Snapshot the current (full) definition before applying, for rollback.
    if let Ok(snapshot) = serde_json::to_value(previous.as_ref()) {
        let _ = state
            .registry_store
            .upsert_raw(&tenant, SNAPSHOT_TYPE, LATEST_ID, &snapshot)
            .await;
    }

    match state.def_store.apply_update(incoming) {
        Ok(new_version) => {
            let event = json!({
                "event_id": format!("defact_{}", uuid::Uuid::new_v4()),
                "artifact_type": "definition",
                "operation": "activate",
                "kind": kind,
                "from_version": from_version,
                "to_version": new_version,
                "activated_at": chrono::Utc::now().to_rfc3339(),
            });
            let _ = state
                .registry_store
                .upsert_raw(&tenant, ACTIVATION_TYPE, LATEST_ID, &event)
                .await;
            let current = state.def_store.get();
            (
                StatusCode::OK,
                Json(json!({
                    "status": "activated",
                    "event": event,
                    "current": def_summary(&current),
                })),
            )
        }
        Err(e) => (
            StatusCode::CONFLICT,
            Json(json!({
                "status": "rejected",
                "reason": e.to_string(),
                "current": def_summary(&state.def_store.get()),
            })),
        ),
    }
}

async fn rollback(State(state): State<AppState>, Path(tenant): Path<String>) -> impl IntoResponse {
    let snapshot = state
        .registry_store
        .get_raw(&tenant, SNAPSHOT_TYPE, LATEST_ID)
        .await
        .ok()
        .flatten();
    let snapshot = match snapshot {
        Some(s) => s,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"status": "rejected", "reason": "no snapshot to roll back to"})),
            )
        }
    };
    let mut previous: FingerprintDefinition = match serde_json::from_value(snapshot) {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status": "rejected", "reason": format!("bad snapshot: {e}")})),
            )
        }
    };

    let from_version = state.def_store.get().definition_version;
    // Restore previous content as a full definition under a newer version so the
    // monotonic-version gate accepts it.
    previous.kind = DefinitionKind::Full;
    previous.definition_version = from_version + 1;

    match state.def_store.apply_update(previous) {
        Ok(new_version) => {
            let event = json!({
                "event_id": format!("defact_{}", uuid::Uuid::new_v4()),
                "artifact_type": "definition",
                "operation": "rollback",
                "from_version": from_version,
                "to_version": new_version,
                "activated_at": chrono::Utc::now().to_rfc3339(),
            });
            let _ = state
                .registry_store
                .upsert_raw(&tenant, ACTIVATION_TYPE, LATEST_ID, &event)
                .await;
            (
                StatusCode::OK,
                Json(json!({
                    "status": "rolled_back",
                    "event": event,
                    "current": def_summary(&state.def_store.get()),
                })),
            )
        }
        Err(e) => (
            StatusCode::CONFLICT,
            Json(json!({"status": "rejected", "reason": e.to_string()})),
        ),
    }
}
