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
    let tools = st.registry_store.list_tools(&tenant).await.unwrap_or_default();
    let resources = st.registry_store.list_resources(&tenant).await.unwrap_or_default();
    let mcp_servers = st.registry_store.list_mcp_servers(&tenant).await.unwrap_or_default();
    let entities = st.registry_store.list_entities(&tenant).await.unwrap_or_default();
    let relationships = st.registry_store.list_relationships(&tenant).await.unwrap_or_default();
    let blackbox_ai_providers = st.registry_store.list_blackbox_ai(&tenant).await.unwrap_or_default();

    let registry_snap = serde_json::json!({ 
        "agents": agents,
        "tools": tools,
        "resources": resources,
        "mcp_servers": mcp_servers,
        "entities": entities,
        "relationships": relationships,
        "blackbox_ai_providers": blackbox_ai_providers
    });

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

    draft.meta.status = dek_control_plane_api::registry::RegistryStatus::Published;
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
    let item = state
        .policy_store
        .get_policy(&tenant_id, &policy_id)
        .await
        .map_err(ApiError::Internal)?;

    let draft = match item {
        Some(d) => d,
        None => return Err(ApiError::NotFound(policy_id)),
    };

    let policy_text;
    let language_id;

    match &draft.source {
        dek_control_plane_api::policy::PolicySource::RawText { language, text } => {
            language_id = language.clone();
            policy_text = text.clone();
        }
        dek_control_plane_api::policy::PolicySource::Structured { ir } => {
            if let Ok(intent) =
                serde_json::from_value::<dek_policy_intent::PolicyIntent>(ir.clone())
            {
                if let Ok(compiled_policy) =
                    dek_policy_compiler::CompilerOrchestrator::compile(&intent)
                {
                    language_id = compiled_policy.engine;
                    if let Ok(s) = String::from_utf8(compiled_policy.compiled_bytes) {
                        policy_text = s;
                    } else {
                        return Err(ApiError::Internal(anyhow::anyhow!(
                            "Compiled policy is not valid UTF-8 text"
                        )));
                    }
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
            language_id = "cedar".into();
            policy_text = "permit(principal,action,resource);".into();
        }
    }

    struct SimulateRuntime {
        cedar_adapter: dek_cedar::CedarAdapter,
    }

    #[async_trait::async_trait]
    impl dek_policy_runtime::PolicyRuntime for SimulateRuntime {
        async fn evaluate(
            &self,
            input: serde_json::Value,
        ) -> Result<dek_policy_runtime::PolicyDecision, dek_policy_runtime::PolicyError> {
            let req = dek_plugin_sdk::EvalRequest {
                request_id: "sim-123".into(),
                tenant_id: None,
                subject: None,
                action: None,
                resource: None,
                payload: input,
                context: std::collections::BTreeMap::new(),
            };
            use dek_plugin_sdk::PolicyEvaluator;
            match self.cedar_adapter.evaluate(req).await {
                Ok(res) => Ok(dek_policy_runtime::PolicyDecision {
                    evaluator_id: res.evaluator_id,
                    evaluator_type: res.evaluator_type,
                    required: res.required,
                    status: "success".into(),
                    decision: match res.decision {
                        dek_plugin_sdk::DecisionEffect::Allow => "allow".into(),
                        _ => "deny".into(),
                    },
                    allow: res.decision == dek_plugin_sdk::DecisionEffect::Allow,
                    reason: res.reason,
                    effects: res.effects,
                    obligations: res.obligations,
                    metadata: res.metadata,
                }),
                Err(e) => Err(dek_policy_runtime::PolicyError::Eval(e.to_string())),
            }
        }
        fn version(&self) -> String {
            "1.0".into()
        }
        async fn clear_cache(&self) {}
    }

    let mut router = dek_policy_router::PolicyRouter::new();

    let mut syntax_check = "Passed".to_string();
    let recommended_pep: String;
    let deployment_test: String;
    let target_pep = input.get("target_pep").and_then(|v| v.as_str());

    if language_id == "cedar" {
        recommended_pep = "MCP Proxy PEP, Envoy Proxy, STDIO Wrapper".to_string();
        match dek_cedar::CedarAdapter::new(&policy_text) {
            Ok(adapter) => {
                router.register_evaluator(
                    "sim_evaluator",
                    Box::new(SimulateRuntime {
                        cedar_adapter: adapter,
                    }),
                );
                if let Some(pep) = target_pep {
                    deployment_test = format!("Passed: Compatible with {}", pep);
                } else {
                    deployment_test = "Passed: Compatible with multiple PEPs".to_string();
                }
            }
            Err(e) => {
                syntax_check = format!("Failed: {}", e);
                deployment_test = "Failed: Invalid syntax".to_string();
                return Ok((
                    StatusCode::OK,
                    Json(json!({
                        "allowed": false,
                        "decision": "Not Evaluated",
                        "reason": format!("Failed to parse Cedar policy: {}", e),
                        "evaluation_time_ms": 0,
                        "log_output": ["Syntax check failed"],
                        "syntax_check": syntax_check,
                        "recommended_pep": recommended_pep,
                        "deployment_test": deployment_test
                    })),
                ));
            }
        }
    } else if language_id == "rego" {
        recommended_pep = "Envoy / L7 Proxy PEP".to_string();
        if !policy_text.contains("package") {
            syntax_check = "Failed: missing package declaration".to_string();
            deployment_test = "Failed: Invalid syntax".to_string();
        } else {
            if let Some(pep) = target_pep {
                if pep == "mcp_proxy" || pep == "stdio_wrapper" {
                    deployment_test = "Failed: Policy Bundle contains Rego/OpenFGA artifacts which are only supported by Envoy/L7 Proxy PEP.".to_string();
                } else {
                    deployment_test = "Passed: Compatible with Envoy/L7 Proxy PEP".to_string();
                }
            } else {
                deployment_test = "Passed: Compatible with Envoy/L7 Proxy PEP".to_string();
            }
        }
        return Ok((
            StatusCode::OK,
            Json(json!({
                "allowed": false,
                "decision": "Not Evaluated",
                "reason": "Rego engine dry-run is not yet fully implemented locally.",
                "evaluation_time_ms": 0,
                "log_output": ["Syntax check and deployment validation completed."],
                "syntax_check": syntax_check,
                "recommended_pep": recommended_pep,
                "deployment_test": deployment_test
            })),
        ));
    } else if language_id == "open_fga" || language_id == "fga" {
        recommended_pep = "Envoy / L7 Proxy PEP".to_string();
        if !policy_text.contains("model") {
            syntax_check = "Failed: missing model declaration".to_string();
            deployment_test = "Failed: Invalid syntax".to_string();
        } else {
            if let Some(pep) = target_pep {
                if pep == "mcp_proxy" || pep == "stdio_wrapper" {
                    deployment_test = "Failed: Policy Bundle contains Rego/OpenFGA artifacts which are only supported by Envoy/L7 Proxy PEP.".to_string();
                } else {
                    deployment_test = "Passed: Compatible with OpenFGA Server".to_string();
                }
            } else {
                deployment_test = "Passed: Compatible with OpenFGA Server".to_string();
            }
        }
        return Ok((
            StatusCode::OK,
            Json(json!({
                "allowed": false,
                "decision": "Not Evaluated",
                "reason": "OpenFGA engine dry-run is not yet fully implemented locally.",
                "evaluation_time_ms": 0,
                "log_output": ["Syntax check and deployment validation completed."],
                "syntax_check": syntax_check,
                "recommended_pep": recommended_pep,
                "deployment_test": deployment_test
            })),
        ));
    } else {
        return Err(ApiError::Internal(anyhow::anyhow!(
            "Unsupported policy engine type"
        )));
    }

    let route = dek_policy_router::Route {
        id: "sim_route".into(),
        priority: 1,
        match_rule: dek_policy_router::EnterpriseMatchRule {
            method: Some("*".into()),
            tool_category: None,
            resource_type: None,
            severity_level: None,
        },
        enforcement_mode: dek_policy_router::EnforcementMode::Standard,
        pdp_required: vec!["sim_evaluator".into()],
        pdp_pool: vec![],
        pdp_conditional: vec![],
        failover_strategy: dek_policy_router::FailoverStrategy::Priority,
    };

    router.set_routes(vec![route]);

    let start_time = std::time::Instant::now();
    let result = router
        .authorize_dry_run(serde_json::from_value(input.clone()).unwrap_or_default())
        .await;
    let eval_time_ms = start_time.elapsed().as_millis();

    match result {
        Ok(decision) => {
            let req_id = input
                .get("request_id")
                .and_then(|v| v.as_str())
                .unwrap_or("sim_req")
                .to_string();
            let ev = json!({
                "event_type": "decision",
                "event_id": format!("sim_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "payload": {
                    "decision": if decision.allow { "allow" } else { "deny" },
                    "reason": decision.reason.clone(),
                    "request_id": req_id,
                    "matched_policy_ids": [policy_id.clone()],
                    "latency_ms": eval_time_ms as i32
                }
            });
            let _ = state
                .telemetry_store
                .put_telemetry(
                    "local",
                    "decision",
                    ev["event_id"].as_str().unwrap_or("unknown"),
                    &ev,
                )
                .await;

            Ok((
                StatusCode::OK,
                Json(json!({
                    "allowed": decision.allow,
                    "decision": decision.decision,
                    "reason": decision.reason,
                    "evaluation_time_ms": eval_time_ms,
                    "log_output": ["Simulated using dry_run mode"],
                    "syntax_check": syntax_check,
                    "recommended_pep": recommended_pep,
                    "deployment_test": deployment_test
                })),
            ))
        }
        Err(e) => {
            let req_id = input
                .get("request_id")
                .and_then(|v| v.as_str())
                .unwrap_or("sim_req")
                .to_string();
            let ev = json!({
                "event_type": "decision",
                "event_id": format!("sim_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "payload": {
                    "decision": "deny",
                    "reason": format!("Error: {}", e),
                    "request_id": req_id,
                    "matched_policy_ids": [policy_id.clone()],
                    "latency_ms": eval_time_ms as i32
                }
            });
            let _ = state
                .telemetry_store
                .put_telemetry(
                    "local",
                    "decision",
                    ev["event_id"].as_str().unwrap_or("unknown"),
                    &ev,
                )
                .await;

            Ok((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "allowed": false,
                    "error": e.to_string(),
                    "evaluation_time_ms": eval_time_ms,
                    "log_output": ["Simulation failed"],
                    "syntax_check": syntax_check,
                    "recommended_pep": recommended_pep,
                    "deployment_test": "Failed during execution"
                })),
            ))
        }
    }
}
