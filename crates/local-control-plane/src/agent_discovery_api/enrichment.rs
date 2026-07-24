//! Candidate enrichment sessions: consent-gated, local-evidence learned profiles.
use super::*;

pub(super) async fn start_candidate_enrichment(
    Path((tenant, candidate_id)): Path<(String, String)>,
    State(st): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let candidate = load_candidate(&st, &tenant, &candidate_id).await?;
    let session_id = format!("enrich_{}", uuid::Uuid::new_v4());
    let requested_sources = req
        .get("sources")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let session = build_enrichment_session(
        &session_id,
        &tenant,
        &candidate,
        "waiting_for_consent",
        requested_sources,
    );

    st.registry_store
        .upsert_raw(
            &tenant,
            "discovery_enrichment_session",
            &session_id,
            &session,
        )
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(session))
}

pub(super) async fn get_candidate_enrichment(
    Path((tenant, session_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let raw = st
        .registry_store
        .get_raw(&tenant, "discovery_enrichment_session", &session_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(session_id.clone()))?;
    Ok(Json(raw))
}

pub(super) async fn approve_candidate_enrichment(
    Path((tenant, session_id)): Path<(String, String)>,
    State(st): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut session = st
        .registry_store
        .get_raw(&tenant, "discovery_enrichment_session", &session_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(session_id.clone()))?;
    let accepted_sources = req
        .get("accepted_sources")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if let Some(obj) = session.as_object_mut() {
        obj.insert("status".into(), serde_json::json!("researched"));
        obj.insert(
            "approved_at".into(),
            serde_json::json!(chrono::Utc::now().to_rfc3339()),
        );
        obj.insert(
            "accepted_sources".into(),
            serde_json::json!(accepted_sources),
        );
        obj.insert(
            "research_result".into(),
            serde_json::json!({
                "mode": "local_safe_manifest",
                "network_fetch": "not_performed_by_default",
                "summary": "Pollek prepared a learned profile from local evidence and the selected safe source plan. Online metadata fetches require explicit source-specific connector support.",
                "facts_source": "candidate_local_evidence_and_definition_baseline"
            }),
        );
    }

    st.registry_store
        .upsert_raw(
            &tenant,
            "discovery_enrichment_session",
            &session_id,
            &session,
        )
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(session))
}

pub(super) async fn submit_candidate_enrichment(
    Path((tenant, session_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let mut session = st
        .registry_store
        .get_raw(&tenant, "discovery_enrichment_session", &session_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(session_id.clone()))?;
    let candidate_id = session
        .get("candidate_id")
        .and_then(|value| value.as_str())
        .ok_or_else(|| ApiError::BadRequest("Enrichment session has no candidate_id".into()))?
        .to_string();
    let learned_profile_id = format!("profile_{}", candidate_id);
    let definition_candidate = session
        .get("definition_candidate")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let learned_profile = serde_json::json!({
        "schema_version": "pollek.discovery.learned_profile.v1",
        "profile_id": learned_profile_id,
        "candidate_id": candidate_id,
        "session_id": session_id,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "definition_candidate": definition_candidate,
        "source_session": session
    });

    st.registry_store
        .upsert_raw(
            &tenant,
            "learned_discovery_profile",
            learned_profile
                .get("profile_id")
                .and_then(|value| value.as_str())
                .unwrap_or("profile_unknown"),
            &learned_profile,
        )
        .await
        .map_err(ApiError::Internal)?;

    if let Ok(mut candidate) = load_candidate(&st, &tenant, &candidate_id).await {
        candidate
            .labels
            .insert("learned_profile_id".into(), learned_profile_id.clone());
        candidate
            .labels
            .insert("definition_candidate_status".into(), "submitted".into());
        let value = serde_json::to_value(&candidate)
            .map_err(|error| ApiError::Internal(anyhow::anyhow!(error)))?;
        st.registry_store
            .upsert_raw(&tenant, "discovery_candidate", &candidate_id, &value)
            .await
            .map_err(ApiError::Internal)?;
    }

    if let Some(obj) = session.as_object_mut() {
        obj.insert("status".into(), serde_json::json!("submitted"));
        obj.insert(
            "submitted_at".into(),
            serde_json::json!(chrono::Utc::now().to_rfc3339()),
        );
        obj.insert(
            "learned_profile_id".into(),
            serde_json::json!(learned_profile_id),
        );
    }
    st.registry_store
        .upsert_raw(
            &tenant,
            "discovery_enrichment_session",
            &session_id,
            &session,
        )
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(session))
}

fn build_enrichment_session(
    session_id: &str,
    tenant: &str,
    candidate: &dek_agent_discovery::model::DiscoveredAgentCandidateV2,
    status: &str,
    requested_sources: Vec<String>,
) -> serde_json::Value {
    let evidence_sources = candidate
        .evidence
        .iter()
        .map(|evidence| format!("{:?}", evidence.source))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let source_plan = serde_json::json!([
        {
            "source_id": "official_site",
            "label": "Official website or documentation",
            "allowed": requested_sources.iter().any(|source| source == "official_site"),
            "network_access": "requires_user_approval",
            "safety": "https_only_public_metadata"
        },
        {
            "source_id": "package_registry",
            "label": "npm, PyPI, VS Code, or browser extension registry metadata",
            "allowed": requested_sources.iter().any(|source| source == "package_registry"),
            "network_access": "requires_user_approval",
            "safety": "metadata_only_no_install_no_execution"
        },
        {
            "source_id": "github_metadata",
            "label": "GitHub repository metadata",
            "allowed": requested_sources.iter().any(|source| source == "github_metadata"),
            "network_access": "requires_user_approval",
            "safety": "metadata_only_no_code_execution"
        },
        {
            "source_id": "mcp_manifest",
            "label": "MCP manifest or connector metadata",
            "allowed": requested_sources.iter().any(|source| source == "mcp_manifest"),
            "network_access": "requires_user_approval",
            "safety": "manifest_only_no_tool_invocation"
        }
    ]);

    serde_json::json!({
        "schema_version": "pollek.discovery.enrichment_session.v1",
        "session_id": session_id,
        "tenant_id": tenant,
        "candidate_id": candidate.candidate_id,
        "status": status,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "consent_required": true,
        "privacy_guardrails": [
            "No package installation",
            "No code execution",
            "No MCP tool invocation",
            "No prompt, response, email body, secret, or file content collection",
            "HTTPS/public metadata sources only after source approval"
        ],
        "local_evidence_summary": {
            "display_name": candidate.display_name,
            "vendor": candidate.vendor,
            "canonical_service_id": candidate.canonical_service_id,
            "surface_group_id": candidate.surface_group_id,
            "authority_boundary": candidate.authority_boundary,
            "entity_role": candidate.entity_role,
            "duplicate_policy": candidate.duplicate_policy,
            "confidence": candidate.confidence,
            "evidence_count": candidate.evidence.len(),
            "evidence_sources": evidence_sources,
            "capability_tags": candidate.capability_tags
        },
        "source_plan": source_plan,
        "extracted_facts": [
            {
                "fact": "canonical_service_id",
                "value": candidate.canonical_service_id,
                "confidence": candidate.confidence,
                "source": "local_discovery_candidate"
            },
            {
                "fact": "authority_boundary",
                "value": format!("{:?}", candidate.authority_boundary),
                "confidence": candidate.confidence,
                "source": "local_discovery_candidate"
            },
            {
                "fact": "observe_scope",
                "value": candidate.observe_scope,
                "confidence": candidate.confidence,
                "source": "local_discovery_candidate"
            }
        ],
        "definition_candidate": {
            "schema_version": "pollek.discovery.definition_candidate.v1",
            "canonical_service_id": candidate.canonical_service_id,
            "display_name": candidate.display_name,
            "vendor": candidate.vendor,
            "surface_group_id": candidate.surface_group_id,
            "authority_boundary": candidate.authority_boundary,
            "entity_role": candidate.entity_role,
            "observe_scope": candidate.observe_scope,
            "enforce_scope": candidate.enforce_scope,
            "capability_tags": candidate.capability_tags,
            "related_surfaces": candidate.related_surfaces
        }
    })
}
