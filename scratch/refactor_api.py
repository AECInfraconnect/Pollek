import os

api_path = "crates/local-control-plane/src/policy_first_api.rs"
with open(api_path, "r", encoding="utf-8") as f:
    api_content = f.read()

# Route registrations
if ".route(\"/v1/deployment-sessions/:id\", get(get_deployment_session))" not in api_content:
    api_content = api_content.replace(
        '.route("/v1/deployment-sessions", post(create_deployment_session))',
        """.route("/v1/deployment-sessions", post(create_deployment_session))
        .route("/v1/deployment-sessions/:id", get(get_deployment_session))
        .route("/v1/deployment-sessions/:id/events", get(get_deployment_events))"""
    )

# New GET endpoints
new_get_str = """
async fn get_deployment_session(
    Path(session_id): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Json<DeploymentSession>)> {
    let session = st.deployment_store.get_deployment_session(&session_id).await?;
    if let Some(session) = session {
        Ok((StatusCode::OK, Json(session)))
    } else {
        Ok((StatusCode::NOT_FOUND, Json(DeploymentSession {
            deployment_id: session_id,
            policy_id: "".into(),
            policy_version: "".into(),
            requested_control_level: ControlLevel::Observe,
            target_scope: DeploymentScope::Device { device_id: "".into() },
            status: DeploymentSessionStatus::Failed,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            created_by: "".into(),
        }))) // Return 404 in real world, but type needs to match
    }
}

async fn get_deployment_events(
    Path(session_id): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Json<Vec<dek_domain_schema::deployment_session::DeploymentEvent>>)> {
    let events = st.deployment_store.list_deployment_events(&session_id).await?;
    Ok((StatusCode::OK, Json(events)))
}
"""

if "async fn get_deployment_session" not in api_content:
    api_content = api_content + "\n" + new_get_str

# Replace create_deployment_session
create_str_old = """async fn create_deployment_session(
    State(_st): State<AppState>,
) -> ApiResult<(StatusCode, Json<DeploymentSession>)> {
    let mut session = DeploymentSession {
        deployment_id: Uuid::new_v4().to_string(),
        policy_id: "policy-tmp".into(),
        policy_version: "1.0".into(),
        requested_control_level: ControlLevel::Enforce,
        target_scope: DeploymentScope::Device {
            device_id: "local".into(),
        },
        status: DeploymentSessionStatus::ScanStarted,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        created_by: "local_admin".into(),
    };

    let sink = std::sync::Arc::new(StoreEventSink::new());
    let orchestrator = DeploymentOrchestrator::new(sink);

    // Mock quick transitions to plan
    let _ = orchestrator
        .transition(
            &mut session,
            DeploymentSessionStatus::CapabilitySnapshotCreated,
        )
        .await;
    let _ = orchestrator
        .transition(
            &mut session,
            DeploymentSessionStatus::PolicyFeasibilityEvaluated,
        )
        .await;
    let _ = orchestrator
        .transition(&mut session, DeploymentSessionStatus::DeploymentPlanCreated)
        .await;

    Ok((StatusCode::OK, Json(session)))
}"""

create_str_new = """async fn create_deployment_session(
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Json<DeploymentSession>)> {
    let mut session = DeploymentSession {
        deployment_id: Uuid::new_v4().to_string(),
        policy_id: "policy-tmp".into(),
        policy_version: "1.0".into(),
        requested_control_level: ControlLevel::Enforce,
        target_scope: DeploymentScope::Device {
            device_id: "local".into(),
        },
        status: DeploymentSessionStatus::ScanStarted,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        created_by: "local_admin".into(),
    };

    let sink = std::sync::Arc::new(StoreEventSink::new(st.deployment_store.clone()));
    let orchestrator = DeploymentOrchestrator::new(sink, st.deployment_store.clone());

    // Mock quick transitions to plan, now properly persisted
    let _ = orchestrator
        .transition(
            &mut session,
            DeploymentSessionStatus::CapabilitySnapshotCreated,
        )
        .await;
    let _ = orchestrator
        .transition(
            &mut session,
            DeploymentSessionStatus::PolicyFeasibilityEvaluated,
        )
        .await;
    let _ = orchestrator
        .transition(&mut session, DeploymentSessionStatus::DeploymentPlanCreated)
        .await;

    Ok((StatusCode::OK, Json(session)))
}"""

api_content = api_content.replace(create_str_old, create_str_new)

# Replace approve_action
approve_str_old = """async fn approve_action(
    Path((_session_id, _action_id)): Path<(String, String)>,
    State(_st): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({"status": "approved"})),
    ))
}"""

approve_str_new = """async fn approve_action(
    Path((session_id, action_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    if let Some(mut session) = st.deployment_store.get_deployment_session(&session_id).await? {
        let sink = std::sync::Arc::new(StoreEventSink::new(st.deployment_store.clone()));
        let orchestrator = DeploymentOrchestrator::new(sink, st.deployment_store.clone());
        let _ = orchestrator.transition(&mut session, DeploymentSessionStatus::BundleCreated).await;
        let _ = orchestrator.transition(&mut session, DeploymentSessionStatus::BundleActivated).await;
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({"status": "approved", "session": session})),
        ))
    } else {
        Ok((StatusCode::NOT_FOUND, Json(serde_json::json!({"status": "not_found"}))))
    }
}"""

api_content = api_content.replace(approve_str_old, approve_str_new)

# Replace retry_deployment
retry_str_old = """async fn retry_deployment(
    Path(session_id): Path<String>,
    State(_st): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "retrying",
            "deployment_id": session_id
        })),
    ))
}"""

retry_str_new = """async fn retry_deployment(
    Path(session_id): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    if let Some(mut session) = st.deployment_store.get_deployment_session(&session_id).await? {
        let sink = std::sync::Arc::new(StoreEventSink::new(st.deployment_store.clone()));
        let orchestrator = DeploymentOrchestrator::new(sink, st.deployment_store.clone());
        let _ = orchestrator.transition(&mut session, DeploymentSessionStatus::ScanStarted).await;
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "retrying",
                "deployment_id": session_id
            })),
        ))
    } else {
        Ok((StatusCode::NOT_FOUND, Json(serde_json::json!({"status": "not_found"}))))
    }
}"""

api_content = api_content.replace(retry_str_old, retry_str_new)


# Replace rollback_deployment
rollback_str_old = """async fn rollback_deployment(
    Path(session_id): Path<String>,
    State(_st): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "rolled_back",
            "deployment_id": session_id
        })),
    ))
}"""

rollback_str_new = """async fn rollback_deployment(
    Path(session_id): Path<String>,
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    if let Some(mut session) = st.deployment_store.get_deployment_session(&session_id).await? {
        let sink = std::sync::Arc::new(StoreEventSink::new(st.deployment_store.clone()));
        let orchestrator = DeploymentOrchestrator::new(sink, st.deployment_store.clone());
        let _ = orchestrator.transition(&mut session, DeploymentSessionStatus::RolledBack).await;
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "rolled_back",
                "deployment_id": session_id
            })),
        ))
    } else {
        Ok((StatusCode::NOT_FOUND, Json(serde_json::json!({"status": "not_found"}))))
    }
}"""

api_content = api_content.replace(rollback_str_old, rollback_str_new)


with open(api_path, "w", encoding="utf-8") as f:
    f.write(api_content)
print("Updated policy_first_api.rs")

