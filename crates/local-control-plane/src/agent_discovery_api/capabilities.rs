//! Candidate capability inventory: derive and persist discovery entities/capabilities/relationships.
use super::*;

pub(super) async fn get_candidate_capabilities(
    Path((tenant, candidate_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let candidate = load_candidate(&st, &tenant, &candidate_id).await?;
    let entity = dek_agent_discovery::capability_inventory::entity_for_candidate(&candidate);
    Ok(Json(capability_inventory_response(&entity, "derived")))
}

pub(super) async fn retrieve_candidate_capabilities(
    Path((tenant, candidate_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let candidate = load_candidate(&st, &tenant, &candidate_id).await?;
    let entity = dek_agent_discovery::capability_inventory::entity_for_candidate(&candidate);
    persist_discovery_entity(&st, &tenant, &entity).await?;

    Ok(Json(capability_inventory_response(&entity, "persisted")))
}

async fn persist_discovery_entity(
    st: &AppState,
    tenant: &str,
    entity: &dek_agent_discovery::model::DiscoveryEntityCandidate,
) -> ApiResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let entity_value =
        serde_json::to_value(entity).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    st.registry_store
        .upsert_raw(
            tenant,
            "discovery_entity",
            &entity.candidate_id,
            &entity_value,
        )
        .await
        .map_err(ApiError::Internal)?;

    for capability in &entity.capabilities {
        let mut value =
            serde_json::to_value(capability).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert("updated_at".to_string(), serde_json::json!(now.clone()));
        }
        st.registry_store
            .upsert_raw(
                tenant,
                "discovered_capability",
                &capability.capability_id,
                &value,
            )
            .await
            .map_err(ApiError::Internal)?;
    }

    for relationship in &entity.relationships {
        let mut value = serde_json::to_value(relationship)
            .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert("updated_at".to_string(), serde_json::json!(now.clone()));
        }
        st.registry_store
            .upsert_raw(
                tenant,
                "discovered_relationship",
                &relationship.relationship_id,
                &value,
            )
            .await
            .map_err(ApiError::Internal)?;
    }

    Ok(())
}

fn capability_inventory_response(
    entity: &dek_agent_discovery::model::DiscoveryEntityCandidate,
    retrieval_status: &str,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "discovery-capability-inventory.v1",
        "candidate_id": entity.candidate_id,
        "entity": entity,
        "capabilities": entity.capabilities,
        "relationships": entity.relationships,
        "retrieval_status": retrieval_status,
        "source": "local_discovery_metadata",
        "privacy_note": "Discovery capability inventory is derived from metadata already collected by Auto Discovery. It does not invoke MCP tools, read MCP resources, or capture raw prompts/responses."
    })
}
