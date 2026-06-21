use crate::{error::{ApiError, ApiResult}, state::AppState};
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use dek_control_plane_api::registry::*;
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new()
        // Agents
        .route(
            "/v1/tenants/:tenant_id/registry/agents",
            get(list_agents).post(create_agent),
        )
        .route(
            "/v1/tenants/:tenant_id/registry/agents/:agent_id",
            get(get_agent).patch(patch_agent).delete(delete_agent),
        )
        // Blackbox AI
        .route(
            "/v1/tenants/:tenant_id/registry/blackbox-ai",
            get(list_blackbox_ai).post(create_blackbox_ai),
        )
        .route(
            "/v1/tenants/:tenant_id/registry/blackbox-ai/:provider_id",
            get(get_blackbox_ai)
                .patch(patch_blackbox_ai)
                .delete(delete_blackbox_ai),
        )
        // MCP Servers
        .route(
            "/v1/tenants/:tenant_id/registry/mcp-servers",
            get(list_mcp_servers).post(create_mcp_server),
        )
        .route(
            "/v1/tenants/:tenant_id/registry/mcp-servers/:server_id",
            get(get_mcp_server)
                .patch(patch_mcp_server)
                .delete(delete_mcp_server),
        )
        // Tools
        .route(
            "/v1/tenants/:tenant_id/registry/tools",
            get(list_tools).post(create_tool),
        )
        .route(
            "/v1/tenants/:tenant_id/registry/tools/:tool_id",
            get(get_tool).patch(patch_tool).delete(delete_tool),
        )
        // Resources
        .route(
            "/v1/tenants/:tenant_id/registry/resources",
            get(list_resources).post(create_resource),
        )
        .route(
            "/v1/tenants/:tenant_id/registry/resources/:resource_id",
            get(get_resource)
                .patch(patch_resource)
                .delete(delete_resource),
        )
        // Entities
        .route(
            "/v1/tenants/:tenant_id/registry/entities",
            get(list_entities).post(create_entity),
        )
        .route(
            "/v1/tenants/:tenant_id/registry/entities/:entity_id",
            get(get_entity).patch(patch_entity).delete(delete_entity),
        )
        // Relationships
        .route(
            "/v1/tenants/:tenant_id/registry/relationships",
            get(list_relationships).post(create_relationship),
        )
        .route(
            "/v1/tenants/:tenant_id/registry/relationships/:relationship_id",
            get(get_relationship)
                .patch(patch_relationship)
                .delete(delete_relationship),
        )
}

// -----------------------------------------------------------------------------
// Agents
// -----------------------------------------------------------------------------
async fn list_agents(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let items = state.registry_store.list_agents(&tenant_id).await.map_err(ApiError::Internal)?;
    Ok(Json(json!(items)))
}

async fn create_agent(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    Json(mut payload): Json<AiAgent>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    payload.meta.tenant_id = tenant_id;
    let item = state.registry_store.upsert_agent(payload).await.map_err(ApiError::Internal)?;
    Ok((axum::http::StatusCode::CREATED, Json(json!(item))))
}

async fn get_agent(
    Path((tenant_id, agent_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let item = state.registry_store.get_agent(&tenant_id, &agent_id).await.map_err(ApiError::Internal)?;
    match item {
        Some(i) => Ok(Json(json!(i))),
        None => Err(ApiError::NotFound(agent_id)),
    }
}

async fn patch_agent(
    Path((tenant_id, _agent_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(mut payload): Json<AiAgent>,
) -> ApiResult<Json<serde_json::Value>> {
    payload.meta.tenant_id = tenant_id;
    let item = state.registry_store.upsert_agent(payload).await.map_err(ApiError::Internal)?;
    Ok(Json(json!(item)))
}

async fn delete_agent(
    Path((tenant_id, agent_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    let deleted = state.registry_store.delete_agent(&tenant_id, &agent_id).await.map_err(ApiError::Internal)?;
    if deleted {
        Ok((axum::http::StatusCode::NO_CONTENT, Json(json!({}))))
    } else {
        Err(ApiError::NotFound(agent_id))
    }
}

// -----------------------------------------------------------------------------
// Blackbox AI
// -----------------------------------------------------------------------------
async fn list_blackbox_ai(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let items = state.registry_store.list_blackbox_ai(&tenant_id).await.map_err(ApiError::Internal)?;
    Ok(Json(json!(items)))
}

async fn create_blackbox_ai(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    Json(mut payload): Json<BlackboxAiProvider>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    payload.meta.tenant_id = tenant_id;
    let item = state.registry_store.upsert_blackbox_ai(payload).await.map_err(ApiError::Internal)?;
    Ok((axum::http::StatusCode::CREATED, Json(json!(item))))
}

async fn get_blackbox_ai(
    Path((tenant_id, provider_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let item = state.registry_store.get_blackbox_ai(&tenant_id, &provider_id).await.map_err(ApiError::Internal)?;
    match item {
        Some(i) => Ok(Json(json!(i))),
        None => Err(ApiError::NotFound(provider_id)),
    }
}

async fn patch_blackbox_ai(
    Path((tenant_id, _provider_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(mut payload): Json<BlackboxAiProvider>,
) -> ApiResult<Json<serde_json::Value>> {
    payload.meta.tenant_id = tenant_id;
    let item = state.registry_store.upsert_blackbox_ai(payload).await.map_err(ApiError::Internal)?;
    Ok(Json(json!(item)))
}

async fn delete_blackbox_ai(
    Path((tenant_id, provider_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    let deleted = state.registry_store.delete_blackbox_ai(&tenant_id, &provider_id).await.map_err(ApiError::Internal)?;
    if deleted {
        Ok((axum::http::StatusCode::NO_CONTENT, Json(json!({}))))
    } else {
        Err(ApiError::NotFound(provider_id))
    }
}

// -----------------------------------------------------------------------------
// MCP Servers
// -----------------------------------------------------------------------------
async fn list_mcp_servers(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let items = state.registry_store.list_mcp_servers(&tenant_id).await.map_err(ApiError::Internal)?;
    Ok(Json(json!(items)))
}

async fn create_mcp_server(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    Json(mut payload): Json<McpServer>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    payload.meta.tenant_id = tenant_id;
    let item = state.registry_store.upsert_mcp_server(payload).await.map_err(ApiError::Internal)?;
    Ok((axum::http::StatusCode::CREATED, Json(json!(item))))
}

async fn get_mcp_server(
    Path((tenant_id, server_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let item = state.registry_store.get_mcp_server(&tenant_id, &server_id).await.map_err(ApiError::Internal)?;
    match item {
        Some(i) => Ok(Json(json!(i))),
        None => Err(ApiError::NotFound(server_id)),
    }
}

async fn patch_mcp_server(
    Path((tenant_id, _server_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(mut payload): Json<McpServer>,
) -> ApiResult<Json<serde_json::Value>> {
    payload.meta.tenant_id = tenant_id;
    let item = state.registry_store.upsert_mcp_server(payload).await.map_err(ApiError::Internal)?;
    Ok(Json(json!(item)))
}

async fn delete_mcp_server(
    Path((tenant_id, server_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    let deleted = state.registry_store.delete_mcp_server(&tenant_id, &server_id).await.map_err(ApiError::Internal)?;
    if deleted {
        Ok((axum::http::StatusCode::NO_CONTENT, Json(json!({}))))
    } else {
        Err(ApiError::NotFound(server_id))
    }
}

// -----------------------------------------------------------------------------
// Tools
// -----------------------------------------------------------------------------
async fn list_tools(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let items = state.registry_store.list_tools(&tenant_id).await.map_err(ApiError::Internal)?;
    Ok(Json(json!(items)))
}

async fn create_tool(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    Json(mut payload): Json<Tool>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    payload.meta.tenant_id = tenant_id;
    let item = state.registry_store.upsert_tool(payload).await.map_err(ApiError::Internal)?;
    Ok((axum::http::StatusCode::CREATED, Json(json!(item))))
}

async fn get_tool(
    Path((tenant_id, tool_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let item = state.registry_store.get_tool(&tenant_id, &tool_id).await.map_err(ApiError::Internal)?;
    match item {
        Some(i) => Ok(Json(json!(i))),
        None => Err(ApiError::NotFound(tool_id)),
    }
}

async fn patch_tool(
    Path((tenant_id, _tool_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(mut payload): Json<Tool>,
) -> ApiResult<Json<serde_json::Value>> {
    payload.meta.tenant_id = tenant_id;
    let item = state.registry_store.upsert_tool(payload).await.map_err(ApiError::Internal)?;
    Ok(Json(json!(item)))
}

async fn delete_tool(
    Path((tenant_id, tool_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    let deleted = state.registry_store.delete_tool(&tenant_id, &tool_id).await.map_err(ApiError::Internal)?;
    if deleted {
        Ok((axum::http::StatusCode::NO_CONTENT, Json(json!({}))))
    } else {
        Err(ApiError::NotFound(tool_id))
    }
}

// -----------------------------------------------------------------------------
// Resources
// -----------------------------------------------------------------------------
async fn list_resources(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let items = state.registry_store.list_resources(&tenant_id).await.map_err(ApiError::Internal)?;
    Ok(Json(json!(items)))
}

async fn create_resource(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    Json(mut payload): Json<Resource>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    payload.meta.tenant_id = tenant_id;
    let item = state.registry_store.upsert_resource(payload).await.map_err(ApiError::Internal)?;
    Ok((axum::http::StatusCode::CREATED, Json(json!(item))))
}

async fn get_resource(
    Path((tenant_id, resource_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let item = state.registry_store.get_resource(&tenant_id, &resource_id).await.map_err(ApiError::Internal)?;
    match item {
        Some(i) => Ok(Json(json!(i))),
        None => Err(ApiError::NotFound(resource_id)),
    }
}

async fn patch_resource(
    Path((tenant_id, _resource_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(mut payload): Json<Resource>,
) -> ApiResult<Json<serde_json::Value>> {
    payload.meta.tenant_id = tenant_id;
    let item = state.registry_store.upsert_resource(payload).await.map_err(ApiError::Internal)?;
    Ok(Json(json!(item)))
}

async fn delete_resource(
    Path((tenant_id, resource_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    let deleted = state.registry_store.delete_resource(&tenant_id, &resource_id).await.map_err(ApiError::Internal)?;
    if deleted {
        Ok((axum::http::StatusCode::NO_CONTENT, Json(json!({}))))
    } else {
        Err(ApiError::NotFound(resource_id))
    }
}

// -----------------------------------------------------------------------------
// Entities
// -----------------------------------------------------------------------------
async fn list_entities(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let items = state.registry_store.list_entities(&tenant_id).await.map_err(ApiError::Internal)?;
    Ok(Json(json!(items)))
}

async fn create_entity(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    Json(mut payload): Json<Entity>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    payload.meta.tenant_id = tenant_id;
    let item = state.registry_store.upsert_entity(payload).await.map_err(ApiError::Internal)?;
    Ok((axum::http::StatusCode::CREATED, Json(json!(item))))
}

async fn get_entity(
    Path((tenant_id, entity_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let item = state.registry_store.get_entity(&tenant_id, &entity_id).await.map_err(ApiError::Internal)?;
    match item {
        Some(i) => Ok(Json(json!(i))),
        None => Err(ApiError::NotFound(entity_id)),
    }
}

async fn patch_entity(
    Path((tenant_id, _entity_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(mut payload): Json<Entity>,
) -> ApiResult<Json<serde_json::Value>> {
    payload.meta.tenant_id = tenant_id;
    let item = state.registry_store.upsert_entity(payload).await.map_err(ApiError::Internal)?;
    Ok(Json(json!(item)))
}

async fn delete_entity(
    Path((tenant_id, entity_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    let deleted = state.registry_store.delete_entity(&tenant_id, &entity_id).await.map_err(ApiError::Internal)?;
    if deleted {
        Ok((axum::http::StatusCode::NO_CONTENT, Json(json!({}))))
    } else {
        Err(ApiError::NotFound(entity_id))
    }
}

// -----------------------------------------------------------------------------
// Relationships
// -----------------------------------------------------------------------------
async fn list_relationships(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let items = state.registry_store.list_relationships(&tenant_id).await.map_err(ApiError::Internal)?;
    Ok(Json(json!(items)))
}

async fn create_relationship(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    Json(mut payload): Json<Relationship>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    payload.meta.tenant_id = tenant_id;
    let item = state.registry_store.upsert_relationship(payload).await.map_err(ApiError::Internal)?;
    Ok((axum::http::StatusCode::CREATED, Json(json!(item))))
}

async fn get_relationship(
    Path((tenant_id, relationship_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let item = state.registry_store.get_relationship(&tenant_id, &relationship_id).await.map_err(ApiError::Internal)?;
    match item {
        Some(i) => Ok(Json(json!(i))),
        None => Err(ApiError::NotFound(relationship_id)),
    }
}

async fn patch_relationship(
    Path((tenant_id, _relationship_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(mut payload): Json<Relationship>,
) -> ApiResult<Json<serde_json::Value>> {
    payload.meta.tenant_id = tenant_id;
    let item = state.registry_store.upsert_relationship(payload).await.map_err(ApiError::Internal)?;
    Ok(Json(json!(item)))
}

async fn delete_relationship(
    Path((tenant_id, relationship_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(axum::http::StatusCode, Json<serde_json::Value>)> {
    let deleted = state.registry_store.delete_relationship(&tenant_id, &relationship_id).await.map_err(ApiError::Internal)?;
    if deleted {
        Ok((axum::http::StatusCode::NO_CONTENT, Json(json!({}))))
    } else {
        Err(ApiError::NotFound(relationship_id))
    }
}
