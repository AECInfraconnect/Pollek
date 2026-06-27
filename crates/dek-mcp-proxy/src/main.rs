// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::unwrap_used, clippy::expect_used)]
use anyhow::Result;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router as AxumRouter,
};

use dek_guard_pipeline::{GuardAction, GuardOutcome};
use dek_mcp_normalizer::{http::HttpTransportAdapter, TransportAdapter};
use dek_openfga::OpenFgaAdapter;
use dek_policy_router::PolicyRouter;
use dek_wasm_host::WasmPluginHost;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

// helper: extract arguments from JSON-RPC payload of MCP tools/call
fn extract_tool_params(normalized: &dek_mcp_normalizer::NormalizedMcpEvent) -> serde_json::Value {
    normalized
        .payload
        .get("params")
        .and_then(|p| p.get("arguments"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    const RESPONSE_CORPUS: &str = include_str!("../tests/corpus/response.jsonl");

    #[derive(Debug, Deserialize)]
    struct ResponseCorpusCase {
        id: String,
        text: String,
        expected_marker: String,
        gap: String,
        status: String,
    }

    #[test]
    fn test_extract_tool_params() {
        let mut payload = serde_json::Map::new();
        let mut params = serde_json::Map::new();
        params.insert("arguments".to_string(), json!({"key": "value"}));
        payload.insert("params".to_string(), Value::Object(params));
        let event = dek_mcp_normalizer::NormalizedMcpEvent {
            event_id: "test".into(),
            transport: dek_mcp_normalizer::TransportType::Http,
            direction: dek_mcp_normalizer::MessageDirection::Request,
            request_type: "mcp.tools.call".into(),
            jsonrpc_id: None,
            tenant_id: "test".into(),
            device_id: "test".into(),
            spiffe_id: None,
            user_id: None,
            agent_id: None,
            server_id: None,
            tool_name: Some("test_tool".into()),
            resource_uri: None,
            prompt_name: None,
            payload: Value::Object(payload),
            session: json!({}),
            runtime: json!({}),
        };
        assert_eq!(extract_tool_params(&event), json!({"key": "value"}));
    }

    #[test]
    fn merge_obligations_keeps_guard_obligations_enforceable() {
        let merged = merge_obligation_kinds(
            vec!["redact_content".to_string(), "require_approval".to_string()],
            vec!["redact_content".to_string(), "step_up_mfa".to_string()],
        );

        assert_eq!(
            merged,
            vec![
                "redact_content".to_string(),
                "require_approval".to_string(),
                "step_up_mfa".to_string()
            ]
        );
    }

    #[test]
    fn response_payload_pii_is_redacted_from_golden_corpus() -> Result<(), serde_json::Error> {
        for line in RESPONSE_CORPUS
            .lines()
            .filter(|line| !line.trim().is_empty())
        {
            let case: ResponseCorpusCase = serde_json::from_str(line)?;
            if case.status != "active" {
                continue;
            }
            let response_payload = json!({
                "tool_result": case.text,
                "decision": {
                    "allow": true,
                    "reason": "OK"
                }
            });

            let (redacted, changed) = redact_pii_with_native_plugin(response_payload);
            let rendered = redacted.to_string();

            assert!(case.id.starts_with("rt-pr3-"));
            assert_eq!(case.gap, "G-01");
            assert!(changed);
            assert!(rendered.contains(&case.expected_marker));
            assert!(!rendered.contains("alice@example.com"));
            assert!(rendered.contains("\"decision\""));
        }
        Ok(())
    }

    #[test]
    fn redact_content_transform_changes_response_payload() {
        let response_payload = json!({
            "tool_result": "tool echoed sk-test-token-value"
        });

        let redacted = redact_guarded_value(response_payload);
        let rendered = redacted.to_string();

        assert!(rendered.contains("[REDACTED_BY_POLLEK_OUTPUT_GUARD]"));
        assert!(!rendered.contains("sk-test-token-value"));
    }

    #[test]
    fn response_filter_prefers_pipeline_payload_for_spotlighting() {
        let mut guard = GuardOutcome::allow();
        guard.action = GuardAction::Redact;
        guard.redacted_payload = Some(json!({
            "source_type": "tool",
            "content": format!(
                "{}\nretrieved evidence\n{}",
                dek_guard_pipeline::spotlight::UNTRUSTED_DATA_BEGIN,
                dek_guard_pipeline::spotlight::UNTRUSTED_DATA_END
            )
        }));

        let (filtered_payload, reasons, redaction_applied) =
            apply_guard_payload_transform(json!({"content": "retrieved evidence"}), &guard, false);
        let rendered = filtered_payload.to_string();

        assert!(redaction_applied);
        assert_eq!(reasons, vec!["spotlight_untrusted_data".to_string()]);
        assert!(rendered.contains(dek_guard_pipeline::spotlight::UNTRUSTED_DATA_BEGIN));
        assert!(!rendered.contains("[REDACTED_BY_POLLEK_OUTPUT_GUARD]"));
    }
}

mod state;
use state::AppState;

mod reputation;
pub use reputation::{ReputationRegistry, ReputationStatus};

use dek_activation::snapshot::{DekMetadata, RuntimeSnapshot};
use dek_config::{BootstrapConfig, DekConfig};

mod panic_guard;

#[tokio::main]
async fn main() -> Result<()> {
    panic_guard::install_panic_hook();
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .map_err(|_| ())
            .ok();
    }
    #[allow(clippy::print_stderr)]
    {
        dek_config::logging::init_logging("dek-mcp-proxy").unwrap_or_else(|e| {
            eprintln!("Failed to initialize logging: {}", e);
        });
    }
    metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .map_err(|e| anyhow::anyhow!("Metrics error: {}", e))?;
    info!("Starting Pollek DEK MCP Proxy...");

    let bootstrap = BootstrapConfig::load_or_default("bootstrap.json")?;
    let device_id = bootstrap.device_id.clone();

    // Attempt to fetch DekConfig to get tenant_id. Default if unreachable.
    let tenant_id = match DekConfig::fetch_from_cloud(&bootstrap, "https://127.0.0.1:43891").await {
        Ok(cfg) => cfg.tenant_id,
        Err(e) => {
            warn!(
                "Failed to fetch DekConfig from cloud, defaulting tenant. Error: {}",
                e
            );
            "default-tenant".to_string()
        }
    };

    // 1. Load initial config (if available) or fallback
    let mut router = PolicyRouter::new();

    // Initialize metadata
    let mut initial_metadata = DekMetadata {
        tenant_id: tenant_id.clone(),
        device_id: device_id.clone(),
        spiffe_id: None,
        jwt_public_key_pem: None,
        jwks: None,
        issuer_url: None,
        audience: None,
        enterprise_profile: dek_config::EnterpriseProfile::default(),
    };

    let mut scale_config = dek_config::ScaleConfig::default();
    // Attempt to load from staged bundle first
    let bundle_path_buf = dek_config::paths::get_active_bundle_path();
    let staged_path = std::path::Path::new(&bundle_path_buf);
    if staged_path.exists() {
        if let Ok(content) = std::fs::read_to_string(staged_path) {
            if let Ok(payload) = serde_json::from_str::<Value>(&content) {
                info!("Loaded initial configuration from staged active_bundle.json");
                dek_router_builder::load_router_config(&mut router, &payload);
                if let Some(t) = payload.get("tenant_id").and_then(|v| v.as_str()) {
                    initial_metadata.tenant_id = t.to_string();
                }
                if let Some(s) = payload.get("spiffe_id").and_then(|v| v.as_str()) {
                    initial_metadata.spiffe_id = Some(s.to_string());
                }
                if let Some(jwt_cfg) = payload.get("jwt_config") {
                    if let Some(pem) = jwt_cfg.get("public_key_pem").and_then(|v| v.as_str()) {
                        initial_metadata.jwt_public_key_pem = Some(pem.to_string());
                    }
                    if let Some(jwks_val) = jwt_cfg.get("jwks") {
                        if let Ok(jwks) = serde_json::from_value(jwks_val.clone()) {
                            initial_metadata.jwks = Some(jwks);
                        }
                    }
                    if let Some(issuer) = jwt_cfg.get("issuer_url").and_then(|v| v.as_str()) {
                        initial_metadata.issuer_url = Some(issuer.to_string());
                    }
                    if let Some(aud_val) = jwt_cfg.get("audience") {
                        if let Ok(aud) = serde_json::from_value(aud_val.clone()) {
                            initial_metadata.audience = Some(aud);
                        }
                    }
                }
                if let Some(prof_val) = payload.get("enterprise_profile") {
                    if let Ok(prof) = serde_json::from_value(prof_val.clone()) {
                        initial_metadata.enterprise_profile = prof;
                    }
                }
                if let Some(scale_val) = payload.get("scale") {
                    if let Ok(scale) = serde_json::from_value(scale_val.clone()) {
                        scale_config = scale;
                    }
                }
            }
        }
    } else {
        // Fallback defaults if no policy config
        if let Ok(adapter) = OpenFgaAdapter::new("http://localhost:8080", "store_123", None) {
            router.register_evaluator(
                "openfga",
                Arc::new(dek_plugin_host::EvaluatorAdapter::new(Arc::new(adapter))),
            );
        }
    }

    let admission = dek_resilience::admission::AdmissionControl::new(
        scale_config.max_concurrent,
        scale_config.max_concurrent_per_tenant,
    );

    // Determine plugin paths
    let mut plugin_paths = std::collections::HashMap::new();

    // Resolve plugins path via standard installation directory or env var
    let base_dir = dek_config::paths::get_plugin_dir()
        .to_string_lossy()
        .into_owned();

    let paths_to_try = vec![
        format!("{}/pii_redactor.wasm", base_dir),
        "target/wasm32-wasip1/release/pii_redactor.wasm".to_string(),
        "target/wasm32-wasip1/debug/pii_redactor.wasm".to_string(),
    ];

    for p in paths_to_try {
        if std::path::Path::new(&p).exists() {
            plugin_paths.insert("pii-redactor".to_string(), p.to_string());
            break;
        }
    }

    let plugin_host = Arc::new(WasmPluginHost::new(
        dek_wasm_host::WasmHostConfig::default(),
    )?);
    for (name, p) in plugin_paths {
        if let Ok(bytes) = std::fs::read(&p) {
            let key = dek_wasm_host::PluginKey {
                tenant_id: "default".into(),
                plugin_id: name.clone(),
                version: "1.0".into(),
                wasm_sha256: "dev".into(),
                abi_version: "v1".into(),
            };
            let _ = plugin_host.load_plugin(key, &bytes).await;
        }
    }
    let initial_prof = initial_metadata.enterprise_profile.clone();
    let initial_snapshot = RuntimeSnapshot::new(
        0,
        "initial".into(),
        0,
        Arc::new(router),
        initial_metadata,
        plugin_host.clone(),
    );

    let telemetry_db = dek_config::paths::get_data_dir().join("telemetry.db");
    let observer_db = dek_config::paths::get_data_dir().join("observer.db");

    let observer_store =
        dek_agent_observer::ingest::SqliteObservationStore::new(&observer_db.to_string_lossy())
            .unwrap_or_else(|e| {
                tracing::error!("failed to init observer db: {}", e);
                std::process::exit(1);
            });
    let observer = std::sync::Arc::new(observer_store);

    let telemetry = dek_telemetry::CloudTelemetrySink::new(
        "https://telemetry.pollek-cloud.internal",
        &bootstrap.mtls,
        None,
        &telemetry_db.to_string_lossy(),
        bootstrap.local_api_token.clone(),
        std::sync::Arc::new(dek_secure_spool::Spool::default()),
    )
    .await
    .ok();

    if let Some(ref tel) = telemetry {
        tel.set_enterprise_profile(initial_prof);
    }

    let state = AppState::new(
        HttpTransportAdapter,
        initial_snapshot,
        telemetry,
        admission,
        Arc::new(dek_resilience::rate_limit::RateLimiter::new(100.0, 10.0)),
        observer,
    );

    // Start background file watcher for hot-reloading
    let state_clone = state.clone();
    tokio::spawn(async move {
        use notify::event::ModifyKind;
        use notify::{EventKind, RecursiveMode, Watcher};
        let (tx, mut rx) = tokio::sync::mpsc::channel(1000);

        let mut watcher = match notify::recommended_watcher(move |res| {
            if let Ok(event) = res {
                let _ = tx.try_send(event);
            }
        }) {
            Ok(w) => w,
            Err(e) => {
                error!("Failed to initialize file watcher: {}", e);
                return;
            }
        };

        let bundle_path_clone = bundle_path_buf.clone();
        let staged_path_local = std::path::Path::new(&bundle_path_clone);
        let parent_dir = staged_path_local
            .parent()
            .unwrap_or(std::path::Path::new("."));
        if let Err(e) = watcher.watch(parent_dir, RecursiveMode::NonRecursive) {
            error!("Failed to watch target directory: {}", e);
            return;
        }

        info!(
            "Started background file watcher for hot-reloading on {}",
            staged_path_local.display()
        );

        while let Some(event) = rx.recv().await {
            match event.kind {
                EventKind::Modify(ModifyKind::Data(_))
                | EventKind::Modify(ModifyKind::Any)
                | EventKind::Create(_) => {
                    let path = event.paths.first();
                    if let Some(p) = path {
                        let is_active = p.ends_with("active_bundle.json");
                        let is_shadow = p.ends_with("shadow_bundle.json");

                        if is_active || is_shadow {
                            let bundle_type = if is_active { "active" } else { "shadow" };
                            info!(
                                "Detected change in {}. Attempting hot-reload...",
                                p.display()
                            );

                            if let Ok(content) = std::fs::read_to_string(p) {
                                if let Ok(payload) = serde_json::from_str::<Value>(&content) {
                                    let mut new_router = PolicyRouter::new();
                                    // Apply dynamic routing configuration securely
                                    dek_router_builder::load_router_config(
                                        &mut new_router,
                                        &payload,
                                    );

                                    let old_snapshot = state_clone.snapshot.load();
                                    let mut metadata_clone = old_snapshot.metadata.clone();
                                    if let Some(t) =
                                        payload.get("tenant_id").and_then(|v| v.as_str())
                                    {
                                        metadata_clone.tenant_id = t.to_string();
                                    }
                                    if let Some(s) =
                                        payload.get("spiffe_id").and_then(|v| v.as_str())
                                    {
                                        metadata_clone.spiffe_id = Some(s.to_string());
                                    }
                                    if let Some(jwt_cfg) = payload.get("jwt_config") {
                                        if let Some(pem) =
                                            jwt_cfg.get("public_key_pem").and_then(|v| v.as_str())
                                        {
                                            metadata_clone.jwt_public_key_pem =
                                                Some(pem.to_string());
                                        }
                                        if let Some(jwks_val) = jwt_cfg.get("jwks") {
                                            if let Ok(jwks) =
                                                serde_json::from_value(jwks_val.clone())
                                            {
                                                metadata_clone.jwks = Some(jwks);
                                            }
                                        }
                                        if let Some(issuer) =
                                            jwt_cfg.get("issuer_url").and_then(|v| v.as_str())
                                        {
                                            metadata_clone.issuer_url = Some(issuer.to_string());
                                        }
                                        if let Some(aud_val) = jwt_cfg.get("audience") {
                                            if let Ok(aud) = serde_json::from_value(aud_val.clone())
                                            {
                                                metadata_clone.audience = Some(aud);
                                            }
                                        }
                                    }
                                    if let Some(prof_val) = payload.get("enterprise_profile") {
                                        if let Ok(prof) = serde_json::from_value(prof_val.clone()) {
                                            metadata_clone.enterprise_profile = prof;
                                            if let Some(ref tel) = state_clone.telemetry {
                                                tel.set_enterprise_profile(
                                                    metadata_clone.enterprise_profile.clone(),
                                                );
                                            }
                                        }
                                    }

                                    let new_snapshot = RuntimeSnapshot::new(
                                        0,
                                        "hot-reload".into(),
                                        0,
                                        Arc::new(new_router),
                                        metadata_clone,
                                        old_snapshot.plugin_host.clone(),
                                    );

                                    if is_active {
                                        // Clear cache of the old router before replacing
                                        old_snapshot.router.clear_caches().await;
                                        state_clone.reload(new_snapshot);
                                        info!("Hot-reloaded active policies and metadata from disk successfully!");
                                    } else {
                                        state_clone.reload_shadow(new_snapshot);
                                        info!("Hot-reloaded shadow policies and metadata from disk successfully!");
                                    }
                                } else {
                                    error!("Failed to parse payload for {}", bundle_type);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    });

    let app = AxumRouter::new()
        .route("/mcp", post(handle_mcp_request))
        // Sidecar PEP APIs
        .route("/v1/authorize", post(handle_authorize))
        .route("/v1/evaluate", post(handle_authorize)) // Alias for now
        .route("/v1/filter/request", post(handle_filter_request))
        .route("/v1/filter/response", post(handle_filter_response))
        .route("/healthz", axum::routing::get(|| async { "OK" }))
        .route("/readyz", axum::routing::get(|| async { "READY" }))
        // Layer 2 Opt-in Proxy Redirect Handlers
        .fallback(handle_forward_proxy)
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:43890").await?;
    info!("MCP Proxy + Forward Proxy listening on http://127.0.0.1:43890");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("MCP Proxy shut down gracefully.");
    Ok(())
}

async fn handle_forward_proxy(
    State(_state): State<Arc<AppState>>,
    req: axum::extract::Request,
) -> Response {
    use axum::http::Method;
    if req.method() == Method::CONNECT {
        let uri = req.uri().clone();
        let host = uri.host().unwrap_or("").to_string();
        let port = uri.port_u16().unwrap_or(443);
        let target = format!("{}:{}", host, port);

        tokio::spawn(async move {
            match hyper::upgrade::on(req).await {
                Ok(upgraded) => {
                    let mut upgraded = hyper_util::rt::TokioIo::new(upgraded);
                    match tokio::net::TcpStream::connect(&target).await {
                        Ok(mut server) => {
                            let _ = tokio::io::copy_bidirectional(&mut upgraded, &mut server).await;
                        }
                        Err(e) => warn!("Failed to connect to {}: {}", target, e),
                    }
                }
                Err(e) => warn!("Upgrade execution error: {}", e),
            }
        });
        StatusCode::OK.into_response()
    } else {
        (
            StatusCode::BAD_GATEWAY,
            "Only HTTP CONNECT is implemented for Forward Proxy",
        )
            .into_response()
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            warn!("Failed to install Ctrl+C handler: {}", e);
        }
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("Shutdown signal received, starting graceful shutdown");
}

async fn handle_mcp_request(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    info!("Intercepted MCP Request: {}", payload);

    let auth_header = headers.get("Authorization").and_then(|h| h.to_str().ok());

    let snapshot = state.snapshot.load();
    let metadata = &snapshot.metadata;
    let verifier = &snapshot.verifier;

    let token = match dek_auth::extract_bearer(auth_header) {
        Ok(t) => t,
        Err(_) => {
            warn!("missing bearer token");
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Missing bearer token" })),
            )
                .into_response();
        }
    };

    let identity = match verifier.verify(token) {
        Ok(id) => id,
        Err(dek_auth::AuthError::NoKeyConfigured) => {
            warn!("auth not configured");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Auth not configured" })),
            )
                .into_response();
        }
        Err(e) => {
            warn!("jwt verification failed: {}", e);
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Invalid token" })),
            )
                .into_response();
        }
    };

    let principal = identity.principal;
    let jwt_tenant_id = identity.tenant_id;

    let final_tenant_id = jwt_tenant_id.unwrap_or(metadata.tenant_id.clone());

    // Phase 4: Admission Control (Backpressure)
    let _permit = match state.admission.try_admit(&final_tenant_id) {
        Some(p) => p,
        None => {
            metrics::counter!("dek_proxy_requests_total", "decision" => "deny", "reason" => "overloaded").increment(1);
            tracing::warn!(tenant = %final_tenant_id, "request denied by admission control (overloaded)");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "allow": false, "decision": "deny", "reason": "overloaded_backpressure" }))
            ).into_response();
        }
    };

    // Normalize Event
    let normalized = match state.http_adapter.normalize_request(
        payload.clone(),
        &final_tenant_id,
        &metadata.device_id,
        metadata.spiffe_id.as_deref(),
        Some(&principal),
    ) {
        Ok(n) => n,
        Err(_) => {
            error!("Failed to normalize request");
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Failed to normalize request" })),
            )
                .into_response();
        }
    };

    let mut policy_input = serde_json::to_value(&normalized).unwrap_or(json!({}));
    // Provide backwards compatibility for existing mock PDPs
    policy_input["action"] = json!(normalized
        .tool_name
        .clone()
        .unwrap_or(normalized.request_type.clone()));
    policy_input["principal"] = json!(principal.clone());

    let tool_params = extract_tool_params(&normalized);
    let request_guard = state.guard_pipeline.scan_request(&tool_params);
    let mut extra_obligations = Vec::new();
    match request_guard.action {
        GuardAction::Deny => {
            metrics::counter!("dek_proxy_requests_total", "decision" => "deny", "reason" => "guard_pipeline").increment(1);
            tracing::warn!(
                tenant = %final_tenant_id,
                agent = %normalized.agent_id.as_deref().unwrap_or("unknown"),
                "request denied by guard pipeline"
            );
            let body = guard_json_rpc_error(
                payload.get("id").unwrap_or(&serde_json::Value::Null),
                "Access Denied: Prompt injection or policy override detected",
                &request_guard,
            );
            return (axum::http::StatusCode::FORBIDDEN, axum::Json(body)).into_response();
        }
        GuardAction::Redact => {
            extra_obligations.push("redact_content".to_string());
        }
        GuardAction::Allow => {}
    }
    let guarded_tool_params = request_guard
        .redacted_payload
        .clone()
        .unwrap_or_else(|| tool_params.clone());
    policy_input["params"] = guarded_tool_params.clone();
    policy_input["tool"] = serde_json::json!(normalized.tool_name.clone().unwrap_or_default());
    let decision_req = dek_decision::DecisionRequestV1 {
        decision_id: uuid::Uuid::new_v4().to_string(),
        request_id: uuid::Uuid::new_v4().to_string(),
        trace_id: None,
        tenant_id: final_tenant_id.clone(),
        device_id: metadata.device_id.clone(),
        principal: dek_decision::Principal {
            id: principal.clone(),
            roles: vec![],
        },
        agent: None,
        action: normalized
            .tool_name
            .clone()
            .unwrap_or(normalized.request_type.clone()),
        resource: dek_decision::ResourceRef {
            resource_type: "mcp_tool".into(),
            resource_id: normalized.tool_name.clone().unwrap_or_else(|| "*".into()),
            uri: normalized.resource_uri.clone(),
        },
        context: policy_input.clone(),
        input_hash: {
            use sha2::Digest;
            let bytes = serde_jcs::to_vec(&policy_input).unwrap_or_default();
            hex::encode(sha2::Sha256::digest(&bytes))
        },
    };

    let decision_input = serde_json::to_value(&decision_req).unwrap_or(policy_input.clone());

    let rate_key = format!(
        "{}:{}",
        normalized.agent_id.as_deref().unwrap_or("unknown"),
        normalized.tool_name.as_deref().unwrap_or("unknown")
    );

    // Compute real trust score via observer baseline
    let agent_id_str = normalized
        .agent_id
        .clone()
        .unwrap_or_else(|| "unknown".into());
    let trust_score = state
        .observer
        .update_baseline(&agent_id_str)
        .await
        .unwrap_or_else(|_| dek_agent_observer::trust::TrustScore {
            agent_id: agent_id_str.clone(),
            score: 1.0,
            reasons: vec![],
        });

    match dek_agent_observer::trust::enforce_trust(&trust_score) {
        dek_agent_observer::trust::TrustAction::KillSwitch => {
            metrics::counter!("dek_proxy_requests_total", "decision" => "deny", "reason" => "kill_switch").increment(1);
            tracing::error!(agent = %trust_score.agent_id, "request denied by trust kill-switch");
            let body = serde_json::json!({
                "jsonrpc": "2.0",
                "id": payload.get("id").unwrap_or(&serde_json::Value::Null),
                "error": { "code": -32000, "message": "Access Denied: Agent trust score too low" }
            });
            return (axum::http::StatusCode::FORBIDDEN, axum::Json(body)).into_response();
        }
        dek_agent_observer::trust::TrustAction::RequireApproval => {
            tracing::info!(agent = %trust_score.agent_id, "agent requires human approval");
            extra_obligations.push("require_approval".to_string());
        }
        dek_agent_observer::trust::TrustAction::Normal => {}
    }
    if state.rate_limiter.check(&rate_key) == dek_resilience::rate_limit::RateDecision::Throttled {
        metrics::counter!("dek_proxy_requests_total", "decision" => "deny", "reason" => "rate_limit").increment(1);
        tracing::warn!(tenant = %final_tenant_id, agent = %normalized.agent_id.as_deref().unwrap_or("unknown"), "request denied by rate limiter");
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": payload.get("id").unwrap_or(&serde_json::Value::Null),
            "error": {
                "code": -32000,
                "message": "Access Denied: Rate limit exceeded"
            }
        });
        return (axum::http::StatusCode::TOO_MANY_REQUESTS, axum::Json(body)).into_response();
    }

    let start_time = std::time::Instant::now();
    // Evaluate against the Adaptive Policy Pipeline
    // โ”€โ”€ Phase 1: fail-safe freshness gate โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€
    // เธ–เนเธฒ policy bundle stale/absent โ’ DENY เธ—เธฑเธเธ—เธต เนเธ”เธขเนเธกเนเน€เธฃเธตเธขเธ PDP (fail-closed).
    if let Some(reason) = dek_policy_syncer::strict_deny_reason() {
        metrics::counter!("dek_proxy_requests_total", "decision" => "deny").increment(1);
        tracing::warn!(%reason, "request denied by freshness gate (strict-deny)");
        // เธเธทเธ decision deny เนเธเธฃเธนเธเนเธเธเน€เธ”เธตเธขเธงเธเธฑเธ path เธเธเธ•เธด
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": payload.get("id").unwrap_or(&serde_json::Value::Null),
            "error": {
                "code": -32000,
                "message": format!("Access Denied: policy_stale_failsafe: {}", reason)
            }
        });
        return (axum::http::StatusCode::FORBIDDEN, axum::Json(body)).into_response();
    }
    // โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€

    let decision_result = snapshot.router.authorize(decision_input.clone()).await;

    let duration = start_time.elapsed().as_secs_f64();
    metrics::histogram!("dek_proxy_request_duration_seconds").record(duration);

    // Shadow evaluation if shadow_snapshot exists
    if let Some(shadow_snap) = state.shadow_snapshot.load_full() {
        let shadow_input = decision_input.clone();
        tokio::spawn(async move {
            match shadow_snap.router.authorize(shadow_input).await {
                Ok(shadow_decision) => {
                    info!("Shadow Pipeline Decision: {:?}", shadow_decision);
                    // Emit telemetry or compare with Active
                }
                Err(e) => {
                    warn!("Shadow Policy Evaluation Error: {}", e);
                }
            }
        });
    }

    match decision_result {
        Ok(decision) => {
            let enforced_obligations =
                merge_obligation_kinds(extra_obligations.clone(), decision.obligations.clone());
            if decision.allow {
                metrics::counter!("dek_proxy_requests_total", "decision" => "allow").increment(1);
            } else {
                metrics::counter!("dek_proxy_requests_total", "decision" => "deny").increment(1);
            }
            info!("Final Pipeline Decision: {:?}", decision);

            let response = dek_decision::DecisionResponse {
                decision_id: uuid::Uuid::new_v4().to_string(),
                allow: decision.allow,
                reason_code: if decision.allow {
                    "OK".into()
                } else {
                    "DENY".into()
                },
                reason: decision.reason.clone(),
                obligations: {
                    let mut obs = Vec::new();
                    for o in enforced_obligations.clone() {
                        obs.push(dek_decision::Obligation {
                            kind: o,
                            parameters: serde_json::json!({}),
                        });
                    }
                    obs
                },
                effects: decision.effects.clone(),
                policy_bundle_id: "bundle".into(),
                policy_bundle_version: "v1".into(),
                evaluator_results: vec![],
                latency_ms: start_time.elapsed().as_millis() as i64,
            };

            let mut require_approval = false;
            let mut require_mfa = false;
            let mut require_sandbox = false;
            let mut redact_content = false;
            let mut compliance_tags = vec![];

            for ob in &enforced_obligations {
                let ob_type = ob.as_str();
                metrics::counter!("dek_obligation_enforced_total", "type" => ob_type.to_string())
                    .increment(1);
                if ob_type == "require_approval" {
                    require_approval = true;
                    compliance_tags.push("SOC2-CC6.1".to_string());
                } else if ob_type == "step_up_mfa" {
                    require_mfa = true;
                    compliance_tags.push("PCI-DSS-4.0".to_string());
                    compliance_tags.push("HIPAA-164.312(a)(1)".to_string());
                } else if ob_type == "require_sandbox" {
                    require_sandbox = true;
                    compliance_tags.push("ASI05".to_string());
                } else if ob_type == "redact_content" {
                    redact_content = true;
                    compliance_tags.push("OWASP-LLM01".to_string());
                }
            }
            compliance_tags.sort();
            compliance_tags.dedup();

            let action = if normalized.request_type.is_empty() {
                "tools/call".into()
            } else {
                normalized.request_type.clone()
            };
            let is_resource = action == "resources/read" || action == "resources/list";

            let obs = dek_agent_observer::model::AgentObservationEvent {
                event_id: uuid::Uuid::new_v4().to_string(),
                tenant_id: final_tenant_id.clone(),
                trace_id: decision_req.request_id.clone(),
                agent_id: normalized.agent_id.clone(),
                shadow_candidate_id: None,
                tool_id: if is_resource {
                    None
                } else {
                    normalized.tool_name.clone()
                },
                resource_id: None,
                surface: "mcp".into(),
                action: action.clone(),
                pep_type: Some("mcp_proxy".into()),
                risk_level: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                payload_json: "{}".into(),
                token_usage: None,
                browser_scope: None,
                event_kind: if is_resource {
                    dek_agent_observer::model::EventKind::ResourceAccess
                } else {
                    dek_agent_observer::model::EventKind::ToolCall
                },
                decision: Some(dek_agent_observer::model::DecisionInfo {
                    allow: decision.allow,
                    reason_code: if decision.allow {
                        "OK".into()
                    } else {
                        "DENY".into()
                    },
                    obligations: enforced_obligations.clone(),
                    matched_policy_ids: vec![],
                    compliance_tags: compliance_tags.clone(),
                    pep_plane: Some("McpProxy".into()),
                    enforced_for_real: Some(true),
                    status_badge: Some(if decision.allow {
                        "Ok".into()
                    } else {
                        "Denied".into()
                    }),
                    message_th: Some(if decision.allow {
                        "อนุญาต".into()
                    } else {
                        "ปฏิเสธ".into()
                    }),
                }),
                tool_call: if is_resource {
                    None
                } else {
                    Some(dek_agent_observer::model::ToolCall {
                        tool_name: normalized.tool_name.clone().unwrap_or_default(),
                        server: None,
                        args_summary: None,
                        result_status: if decision.allow {
                            "ok".into()
                        } else {
                            "denied".into()
                        },
                    })
                },
                resource_access: if is_resource {
                    Some(dek_agent_observer::model::ResourceAccess {
                        resource_type: "mcp_resource".into(),
                        target_redacted: normalized.resource_uri.clone().unwrap_or_default(),
                        bytes: None,
                        verb: "read".into(),
                    })
                } else {
                    None
                },
                latency_ms: Some(start_time.elapsed().as_millis() as i64),
                provider: None,
            };

            if let Some(telemetry) = &state.telemetry {
                let mut event_json = serde_json::to_value(&obs).unwrap_or(serde_json::json!({}));
                // ensure the spooler treats it correctly
                event_json["event_type"] = serde_json::json!("agent_observation");
                telemetry.emit_async(event_json, dek_telemetry::spooler::Priority::Normal);

                if !is_resource {
                    let tool_payload = serde_json::json!({
                        "agent_id": obs.agent_id.clone().unwrap_or_default(),
                        "agent_label": obs.agent_id.clone().unwrap_or_default(),
                        "tool_kind": "mcp_tool",
                        "tool_name": normalized.tool_name.clone().unwrap_or_default(),
                        "server": "mcp-proxy",
                        "decision": if decision.allow { "allow" } else { "deny" },
                        "enforced_for_real": true,
                        "args_redacted": "<redacted>",
                        "observed_at": chrono::Utc::now().to_rfc3339()
                    });
                    let env = serde_json::json!({
                        "schema_version": "telemetry-envelope.v1",
                        "event_id": uuid::Uuid::new_v4().to_string(),
                        "event_type": "tool_usage",
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                        "tenant_id": final_tenant_id.clone(),
                        "device_id": metadata.device_id.clone(),
                        "redaction_applied": false,
                        "payload": tool_payload
                    });
                    telemetry.emit_async(env, dek_telemetry::spooler::Priority::Normal);
                }

                if is_resource {
                    let res_payload = serde_json::json!({
                        "agent_id": obs.agent_id.clone().unwrap_or_default(),
                        "agent_label": obs.agent_id.clone().unwrap_or_default(),
                        "scope": "local",
                        "kind": "file",
                        "target_redacted": normalized.resource_uri.clone().unwrap_or_default(),
                        "target_hash": normalized.resource_uri.clone().unwrap_or_default(),
                        "mode": "read",
                        "decision": if decision.allow { "allow" } else { "deny" },
                        "enforced_for_real": true,
                        "observed_at": chrono::Utc::now().to_rfc3339()
                    });
                    let env = serde_json::json!({
                        "schema_version": "telemetry-envelope.v1",
                        "event_id": uuid::Uuid::new_v4().to_string(),
                        "event_type": "resource_access",
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                        "tenant_id": final_tenant_id.clone(),
                        "device_id": metadata.device_id.clone(),
                        "redaction_applied": false,
                        "payload": res_payload
                    });
                    telemetry.emit_async(env, dek_telemetry::spooler::Priority::Normal);
                }
            }

            let _ = state.observer.append(obs).await;

            let has_mfa = identity
                .claims
                .get("mfa")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                || identity
                    .claims
                    .get("amr")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().any(|s| s.as_str() == Some("mfa")))
                    .unwrap_or(false);

            let mfa_failed = require_mfa && !has_mfa;

            // Emit approval_required event if applicable
            if require_approval && decision.allow && !mfa_failed {
                if let Some(telemetry) = &state.telemetry {
                    let approval_event = json!({
                        "schema_version": "1.0",
                        "event_type": "enforcement.approval_required",
                        "device_id": metadata.device_id.clone(),
                        "tenant_id": final_tenant_id.clone(),
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                        "compliance_tags": if compliance_tags.is_empty() { serde_json::Value::Null } else { json!(compliance_tags) },
                        "mcp": {
                            "principal": principal.clone(),
                            "tool": normalized.tool_name.clone().unwrap_or_default(),
                            "method": normalized.request_type.clone(),
                            "request_id": decision_req.request_id.clone(),
                        }
                    });
                    telemetry.emit_async(approval_event, dek_telemetry::spooler::Priority::High);
                }
            }

            if decision.allow && !require_approval && !mfa_failed {
                if require_sandbox {
                    let mut sandbox = dek_execution_sandbox::WasmSandbox::new();
                    use dek_execution_sandbox::SandboxEnvironment;
                    let _ = sandbox.spawn(dek_execution_sandbox::SandboxConfig {
                        timeout_ms: 5000,
                        max_memory_mb: 256,
                        enable_network: false,
                    });
                    tracing::info!("Tool execution delegated to isolated sandbox");
                    let _ = sandbox.execute_tool(&normalized);
                    let _ = sandbox.terminate();
                }

                let final_response = json!({
                    "status": "allowed",
                    "message": "The request passed the PEP evaluation.",
                    "decision": response
                });

                let outcome =
                    filter_response_payload(state.as_ref(), final_response, redact_content).await;
                if outcome.action == GuardAction::Deny {
                    let body = json!({
                        "jsonrpc": "2.0",
                        "id": payload.get("id").unwrap_or(&serde_json::Value::Null),
                        "error": {
                            "code": -32000,
                            "message": format!("Access Denied: {}", outcome.reason),
                            "data": {
                                "status": "denied",
                                "reason": outcome.reason,
                                "guard": outcome.guard
                            }
                        }
                    });
                    (StatusCode::FORBIDDEN, Json(body)).into_response()
                } else {
                    (StatusCode::OK, Json(outcome.payload)).into_response()
                }
            } else {
                let (reason, code) = if mfa_failed {
                    ("step_up_mfa_required".to_string(), -32001)
                } else if require_approval {
                    ("pending_approval".to_string(), -32002)
                } else {
                    (decision.reason.clone(), -32000)
                };

                let body = json!({
                    "jsonrpc": "2.0",
                    "id": payload.get("id").unwrap_or(&serde_json::Value::Null),
                    "error": {
                        "code": code,
                        "message": format!("Access Denied: {}", reason),
                        "data": {
                            "status": "denied",
                            "reason": reason,
                            "decision": response
                        }
                    }
                });

                (StatusCode::FORBIDDEN, Json(body)).into_response()
            }
        }
        Err(e) => {
            metrics::counter!("dek_proxy_requests_total", "decision" => "error").increment(1);
            error!("Policy Evaluation Error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "status": "denied",
                    "reason": "policy_evaluation_error"
                })),
            )
                .into_response()
        }
    }
}

async fn handle_authorize(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Response {
    let snapshot = state.snapshot.load();
    let start_time = std::time::Instant::now();
    // โ”€โ”€ Phase 1: fail-safe freshness gate โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€
    if let Some(reason) = dek_policy_syncer::strict_deny_reason() {
        metrics::counter!("dek_proxy_requests_total", "decision" => "deny").increment(1);
        tracing::warn!(%reason, "request denied by freshness gate (strict-deny)");
        let body = serde_json::json!({
            "allow": false,
            "decision": "deny",
            "reason": format!("policy_stale_failsafe: {}", reason),
            "evaluator_id": "freshness_gate"
        });
        return (axum::http::StatusCode::FORBIDDEN, axum::Json(body)).into_response();
    }
    // โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€โ”€
    let decision_result = snapshot.router.authorize(payload).await;
    let duration = start_time.elapsed().as_secs_f64();
    metrics::histogram!("dek_proxy_request_duration_seconds").record(duration);

    match decision_result {
        Ok(mut decision) => {
            let mut require_approval = false;
            let mut require_mfa = false;

            for ob in &decision.obligations {
                let ob_type = ob.as_str();
                if ob_type == "require_approval" {
                    require_approval = true;
                } else if ob_type == "step_up_mfa" {
                    require_mfa = true;
                }
            }

            if require_mfa || require_approval {
                decision.allow = false;
                if require_mfa {
                    decision.reason = "step_up_mfa_required".into();
                } else if require_approval {
                    decision.reason = "pending_approval".into();
                }
            }

            if decision.allow {
                metrics::counter!("dek_proxy_requests_total", "decision" => "allow").increment(1);
                (StatusCode::OK, Json(decision)).into_response()
            } else {
                metrics::counter!("dek_proxy_requests_total", "decision" => "deny").increment(1);
                (StatusCode::FORBIDDEN, Json(decision)).into_response()
            }
        }
        Err(e) => {
            metrics::counter!("dek_proxy_requests_total", "decision" => "error").increment(1);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    }
}

async fn handle_filter_request(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Response {
    let _snapshot = state.snapshot.load();
    // In a real scenario, this would apply request-side obligations (e.g. inject headers)
    (
        StatusCode::OK,
        Json(json!({"status": "filtered", "payload": payload})),
    )
        .into_response()
}

fn redact_guarded_value(value: Value) -> Value {
    match value {
        Value::String(text) => Value::String(content_guard::redact_text(&text)),
        Value::Array(items) => Value::Array(items.into_iter().map(redact_guarded_value).collect()),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, redact_guarded_value(value)))
                .collect(),
        ),
        other => other,
    }
}

#[derive(Debug)]
struct ResponseFilterOutcome {
    action: GuardAction,
    payload: Value,
    reason: String,
    guard: Value,
}

fn merge_obligation_kinds(mut extra: Vec<String>, decision: Vec<String>) -> Vec<String> {
    for obligation in decision {
        if !extra.iter().any(|existing| existing == &obligation) {
            extra.push(obligation);
        }
    }
    extra
}

fn guard_action_label(action: GuardAction) -> &'static str {
    match action {
        GuardAction::Allow => "allow",
        GuardAction::Redact => "redact",
        GuardAction::Deny => "deny",
    }
}

fn guard_metadata(outcome: &GuardOutcome) -> Value {
    json!({
        "plugin_id": "dek.guard-pipeline",
        "action": guard_action_label(outcome.action),
        "injection_score": outcome.injection_score,
        "categories": outcome.categories,
        "normalization_steps": outcome.normalization_steps,
        "confidence": outcome.confidence,
        "findings_count": outcome.findings.len(),
    })
}

fn guard_json_rpc_error(id: &Value, message: &str, outcome: &GuardOutcome) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": -32000,
            "message": message,
            "data": {
                "guard": guard_metadata(outcome)
            }
        }
    })
}

async fn filter_response_payload(
    state: &AppState,
    payload: Value,
    force_redact: bool,
) -> ResponseFilterOutcome {
    let guard = state.guard_pipeline.scan_response(&payload);
    if guard.action == GuardAction::Deny {
        return ResponseFilterOutcome {
            action: GuardAction::Deny,
            payload,
            reason: "output_guard_blocked_risky_tool_response".to_string(),
            guard: guard_metadata(&guard),
        };
    }

    let (mut filtered_payload, mut reasons, mut redaction_applied) =
        apply_guard_payload_transform(payload, &guard, force_redact);

    let (pii_redacted_payload, pii_redacted) = apply_pii_redaction(state, filtered_payload).await;
    filtered_payload = pii_redacted_payload;
    if pii_redacted {
        redaction_applied = true;
        reasons.push("pii_redacted".to_string());
    }

    if redaction_applied {
        ResponseFilterOutcome {
            action: GuardAction::Redact,
            payload: filtered_payload,
            reason: reasons.join(","),
            guard: guard_metadata(&guard),
        }
    } else {
        ResponseFilterOutcome {
            action: GuardAction::Allow,
            payload: filtered_payload,
            reason: "allow".to_string(),
            guard: guard_metadata(&guard),
        }
    }
}

fn apply_guard_payload_transform(
    payload: Value,
    guard: &GuardOutcome,
    force_redact: bool,
) -> (Value, Vec<String>, bool) {
    let mut filtered_payload = payload;
    let mut reasons = Vec::new();
    let mut redaction_applied = false;
    let guard_payload_applied = if let Some(payload) = guard.redacted_payload.clone() {
        filtered_payload = payload;
        redaction_applied = true;
        reasons.push("spotlight_untrusted_data".to_string());
        true
    } else {
        false
    };

    if (force_redact || guard.action == GuardAction::Redact) && !guard_payload_applied {
        filtered_payload = redact_guarded_value(filtered_payload);
        redaction_applied = true;
        reasons.push("redact_content".to_string());
    }

    (filtered_payload, reasons, redaction_applied)
}

async fn apply_pii_redaction(state: &AppState, payload: Value) -> (Value, bool) {
    if let Some(redacted) = invoke_wasm_pii_redactor(state, &payload).await {
        let changed = redacted != payload;
        return (redacted, changed);
    }
    redact_pii_with_native_plugin(payload)
}

async fn invoke_wasm_pii_redactor(state: &AppState, payload: &Value) -> Option<Value> {
    let snapshot = state.snapshot.load();
    let bytes = snapshot
        .plugin_host
        .invoke(
            "default:pii-redactor:1.0:dev",
            uuid::Uuid::new_v4().to_string(),
            payload.to_string().as_bytes(),
            100_000_000,
        )
        .await
        .ok()?;
    serde_json::from_slice::<serde_json::Value>(&bytes).ok()
}

fn redact_pii_with_native_plugin(mut payload: Value) -> (Value, bool) {
    let detector = match pii_redactor_plugin::DeterministicDetector::new() {
        Ok(detector) => detector,
        Err(err) => {
            tracing::warn!("failed to initialize native pii redactor: {}", err);
            return (payload, false);
        }
    };
    let original = payload.clone();
    pii_redactor_plugin::process_json(&mut payload, &detector);
    let changed = payload != original;
    (payload, changed)
}

async fn handle_filter_response(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Response {
    let outcome = filter_response_payload(state.as_ref(), payload, false).await;
    match outcome.action {
        GuardAction::Deny => {
            metrics::counter!("dek_proxy_responses_total", "decision" => "deny", "reason" => "output_guard").increment(1);
            let body = json!({
                "status": "denied",
                "reason": outcome.reason,
                "guard": outcome.guard,
            });
            (StatusCode::FORBIDDEN, Json(body)).into_response()
        }
        GuardAction::Redact => {
            metrics::counter!("dek_proxy_responses_total", "decision" => "redact", "reason" => "output_guard").increment(1);
            (
                StatusCode::OK,
                Json(json!({
                    "status": "filtered",
                    "reason": outcome.reason,
                    "guard": outcome.guard,
                    "payload": outcome.payload
                })),
            )
                .into_response()
        }
        GuardAction::Allow => (StatusCode::OK, Json(outcome.payload)).into_response(),
    }
}
