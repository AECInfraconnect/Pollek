use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};

mod candidates;
mod capabilities;
mod enrichment;
mod scan;

use candidates::*;
use capabilities::*;
use enrichment::*;
use scan::*;

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<usize>,
    pub cursor: Option<usize>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/discovery/scans",
            post(start_scan).get(list_scans),
        )
        .route(
            "/v1/tenants/:tenant/discovery/scans/:scan_id",
            get(get_scan_status),
        )
        .route(
            "/v1/tenants/:tenant/discovery/scans/:scan_id/cancel",
            post(cancel_scan),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates",
            get(list_candidates).delete(clear_candidates),
        )
        .route(
            "/v1/tenants/:tenant/discovery/entities",
            get(list_discovery_entities),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate_id/capabilities",
            get(get_candidate_capabilities),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate_id/enrichment/start",
            post(start_candidate_enrichment),
        )
        .route(
            "/v1/tenants/:tenant/discovery/enrichment/:session_id",
            get(get_candidate_enrichment),
        )
        .route(
            "/v1/tenants/:tenant/discovery/enrichment/:session_id/approve",
            post(approve_candidate_enrichment),
        )
        .route(
            "/v1/tenants/:tenant/discovery/enrichment/:session_id/submit",
            post(submit_candidate_enrichment),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate_id/retrieve-capabilities",
            post(retrieve_candidate_capabilities),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate_id",
            axum::routing::delete(delete_candidate),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate/register",
            post(register_candidate),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate_id/confirm",
            post(confirm_candidate),
        )
        .route(
            "/v1/tenants/:tenant/discovery/candidates/:candidate_id/control-plan",
            post(generate_control_plan),
        )
        .route(
            "/v1/tenants/:tenant/discovery/control-bindings/:binding_id/apply",
            post(crate::control_binding::apply_control_binding),
        )
        .route(
            "/v1/tenants/:tenant/discovery/control-bindings/:binding_id/rollback",
            post(crate::control_binding::rollback_control_binding),
        )
}

/// Load a discovery candidate by id, mapping storage/deserialization failures to API errors.
async fn load_candidate(
    st: &AppState,
    tenant: &str,
    candidate_id: &str,
) -> ApiResult<dek_agent_discovery::model::DiscoveredAgentCandidateV2> {
    let raw = st
        .registry_store
        .get_raw(tenant, "discovery_candidate", candidate_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::NotFound(candidate_id.to_string()))?;

    serde_json::from_value(raw).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))
}

/// Resolve the registry agent id (if any) that a discovery candidate has already been
/// registered as, matching by stable key, suggested id, label, or discovery merge key.
pub(crate) async fn registered_agent_id_for_candidate(
    st: &AppState,
    tenant: &str,
    candidate: &dek_agent_discovery::model::DiscoveredAgentCandidateV2,
) -> anyhow::Result<Option<String>> {
    let mut direct_ids = vec![dek_agent_discovery::stable_agent_key(candidate)];
    if !candidate.suggested_registration.agent_id.is_empty() {
        direct_ids.push(candidate.suggested_registration.agent_id.clone());
    }
    if let Some(agent_id) = candidate.labels.get("registered_agent_id") {
        direct_ids.push(agent_id.clone());
    }

    direct_ids.sort();
    direct_ids.dedup();
    for agent_id in direct_ids {
        if st
            .registry_store
            .get_raw(tenant, "agent", &agent_id)
            .await?
            .is_some()
        {
            return Ok(Some(agent_id));
        }
    }

    let candidate_merge_key = candidate
        .evidence
        .iter()
        .find_map(|ev| ev.merge_key.as_deref());
    for agent in st.registry_store.list_agents(tenant).await? {
        if agent
            .labels
            .get("discovery_candidate_id")
            .is_some_and(|id| id == &candidate.candidate_id)
        {
            return Ok(Some(agent.agent_id));
        }
        if let (Some(agent_merge_key), Some(candidate_merge_key)) = (
            agent.labels.get("discovery_candidate_merge_key"),
            candidate_merge_key,
        ) {
            if agent_merge_key == candidate_merge_key {
                return Ok(Some(agent.agent_id));
            }
        }
    }

    Ok(None)
}

/// Re-derive a candidate's registered status against the live agent registry, promoting to
/// `Registered` when a match exists or demoting a stale `Registered` back to `PendingApproval`.
async fn reconcile_candidate_registered_status(
    st: &AppState,
    tenant: &str,
    candidate: &mut dek_agent_discovery::model::DiscoveredAgentCandidateV2,
) -> anyhow::Result<()> {
    if let Some(agent_id) = registered_agent_id_for_candidate(st, tenant, candidate).await? {
        candidate.status = dek_agent_discovery::model::DiscoveryStatus::Registered;
        candidate
            .labels
            .insert("registered_agent_id".to_string(), agent_id);
    } else if matches!(
        candidate.status,
        dek_agent_discovery::model::DiscoveryStatus::Registered
    ) {
        candidate.status = dek_agent_discovery::model::DiscoveryStatus::PendingApproval;
        candidate.labels.remove("registered_agent_id");
    }

    Ok(())
}
