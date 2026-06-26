// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::state::AppState;
use axum::{
    routing::{get, post},
    Json, Router,
};
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/target/resource", get(target_resource))
        .route("/v1/target/action", post(target_action))
        .route("/v1/target/restricted", get(target_restricted))
        .route("/v1/mcp/call", post(mcp_call))
        .route("/v1/ext_authz", post(ext_authz_check))
}

async fn target_resource() -> axum::response::Result<Json<serde_json::Value>, axum::http::StatusCode>
{
    Ok(Json(
        json!({"status": "ok", "message": "Simulated backend resource reached successfully"}),
    ))
}

async fn target_action() -> axum::response::Result<Json<serde_json::Value>, axum::http::StatusCode>
{
    Ok(Json(
        json!({"status": "ok", "message": "Simulated backend action reached successfully"}),
    ))
}

async fn target_restricted(
) -> axum::response::Result<Json<serde_json::Value>, axum::http::StatusCode> {
    Ok(Json(
        json!({"status": "restricted", "message": "This is a sensitive resource that should be blocked by DEK!"}),
    ))
}

async fn mcp_call() -> axum::response::Result<Json<serde_json::Value>, axum::http::StatusCode> {
    Ok(Json(json!({
        "status": "success",
        "result": "Mock MCP tool executed successfully"
    })))
}

async fn ext_authz_check() -> axum::response::Result<Json<serde_json::Value>, axum::http::StatusCode>
{
    // Basic Envoy ext_authz HTTP stub response
    // 200 OK means authorized
    Ok(Json(json!({
        "status": {
            "code": 0 // OK
        },
        "dynamic_metadata": {
            "fields": {
                "pollek.authz": {
                    "kind": "Struct",
                    "fields": {
                        "decision": { "kind": "StringValue", "string_value": "Allow" }
                    }
                }
            }
        }
    })))
}
