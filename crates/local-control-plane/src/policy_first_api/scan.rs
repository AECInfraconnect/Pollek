//! Agent scan sessions: kick off a policy-first discovery scan across the
//! requested sources, stream progress events, and expose scan results.

use super::*;

#[derive(Serialize)]
pub(super) struct ScanResponse {
    job_id: String,
    scan_id: String,
    status: ScanStatus,
}

pub(super) async fn scan_agents(
    Path(tenant): Path<String>,
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> ApiResult<(StatusCode, Json<ScanResponse>)> {
    let (_status, Json(session)) = create_scan_session(Path(tenant), State(state), body).await?;
    let scan_id = session.scan_id;
    Ok((
        StatusCode::ACCEPTED,
        Json(ScanResponse {
            job_id: scan_id.clone(),
            scan_id,
            status: ScanStatus::Queued,
        }),
    ))
}

pub(super) async fn get_scan_result(
    Path((tenant, job_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let raw = state
        .registry_store
        .get_raw(&tenant, "policy_first_scan_session", &job_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(job_id.clone()))?;
    Ok((StatusCode::OK, Json(raw)))
}

pub(super) fn requested_sources(req: &serde_json::Value) -> Vec<DiscoverySourceKind> {
    let parse = |value: &serde_json::Value| match value.as_str() {
        Some("process") => Some(DiscoverySourceKind::ProcessScan),
        Some("mcp_config") => Some(DiscoverySourceKind::McpConfigScan),
        Some("browser_extension") => Some(DiscoverySourceKind::BrowserExtensionScan),
        Some("local_model") => Some(DiscoverySourceKind::LocalModelScan),
        Some("container") => Some(DiscoverySourceKind::ContainerScan),
        Some("web_ai") | Some("network_egress") => Some(DiscoverySourceKind::NetworkEgress),
        Some("ide_extension") => Some(DiscoverySourceKind::IdeExtensionScan),
        Some("cli_agent") => Some(DiscoverySourceKind::CliAgentScan),
        Some("installed_app") => Some(DiscoverySourceKind::InstalledAppScan),
        Some("python_framework") => Some(DiscoverySourceKind::PythonFrameworkScan),
        _ => None,
    };

    req.get("sources")
        .and_then(|v| v.as_array())
        .map(|items| items.iter().filter_map(parse).collect::<Vec<_>>())
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| {
            vec![
                DiscoverySourceKind::ProcessScan,
                DiscoverySourceKind::McpConfigScan,
                DiscoverySourceKind::LocalModelScan,
                DiscoverySourceKind::IdeExtensionScan,
                DiscoverySourceKind::CliAgentScan,
                DiscoverySourceKind::ContainerScan,
                DiscoverySourceKind::BrowserExtensionScan,
                DiscoverySourceKind::InstalledAppScan,
                DiscoverySourceKind::NetworkEgress,
                DiscoverySourceKind::PythonFrameworkScan,
            ]
        })
}

pub(super) fn source_result(
    source: DiscoverySourceKind,
    state: ScanSourceState,
) -> DiscoverySourceResult {
    let (privacy_en, privacy_th) = match source {
        DiscoverySourceKind::ProcessScan => (
            "Pollek checks process metadata, redacted paths, and hashes.",
            "Pollek ตรวจ metadata ของ process, path ที่ redacted แล้ว และ hash",
        ),
        DiscoverySourceKind::McpConfigScan => (
            "Pollek checks known MCP configuration locations and redacts local paths.",
            "Pollek ตรวจตำแหน่ง MCP configuration ที่รู้จักและ redacted path ในเครื่อง",
        ),
        DiscoverySourceKind::BrowserExtensionScan => (
            "Browser AI visibility requires a browser extension or profile permission.",
            "การมองเห็น AI บน browser ต้องใช้ extension หรือสิทธิ์ profile",
        ),
        DiscoverySourceKind::NetworkEgress => (
            "Real network blocking requires OS-level setup; simulator events are labeled.",
            "การบล็อกเครือข่ายจริงต้องตั้งค่าระดับ OS และ event จำลองจะถูกติดป้ายชัดเจน",
        ),
        _ => (
            "Pollek stores source-level evidence with sensitive fields redacted.",
            "Pollek เก็บหลักฐานระดับ source โดย redacted ข้อมูลอ่อนไหว",
        ),
    };

    DiscoverySourceResult {
        source,
        status: state,
        candidates_found: 0,
        evidence_found: 0,
        error_message: None,
        privacy_note_en: privacy_en.into(),
        privacy_note_th: privacy_th.into(),
    }
}

pub(super) async fn create_scan_session(
    Path(tenant): Path<String>,
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> ApiResult<(StatusCode, Json<ScanSessionV2>)> {
    // An empty or invalid body means "scan with defaults" — the wizard sends no body.
    let req = serde_json::from_slice::<serde_json::Value>(&body)
        .unwrap_or_else(|_| serde_json::json!({}));
    let scan_id = format!("scan_{}", uuid::Uuid::new_v4());
    let device_id = local_device_id();
    let sources = requested_sources(&req);
    let started_at = chrono::Utc::now();
    let session = ScanSessionV2 {
        schema_version: "discovery-scan-session.v2".into(),
        scan_id: scan_id.clone(),
        tenant_id: tenant.clone(),
        device_id: device_id.clone(),
        status: ScanStatus::Queued,
        requested_sources: sources.clone(),
        source_results: sources
            .iter()
            .cloned()
            .map(|source| source_result(source, ScanSourceState::Queued))
            .collect(),
        candidates_found: 0,
        started_at,
        finished_at: None,
        friendly_summary_en: "Scan queued. Pollek will check local agent evidence sources.".into(),
        friendly_summary_th: "เข้าคิวสแกนแล้ว Pollek จะตรวจแหล่งหลักฐาน Agent บนเครื่อง".into(),
    };

    let value = serde_json::to_value(&session).map_err(|e| ApiError::Internal(e.into()))?;
    state
        .registry_store
        .upsert_raw(&tenant, "policy_first_scan_session", &scan_id, &value)
        .await
        .map_err(ApiError::Internal)?;

    let state2 = state.clone();
    let tenant2 = tenant.clone();
    let scan_id2 = scan_id.clone();
    tokio::spawn(async move {
        run_policy_first_scan(state2, tenant2, scan_id2, device_id, sources, req).await;
    });

    Ok((StatusCode::ACCEPTED, Json(session)))
}

pub(super) async fn run_policy_first_scan(
    state: AppState,
    tenant: String,
    scan_id: String,
    device_id: String,
    sources: Vec<DiscoverySourceKind>,
    req: serde_json::Value,
) {
    let running = ScanSessionV2 {
        schema_version: "discovery-scan-session.v2".into(),
        scan_id: scan_id.clone(),
        tenant_id: tenant.clone(),
        device_id: device_id.clone(),
        status: ScanStatus::Running,
        requested_sources: sources.clone(),
        source_results: sources
            .iter()
            .cloned()
            .map(|source| source_result(source, ScanSourceState::Running))
            .collect(),
        candidates_found: 0,
        started_at: chrono::Utc::now(),
        finished_at: None,
        friendly_summary_en: "Pollek is scanning local evidence sources.".into(),
        friendly_summary_th: "Pollek กำลังสแกนแหล่งหลักฐานบนเครื่อง".into(),
    };
    if let Ok(value) = serde_json::to_value(&running) {
        let _ = state
            .registry_store
            .upsert_raw(&tenant, "policy_first_scan_session", &scan_id, &value)
            .await;
    }

    let req_with_sources = serde_json::json!({
        "sources": sources.iter().map(DiscoverySourceKind::as_api_source).collect::<Vec<_>>()
    });
    let scan_req = if req.get("sources").is_some() {
        req
    } else {
        req_with_sources
    };

    let result = dek_agent_discovery::run_scan_v2(
        &tenant,
        &scan_id,
        &scan_req,
        None,
        None,
        state.def_store.get(),
    )
    .await;

    match result {
        Ok((job, candidates)) => {
            for candidate in &candidates {
                if let Ok(value) = serde_json::to_value(candidate) {
                    let _ = state
                        .registry_store
                        .upsert_raw(
                            &tenant,
                            "discovery_candidate",
                            &candidate.candidate_id,
                            &value,
                        )
                        .await;
                }
            }
            let status = match job.status {
                dek_agent_discovery::model::ScanJobStatus::Completed => ScanStatus::Completed,
                dek_agent_discovery::model::ScanJobStatus::Partial => ScanStatus::Partial,
                dek_agent_discovery::model::ScanJobStatus::Failed => ScanStatus::Failed,
                dek_agent_discovery::model::ScanJobStatus::Queued => ScanStatus::Queued,
                dek_agent_discovery::model::ScanJobStatus::Running => ScanStatus::Running,
            };
            let per_source_count = if sources.is_empty() {
                0
            } else {
                candidates.len() as u32
            };
            let finished = ScanSessionV2 {
                status: status.clone(),
                source_results: sources
                    .iter()
                    .cloned()
                    .map(|source| {
                        let mut result = source_result(source, ScanSourceState::Completed);
                        result.candidates_found = per_source_count;
                        result.evidence_found = candidates
                            .iter()
                            .map(|candidate| candidate.evidence.len() as u32)
                            .sum();
                        result
                    })
                    .collect(),
                candidates_found: candidates.len() as u32,
                finished_at: Some(chrono::Utc::now()),
                friendly_summary_en: format!(
                    "Scan completed with {} agent candidate(s).",
                    candidates.len()
                ),
                friendly_summary_th: format!(
                    "สแกนเสร็จแล้ว พบ candidate {} รายการ",
                    candidates.len()
                ),
                ..running
            };
            if let Ok(value) = serde_json::to_value(&finished) {
                let _ = state
                    .registry_store
                    .upsert_raw(&tenant, "policy_first_scan_session", &scan_id, &value)
                    .await;
            }
        }
        Err(err) => {
            let failed = ScanSessionV2 {
                status: ScanStatus::Failed,
                source_results: sources
                    .iter()
                    .cloned()
                    .map(|source| {
                        let mut result = source_result(source, ScanSourceState::Failed);
                        result.error_message = Some(err.to_string());
                        result
                    })
                    .collect(),
                finished_at: Some(chrono::Utc::now()),
                friendly_summary_en: "Scan failed. Check source-level errors for setup guidance."
                    .into(),
                friendly_summary_th: "สแกนไม่สำเร็จ โปรดดู error ราย source เพื่อดูวิธีตั้งค่า".into(),
                ..running
            };
            if let Ok(value) = serde_json::to_value(&failed) {
                let _ = state
                    .registry_store
                    .upsert_raw(&tenant, "policy_first_scan_session", &scan_id, &value)
                    .await;
            }
        }
    }
}

pub(super) async fn get_scan_session(
    Path((tenant, scan_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let raw = state
        .registry_store
        .get_raw(&tenant, "policy_first_scan_session", &scan_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(scan_id.clone()))?;
    Ok((StatusCode::OK, Json(raw)))
}

pub(super) async fn get_scan_session_events(
    Path((tenant, scan_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let raw = state
        .registry_store
        .get_raw(&tenant, "policy_first_scan_session", &scan_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(scan_id.clone()))?;
    let events = raw
        .get("source_results")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "events": events })),
    ))
}
