//! Policy-first deployment sessions: create a plan from a scope/candidate,
//! present it for approval, then apply (bind + activate) or roll back, with
//! a timeline of deployment events.

use super::*;

#[allow(dead_code)]
#[derive(Deserialize)]
pub(super) struct CreateDeployRequest {
    /// Full discovery candidate (rich flow, e.g. AutoDiscovery detail).
    candidate: Option<dek_agent_discovery::model::DiscoveredAgentCandidateV2>,
    /// Lightweight policy reference (simple wizard sends the picked suggestion here).
    policy: Option<serde_json::Value>,
    /// Agent ids picked in the simple wizard (used for scope when no candidate).
    #[serde(default)]
    agents: Vec<String>,
    requested_level: ControlLevel,
    policy_id: Option<String>,
}

#[derive(Serialize)]
pub(super) struct DeploySessionResponse {
    id: String,
    feasibility: PolicyFeasibilityResult,
    status: DeploymentSessionStatus,
}

pub(super) fn agent_id_from_scope(scope: &DeploymentScope) -> Option<String> {
    match scope {
        DeploymentScope::Agent { agent_id } => Some(agent_id.clone()),
        _ => None,
    }
}

pub(super) fn deployment_event(
    session: &DeploymentSession,
    phase: DeploymentPhase,
    status: EventStatus,
    title: LocalizedText,
    detail: LocalizedText,
    technical_detail: Option<serde_json::Value>,
    user_action: Option<UserAction>,
) -> DeploymentEvent {
    DeploymentEvent {
        event_id: format!("evt_{}", uuid::Uuid::new_v4()),
        deployment_id: session.deployment_id.clone(),
        agent_id: agent_id_from_scope(&session.target_scope),
        entity_id: None,
        policy_id: session.policy_id.clone(),
        phase,
        status,
        title,
        detail,
        technical_detail,
        user_action,
        created_at: chrono::Utc::now(),
        correlation_id: session.deployment_id.clone(),
    }
}

pub(super) async fn create_deploy_session(
    Path(tenant): Path<String>,
    State(state): State<AppState>,
    Json(req): Json<CreateDeployRequest>,
) -> ApiResult<(StatusCode, Json<DeploySessionResponse>)> {
    let device_id = local_device_id();
    let snap_v2 = build_capability_snapshot_v2(&tenant, &device_id, RuntimeMode::DesktopSimple);
    let snap = legacy_snapshot_from_v2(&snap_v2);
    let policy_id = req
        .policy_id
        .clone()
        .or_else(|| policy_id_from_value(req.policy.as_ref()))
        .unwrap_or_else(|| "pii.redact_before_external_llm".into());
    let mut feasibility = if let Some(candidate) = &req.candidate {
        dek_enforcement_api::feasibility::assess(candidate, req.requested_level.clone(), &snap)
    } else {
        let pol = dek_enforcement_api::planner::Policy {
            id: policy_id.clone(),
            requested_level: req.requested_level.clone(),
        };
        dek_enforcement_api::planner::assess_feasibility(&pol, &snap)
    };
    feasibility.policy_id = policy_id.clone();
    let now = chrono::Utc::now();
    let deployment_id = format!("deploy_{}", uuid::Uuid::new_v4());
    let session = DeploymentSession {
        deployment_id: deployment_id.clone(),
        policy_id: policy_id.clone(),
        policy_version: "draft".into(),
        requested_control_level: match req.requested_level {
            ControlLevel::Observe => dek_domain_schema::control_level::ControlLevel::Observe,
            ControlLevel::Warn => dek_domain_schema::control_level::ControlLevel::Warn,
            ControlLevel::Ask => dek_domain_schema::control_level::ControlLevel::Approval,
            ControlLevel::Enforce => dek_domain_schema::control_level::ControlLevel::Enforce,
        },
        target_scope: DeploymentScope::Agent {
            agent_id: req
                .candidate
                .as_ref()
                .map(|c| c.suggested_registration.agent_id.clone())
                .or_else(|| req.agents.first().cloned())
                .unwrap_or_else(|| "local_host".into()),
        },
        status: DeploymentSessionStatus::PolicyFeasibilityEvaluated,
        created_at: now,
        updated_at: now,
        created_by: "local-admin".into(),
    };
    state
        .deployment_store
        .upsert_deployment_session(session.clone())
        .await
        .map_err(ApiError::Internal)?;
    let event = deployment_event(
        &session,
        DeploymentPhase::RoutePlanning,
        EventStatus::Info,
        LocalizedText {
            en: "Protection preview created".into(),
            th: "สร้างตัวอย่างการป้องกันแล้ว".into(),
        },
        LocalizedText {
            en: feasibility.friendly_en.clone(),
            th: feasibility.friendly_th.clone(),
        },
        Some(serde_json::to_value(&feasibility).map_err(|e| ApiError::Internal(e.into()))?),
        None,
    );
    state
        .deployment_store
        .insert_deployment_event(event)
        .await
        .map_err(ApiError::Internal)?;
    Ok((
        StatusCode::CREATED,
        Json(DeploySessionResponse {
            id: deployment_id,
            feasibility,
            status: DeploymentSessionStatus::PolicyFeasibilityEvaluated,
        }),
    ))
}

pub(super) async fn get_deploy_session(
    Path((_tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(StatusCode, Json<DeploymentSession>)> {
    let session = state
        .deployment_store
        .get_deployment_session(&id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(id.clone()))?;
    Ok((StatusCode::OK, Json(session)))
}

pub(super) async fn get_deploy_timeline(
    Path((_tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(StatusCode, Json<Vec<DeploymentEvent>>)> {
    let events = state
        .deployment_store
        .list_deployment_events(&id)
        .await
        .map_err(ApiError::Internal)?;
    Ok((StatusCode::OK, Json(events)))
}

pub(super) async fn confirm_deploy_session(
    Path((tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(
    StatusCode,
    Json<dek_enforcement_api::planner::ControlMethodPlan>,
)> {
    let session = state
        .deployment_store
        .get_deployment_session(&id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(id.clone()))?;
    let snap_v2 =
        build_capability_snapshot_v2(&tenant, &local_device_id(), RuntimeMode::DesktopSimple);
    let snap = legacy_snapshot_from_v2(&snap_v2);
    let pol = Policy {
        id: session.policy_id.clone(),
        requested_level: match &session.requested_control_level {
            dek_domain_schema::control_level::ControlLevel::Observe => ControlLevel::Observe,
            dek_domain_schema::control_level::ControlLevel::Warn => ControlLevel::Warn,
            dek_domain_schema::control_level::ControlLevel::Approval => ControlLevel::Ask,
            _ => ControlLevel::Enforce,
        },
    };
    let res = dek_enforcement_api::planner::assess_feasibility(&pol, &snap);
    let plan = negotiate(&res);
    let event = deployment_event(
        &session,
        DeploymentPhase::RoutePlanning,
        EventStatus::Success,
        LocalizedText {
            en: "Control method plan selected".into(),
            th: "เลือกแผนวิธีควบคุมแล้ว".into(),
        },
        LocalizedText {
            en: "Pollek selected the best available local control methods for this device.".into(),
            th: "Pollek เลือกวิธีควบคุมบนเครื่องที่พร้อมที่สุดสำหรับอุปกรณ์นี้".into(),
        },
        Some(serde_json::to_value(&plan).map_err(|e| ApiError::Internal(e.into()))?),
        None,
    );
    state
        .deployment_store
        .insert_deployment_event(event)
        .await
        .map_err(ApiError::Internal)?;
    Ok((StatusCode::OK, Json(plan)))
}

/// Approve a single pending setup action on a deployment session, then
/// re-run planning so the plan reflects the approval.
pub(super) async fn approve_deploy_session_action(
    Path((tenant, id, action_id)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(
    StatusCode,
    Json<dek_enforcement_api::planner::ControlMethodPlan>,
)> {
    let session = state
        .deployment_store
        .get_deployment_session(&id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(id.clone()))?;
    let event = deployment_event(
        &session,
        DeploymentPhase::CapabilityCheck,
        EventStatus::Success,
        LocalizedText {
            en: format!("Setup action '{action_id}' approved"),
            th: format!("อนุมัติขั้นตอนตั้งค่า '{action_id}' แล้ว"),
        },
        LocalizedText {
            en: "The user approved this setup action from the dashboard.".into(),
            th: "ผู้ใช้อนุมัติขั้นตอนตั้งค่านี้จากแดชบอร์ด".into(),
        },
        Some(serde_json::json!({ "action_id": action_id })),
        None,
    );
    state
        .deployment_store
        .insert_deployment_event(event)
        .await
        .map_err(ApiError::Internal)?;
    confirm_deploy_session(Path((tenant, id)), State(state)).await
}

#[derive(Serialize)]
pub(super) struct DeployReport {
    status: DeploymentSessionStatus,
    enforced_for_real: bool,
    friendly_en: String,
    friendly_th: String,
}

pub(super) async fn apply_deploy_session(
    Path((_tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(StatusCode, Json<DeployReport>)> {
    let mut session = state
        .deployment_store
        .get_deployment_session(&id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(id.clone()))?;
    let events = state
        .deployment_store
        .list_deployment_events(&id)
        .await
        .map_err(ApiError::Internal)?;
    let has_plan = events.iter().any(|event| {
        event.phase == DeploymentPhase::RoutePlanning && event.status == EventStatus::Success
    });
    let enforced_for_real = events.iter().any(|event| {
        event
            .technical_detail
            .as_ref()
            .and_then(|value| value.get("bindings"))
            .and_then(|value| value.as_array())
            .is_some_and(|bindings| {
                bindings.iter().any(|binding| {
                    binding
                        .get("effective_level")
                        .and_then(|value| value.as_str())
                        .is_some_and(|level| level.eq_ignore_ascii_case("enforce"))
                })
            })
    });

    session.status = if enforced_for_real {
        DeploymentSessionStatus::Active
    } else if has_plan {
        DeploymentSessionStatus::ObserveOnlyActive
    } else {
        DeploymentSessionStatus::ApprovalRequired
    };
    session.updated_at = chrono::Utc::now();
    state
        .deployment_store
        .upsert_deployment_session(session.clone())
        .await
        .map_err(ApiError::Internal)?;

    let (friendly_en, friendly_th, status) = if enforced_for_real {
        (
            "Protection is active and at least one control method is enforcing for real."
                .to_string(),
            "เปิดใช้การป้องกันแล้ว และมีอย่างน้อยหนึ่งวิธีควบคุมที่บังคับใช้จริง".to_string(),
            EventStatus::Success,
        )
    } else if has_plan {
        (
            "Protection is active in observe-only mode until setup actions are completed."
                .to_string(),
            "เปิดใช้การป้องกันแบบสังเกตการณ์เท่านั้นจนกว่าจะตั้งค่าเพิ่มเติมเสร็จ".to_string(),
            EventStatus::Warning,
        )
    } else {
        (
            "Deployment needs approval or setup before activation.".to_string(),
            "Deployment ต้องได้รับอนุมัติหรือตั้งค่าเพิ่มเติมก่อนเปิดใช้".to_string(),
            EventStatus::ActionRequired,
        )
    };
    let event = deployment_event(
        &session,
        if enforced_for_real {
            DeploymentPhase::Enforcement
        } else {
            DeploymentPhase::Observe
        },
        status,
        LocalizedText {
            en: "Protection state updated".into(),
            th: "อัปเดตสถานะการป้องกันแล้ว".into(),
        },
        LocalizedText {
            en: friendly_en.clone(),
            th: friendly_th.clone(),
        },
        Some(serde_json::json!({
            "enforced_for_real": enforced_for_real,
            "simulator_labeled": true,
        })),
        None,
    );
    state
        .deployment_store
        .insert_deployment_event(event)
        .await
        .map_err(ApiError::Internal)?;

    Ok((
        StatusCode::OK,
        Json(DeployReport {
            status: session.status,
            enforced_for_real,
            friendly_en,
            friendly_th,
        }),
    ))
}

pub(super) async fn rollback_deploy_session(
    Path((_tenant, id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(StatusCode, Json<DeployReport>)> {
    let mut session = state
        .deployment_store
        .get_deployment_session(&id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(id.clone()))?;
    session.status = DeploymentSessionStatus::RolledBack;
    session.updated_at = chrono::Utc::now();
    state
        .deployment_store
        .upsert_deployment_session(session.clone())
        .await
        .map_err(ApiError::Internal)?;
    let friendly_en = "Protection rollback completed for this deployment session.".to_string();
    let friendly_th = "Rollback การป้องกันสำหรับ deployment session นี้เสร็จแล้ว".to_string();
    let event = deployment_event(
        &session,
        DeploymentPhase::Rollback,
        EventStatus::Success,
        LocalizedText {
            en: "Rollback completed".into(),
            th: "Rollback เสร็จแล้ว".into(),
        },
        LocalizedText {
            en: friendly_en.clone(),
            th: friendly_th.clone(),
        },
        None,
        None,
    );
    state
        .deployment_store
        .insert_deployment_event(event)
        .await
        .map_err(ApiError::Internal)?;
    Ok((
        StatusCode::OK,
        Json(DeployReport {
            status: session.status,
            enforced_for_real: false,
            friendly_en,
            friendly_th,
        }),
    ))
}
