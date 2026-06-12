use crate::{
    error::{ApiError, ApiResult},
    state::AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use dek_control_plane_api::policy::*;
use serde_json::json;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant_id/policies",
            get(list_policies).post(create_policy),
        )
        .route(
            "/v1/tenants/:tenant_id/policies/:policy_id",
            get(get_policy).patch(patch_policy).delete(delete_policy),
        )
        .route(
            "/v1/tenants/:tenant_id/policies/:policy_id/publish",
            post(publish_policy),
        )
        .route(
            "/v1/tenants/:tenant_id/policies/:policy_id/validate",
            post(validate_policy),
        )
        .route(
            "/v1/tenants/:tenant_id/policies/:policy_id/simulate",
            post(simulate_policy),
        )
}

async fn list_policies(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let items = state
        .policy_store
        .list_policies(&tenant_id)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(json!(items)))
}

async fn create_policy(
    Path(tenant_id): Path<String>,
    State(state): State<AppState>,
    Json(mut payload): Json<PolicyDraft>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    payload.meta.tenant_id = tenant_id;
    let item = state
        .policy_store
        .upsert_policy(payload)
        .await
        .map_err(ApiError::Internal)?;
    Ok((StatusCode::CREATED, Json(json!(item))))
}

async fn get_policy(
    Path((tenant_id, policy_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<Json<serde_json::Value>> {
    let item = state
        .policy_store
        .get_policy(&tenant_id, &policy_id)
        .await
        .map_err(ApiError::Internal)?;
    match item {
        Some(i) => Ok(Json(json!(i))),
        None => Err(ApiError::NotFound(policy_id)),
    }
}

async fn patch_policy(
    Path((tenant_id, _policy_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(mut payload): Json<PolicyDraft>,
) -> ApiResult<Json<serde_json::Value>> {
    payload.meta.tenant_id = tenant_id;
    let item = state
        .policy_store
        .upsert_policy(payload)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(json!(item)))
}

async fn delete_policy(
    Path((tenant_id, policy_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let deleted = state
        .policy_store
        .delete_policy(&tenant_id, &policy_id)
        .await
        .map_err(ApiError::Internal)?;
    if deleted {
        Ok((StatusCode::NO_CONTENT, Json(json!({}))))
    } else {
        Err(ApiError::NotFound(policy_id))
    }
}

async fn publish_policy(
    Path((tenant, policy_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let item = st
        .policy_store
        .get_policy(&tenant, &policy_id)
        .await
        .map_err(ApiError::Internal)?;
        
    let mut draft = match item {
        Some(d) => d,
        None => return Err(ApiError::NotFound(policy_id)),
    };
    let build_number = st
        .build_number
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let mut compiled = vec![];
    match &draft.source {
        dek_control_plane_api::policy::PolicySource::RawText { language, text } => {
            compiled.push(crate::bundle::CompiledArtifact {
                artifact_id: draft.name.clone(),
                adapter_id: language.clone(),
                artifact_type: format!("{}_text", language),
                bytes: text.as_bytes().to_vec(),
            });
        }
        dek_control_plane_api::policy::PolicySource::Structured { ir } => {
            if let Ok(intent) =
                serde_json::from_value::<dek_policy_intent::PolicyIntent>(ir.clone())
            {
                if let Ok(compiled_policy) =
                    dek_policy_compiler::CompilerOrchestrator::compile(&intent)
                {
                    compiled.push(crate::bundle::CompiledArtifact {
                        artifact_id: draft.name.clone(),
                        adapter_id: compiled_policy.engine.clone(),
                        artifact_type: format!("{}_text", compiled_policy.engine),
                        bytes: compiled_policy.compiled_bytes,
                    });
                } else {
                    return Err(ApiError::Internal(anyhow::anyhow!("Compilation failed")));
                }
            } else {
                return Err(ApiError::Internal(anyhow::anyhow!(
                    "Invalid PolicyIntent IR"
                )));
            }
        }
        _ => {
            compiled.push(crate::bundle::CompiledArtifact {
                artifact_id: draft.name.clone(),
                adapter_id: "cedar".into(),
                artifact_type: "cedar_text".into(),
                bytes: b"permit(principal,action,resource);".to_vec(),
            });
        }
    }

    let agents = st
        .registry_store
        .list_agents(&tenant)
        .await
        .unwrap_or_default();
    let registry_snap = serde_json::json!({ "agents": agents });

    let built = crate::bundle::build_signed_bundle(
        &st.signer,
        &tenant,
        "default",
        "local",
        build_number,
        compiled,
        &registry_snap,
        &serde_json::json!({}),
        None,
    )
    .await
    .map_err(ApiError::Internal)?;

    st.policy_store
        .upsert_policy_raw(&tenant, "bundle:latest", &built.envelope)
        .await
        .map_err(ApiError::Internal)?;

    draft.meta.status = "published".to_string();
    st.policy_store
        .upsert_policy(draft)
        .await
        .map_err(ApiError::Internal)?;

    for (path, bytes) in built.blobs {
        st.policy_store
            .put_blob(&tenant, &path, &bytes)
            .await
            .map_err(ApiError::Internal)?;
    }

    let _ = st.bundle_tx.send(built.manifest.metadata.bundle_id.clone());

    Ok((
        StatusCode::OK,
        Json(json!({
            "published": true,
            "bundle_id": built.manifest.metadata.bundle_id,
            "manifest": built.envelope
        })),
    ))
}

async fn validate_policy(
    Path((tenant_id, policy_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let item = state
        .policy_store
        .get_policy(&tenant_id, &policy_id)
        .await
        .map_err(ApiError::Internal)?;

    let policy = match item {
        Some(p) => p,
        None => return Err(ApiError::NotFound(policy_id)),
    };

    let mut errors = vec![];

    if let PolicySource::Structured { ir } = policy.source {
        if let Err(e) = serde_json::from_value::<dek_policy_intent::PolicyIntent>(ir) {
            errors.push(format!("PPI Validation Error: {}", e));
        }
    }

    let is_valid = errors.is_empty();

    Ok((
        StatusCode::OK,
        Json(json!({ "is_valid": is_valid, "errors": errors })),
    ))
}

async fn simulate_policy(
    Path((tenant_id, policy_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(input): Json<serde_json::Value>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let _ = (tenant_id, policy_id, state, input);
    Ok((
        StatusCode::OK,
        Json(json!({
            "allowed": true,
            "evaluation_time_ms": 2,
            "log_output": ["mock simulate"]
        })),
    ))
}
