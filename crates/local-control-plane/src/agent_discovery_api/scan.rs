//! Discovery scan lifecycle: launch, incremental candidate persistence, status/list/cancel.
use super::*;

struct SpoolFlowSourceImpl {
    spooler: Option<dek_telemetry::spooler::Spooler>,
}

impl SpoolFlowSourceImpl {
    fn new() -> Self {
        let db_path = dek_config::paths::get_data_dir().join("telemetry_spool.db");
        Self {
            spooler: dek_telemetry::spooler::Spooler::new(&db_path.to_string_lossy()).ok(),
        }
    }
}

impl dek_agent_discovery::web_ai_scan::SniFlowSource for SpoolFlowSourceImpl {
    fn recent_flows(
        &self,
        _since: std::time::Duration,
    ) -> Vec<dek_agent_discovery::web_ai_scan::SniFlow> {
        let mut flows = Vec::new();
        if let Some(spool) = &self.spooler {
            if let Ok(batch) = spool.peek_recent(500) {
                for (_, val) in batch {
                    if val.get("event").and_then(|v| v.as_str()) == Some("network.flow.v1") {
                        if let Some(sni_host) = val.get("sni_host").and_then(|v| v.as_str()) {
                            let browser_pid =
                                val.get("pid").and_then(|v| v.as_u64()).map(|v| v as u32);
                            flows.push(dek_agent_discovery::web_ai_scan::SniFlow {
                                browser_pid,
                                sni_host: sni_host.to_string(),
                                ts: 0,
                            });
                        }
                    }
                }
            }
        }
        flows
    }
}

pub(super) async fn start_scan(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let scan_id = format!("scan_{}", uuid::Uuid::new_v4());
    let st2 = st.clone();
    let tenant2 = tenant.clone();
    let scan_id2 = scan_id.clone();

    let initial_job = serde_json::json!({
        "scan_id": scan_id,
        "tenant_id": tenant,
        "status": "queued",
        "started_at": chrono::Utc::now().to_rfc3339(),
        "sources": req.get("sources").unwrap_or(&serde_json::json!([])),
        "candidates_found": 0
    });
    let _ = st
        .registry_store
        .upsert_raw(&tenant, "discovery_scan", &scan_id, &initial_job)
        .await;

    tokio::spawn(async move {
        let running_job = serde_json::json!({
            "scan_id": scan_id2,
            "tenant_id": tenant2,
            "status": "running",
            "started_at": chrono::Utc::now().to_rfc3339(),
            "sources": req.get("sources").unwrap_or(&serde_json::json!([])),
            "candidates_found": 0
        });
        let _ = st2
            .registry_store
            .upsert_raw(&tenant2, "discovery_scan", &scan_id2, &running_job)
            .await;

        let sni_source = std::sync::Arc::new(SpoolFlowSourceImpl::new());
        let (tx, mut rx) = tokio::sync::mpsc::channel::<
            dek_agent_discovery::model::DiscoveredAgentCandidateV2,
        >(100);
        let st3 = st2.clone();
        let tenant3 = tenant2.clone();

        // Spawn a receiver to handle incremental candidates
        let receiver_task = tokio::spawn(async move {
            while let Some(mut candidate) = rx.recv().await {
                if let Err(error) =
                    merge_and_persist_candidate(&st3, &tenant3, &mut candidate).await
                {
                    tracing::warn!(
                        %error,
                        candidate_id = %candidate.candidate_id,
                        "failed to persist incremental discovery candidate"
                    );
                }
            }
        });

        let scan_result = dek_agent_discovery::run_scan_v2(
            &tenant2,
            &scan_id2,
            &req,
            Some(sni_source),
            Some(tx),
            st2.def_store.get(),
        )
        .await;
        let _ = receiver_task.await;

        match scan_result {
            Ok((job, candidates)) => {
                for mut candidate in candidates {
                    if let Err(error) =
                        merge_and_persist_candidate(&st2, &tenant2, &mut candidate).await
                    {
                        tracing::warn!(
                            %error,
                            candidate_id = %candidate.candidate_id,
                            scan_id = %job.scan_id,
                            "failed to persist final discovery candidate snapshot"
                        );
                    }
                }
                let job_val = serde_json::to_value(&job).unwrap_or_default();
                let _ = st2
                    .registry_store
                    .upsert_raw(&tenant2, "discovery_scan", &job.scan_id, &job_val)
                    .await;
            }
            Err(e) => {
                tracing::warn!(error=%e, scan_id=%scan_id2, "agent discovery scan failed");
                let job = serde_json::json!({
                    "scan_id": scan_id2,
                    "tenant_id": tenant2,
                    "status": "failed",
                    "error": e.to_string(),
                });
                let _ = st2
                    .registry_store
                    .upsert_raw(&tenant2, "discovery_scan", &scan_id2, &job)
                    .await;
            }
        }
    });

    Ok(Json(serde_json::json!({
        "schema_version": "agent-discovery-scan-response.v1",
        "scan_id": scan_id,
        "status": "queued"
    })))
}

async fn merge_and_persist_candidate(
    st: &AppState,
    tenant: &str,
    candidate: &mut dek_agent_discovery::model::DiscoveredAgentCandidateV2,
) -> anyhow::Result<()> {
    if let Some(existing_raw) = st
        .registry_store
        .get_raw(tenant, "discovery_candidate", &candidate.candidate_id)
        .await?
    {
        if let Ok(existing) = serde_json::from_value::<
            dek_agent_discovery::model::DiscoveredAgentCandidateV2,
        >(existing_raw)
        {
            candidate.first_seen = existing.first_seen.clone();
            for scan_id in &existing.scan_ids {
                if !candidate.scan_ids.iter().any(|id| id == scan_id) {
                    candidate.scan_ids.push(scan_id.clone());
                }
            }
            if matches!(
                &existing.status,
                dek_agent_discovery::model::DiscoveryStatus::Registered
                    | dek_agent_discovery::model::DiscoveryStatus::Ignored
            ) {
                if matches!(
                    &existing.status,
                    dek_agent_discovery::model::DiscoveryStatus::Registered
                ) {
                    if let Some(agent_id) =
                        registered_agent_id_for_candidate(st, tenant, &existing).await?
                    {
                        candidate.status = existing.status.clone();
                        candidate.display_name = existing.display_name.clone();
                        candidate.suggested_registration.name =
                            existing.suggested_registration.name.clone();
                        candidate
                            .labels
                            .insert("registered_agent_id".to_string(), agent_id);
                    }
                } else {
                    candidate.status = existing.status.clone();
                    candidate.display_name = existing.display_name.clone();
                    candidate.suggested_registration.name =
                        existing.suggested_registration.name.clone();
                }
            }
        }
    }

    reconcile_candidate_registered_status(st, tenant, candidate).await?;
    let val = serde_json::to_value(&*candidate)?;
    st.registry_store
        .upsert_raw(tenant, "discovery_candidate", &candidate.candidate_id, &val)
        .await?;
    Ok(())
}

pub(super) async fn get_scan_status(
    Path((tenant, scan_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let raw = st
        .registry_store
        .get_raw(&tenant, "discovery_scan", &scan_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(scan_id.clone()))?;

    Ok(Json(raw))
}

pub(super) async fn list_scans(
    Path(tenant): Path<String>,
    Query(query): Query<PaginationQuery>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut items = st
        .registry_store
        .list_raw(&tenant, "discovery_scan")
        .await
        .map_err(ApiError::Internal)?;

    let limit = query.limit.unwrap_or(100);
    let cursor = query.cursor.unwrap_or(0);

    let total = items.len();
    items = items.into_iter().skip(cursor).take(limit).collect();

    Ok(Json(serde_json::json!({
        "schema_version": "agent-discovery-scan-list.v1",
        "scans": items,
        "next_cursor": if cursor + limit < total { Some(cursor + limit) } else { None },
        "total": total
    })))
}

pub(super) async fn cancel_scan(
    Path((tenant, scan_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let raw = st
        .registry_store
        .get_raw(&tenant, "discovery_scan", &scan_id)
        .await
        .map_err(ApiError::Internal)?;

    if let Some(mut scan_val) = raw {
        if scan_val.get("status").and_then(|v| v.as_str()) == Some("queued")
            || scan_val.get("status").and_then(|v| v.as_str()) == Some("running")
        {
            if let Some(obj) = scan_val.as_object_mut() {
                obj.insert("status".to_string(), serde_json::json!("cancelled"));
            }
            let _ = st
                .registry_store
                .upsert_raw(&tenant, "discovery_scan", &scan_id, &scan_val)
                .await;
        }
        Ok(Json(scan_val))
    } else {
        Err(ApiError::NotFound(scan_id))
    }
}
