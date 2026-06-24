import os
import re

# 1. Fix store.rs
store_path = "crates/local-control-plane/src/store.rs"
with open(store_path, "r", encoding="utf-8") as f:
    store_content = f.read()

store_content = store_content.replace('format!(""{}"",', 'format!("\\"{}\\"",')

store_content = store_content.replace("""            stmt.execute(params![
                session_clone.deployment_id,
                session_clone.policy_id,
                session_clone.policy_version,
                requested_control_level_str,
                target_scope_json,
                status_str,
                session_clone.created_by,
                session_clone.created_at.to_rfc3339(),
                session_clone.updated_at.to_rfc3339()
            ])?;
            
            tx.commit()?;""", """            stmt.execute(params![
                session_clone.deployment_id,
                session_clone.policy_id,
                session_clone.policy_version,
                requested_control_level_str,
                target_scope_json,
                status_str,
                session_clone.created_by,
                session_clone.created_at.to_rfc3339(),
                session_clone.updated_at.to_rfc3339()
            ])?;
            drop(stmt);
            
            tx.commit()?;""")

store_content = store_content.replace("""            stmt.execute(params![
                event.event_id,
                event.deployment_id,
                event.agent_id,
                event.entity_id,
                event.policy_id,
                phase_str,
                status_str,
                title_json,
                detail_json,
                tech_detail_json,
                user_action_json,
                event.created_at.to_rfc3339(),
                event.correlation_id
            ])?;
            
            tx.commit()?;""", """            stmt.execute(params![
                event.event_id,
                event.deployment_id,
                event.agent_id,
                event.entity_id,
                event.policy_id,
                phase_str,
                status_str,
                title_json,
                detail_json,
                tech_detail_json,
                user_action_json,
                event.created_at.to_rfc3339(),
                event.correlation_id
            ])?;
            drop(stmt);
            
            tx.commit()?;""")

with open(store_path, "w", encoding="utf-8") as f:
    f.write(store_content)


# 2. Fix deployment_api.rs
api2_path = "crates/local-control-plane/src/deployment_api.rs"
with open(api2_path, "r", encoding="utf-8") as f:
    api2_content = f.read()

api2_content = api2_content.replace(
    "let orchestrator = DeploymentOrchestrator::new(sink.clone());",
    "let orchestrator = DeploymentOrchestrator::new(sink.clone(), st.deployment_store.clone());"
)

# Wait, `StoreEventSink::new()` also needs to be updated in deployment_api.rs
api2_content = api2_content.replace(
    "let sink = Arc::new(StoreEventSink::new());",
    "let sink = Arc::new(StoreEventSink::new(st.deployment_store.clone()));"
)

# And `create_session` in `deployment_api.rs` might not have `st: State<AppState>`?
# Let's check if it does. I'll just use regex.
api2_content = re.sub(
    r"async fn create_deployment\([^)]+?\)\s*->\s*ApiResult<\(StatusCode,\s*Json<DeploymentSession>\)>\s*\{",
    r"async fn create_deployment(State(st): State<AppState>, Json(req): Json<CreateDeploymentRequest>) -> ApiResult<(StatusCode, Json<DeploymentSession>)> {",
    api2_content
) # We'll do it broadly if needed. Actually it's probably better to just patch exactly what's failing.

with open(api2_path, "w", encoding="utf-8") as f:
    f.write(api2_content)

# 3. Fix policy_first_api.rs
api1_path = "crates/local-control-plane/src/policy_first_api.rs"
with open(api1_path, "r", encoding="utf-8") as f:
    api1_content = f.read()

api1_content = api1_content.replace(
    "let sink = std::sync::Arc::new(StoreEventSink::new());",
    "let sink = std::sync::Arc::new(StoreEventSink::new(st.deployment_store.clone()));"
)
api1_content = api1_content.replace(
    "let sink = Arc::new(StoreEventSink::new());",
    "let sink = Arc::new(StoreEventSink::new(st.deployment_store.clone()));"
)

api1_content = api1_content.replace(
    "let orchestrator = DeploymentOrchestrator::new(sink);",
    "let orchestrator = DeploymentOrchestrator::new(sink, st.deployment_store.clone());"
)

# Fix State(_st) to State(st) in create_deployment_session if not replaced
api1_content = api1_content.replace(
    "async fn create_deployment_session(\n    State(_st): State<AppState>,\n)",
    "async fn create_deployment_session(\n    State(st): State<AppState>,\n)"
)

# Fix unused action_id
api1_content = api1_content.replace(
    "Path((session_id, action_id)): Path<(String, String)>",
    "Path((session_id, _action_id)): Path<(String, String)>"
)

with open(api1_path, "w", encoding="utf-8") as f:
    f.write(api1_content)
