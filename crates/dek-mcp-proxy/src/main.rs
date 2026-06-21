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

use dek_mcp_normalizer::{http::HttpTransportAdapter, TransportAdapter};
use dek_openfga::OpenFgaAdapter;
use dek_policy_router::PolicyRouter;
use dek_wasm_host::WasmPluginHost;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

mod state;
use state::AppState;

use dek_activation::snapshot::{DekMetadata, RuntimeSnapshot};
use dek_config::{BootstrapConfig, DekConfig};

#[tokio::main]
async fn main() -> Result<()> {
    #[allow(clippy::print_stderr)]
    {
        dek_config::logging::init_logging("dek-mcp-proxy").unwrap_or_else(|e| {
            eprintln!("Failed to initialize logging: {}", e);
        });
    }
    metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .map_err(|e| anyhow::anyhow!("Metrics error: {}", e))?;
    info!("Starting Pollen DEK MCP Proxy...");

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
                Box::new(dek_plugin_host::EvaluatorAdapter::new(Arc::new(adapter))),
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
    let telemetry = dek_telemetry::CloudTelemetrySink::new(
        "https://telemetry.pollen-cloud.internal",
        &bootstrap.mtls,
        None,
        &telemetry_db.to_string_lossy(),
        None,
        bootstrap.tenant_id.clone().unwrap_or_else(|| "default".into()),
        bootstrap.device_id.clone(),
    )
    .ok();

    if let Some(ref tel) = telemetry {
        tel.set_enterprise_profile(initial_prof);
    }

    let state = AppState::new(HttpTransportAdapter, initial_snapshot, telemetry, admission);

    // Start background file watcher for hot-reloading
    let state_clone = state.clone();
    tokio::spawn(async move {
        use notify::event::ModifyKind;
        use notify::{EventKind, RecursiveMode, Watcher};
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let mut watcher = match notify::recommended_watcher(move |res| {
            if let Ok(event) = res {
                let _ = tx.send(event);
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
    policy_input["principal"] = json!(principal);
    policy_input["resource"] = json!("mcp_tool");

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
            resource_id: "*".into(),
            uri: None,
        },
        context: policy_input.clone(),
        input_hash: "mock_hash".into(),
    };

    let decision_input = serde_json::to_value(&decision_req).unwrap_or(policy_input);

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
                obligations: vec![],
                effects: decision.effects.clone(),
                policy_bundle_id: "bundle".into(),
                policy_bundle_version: "v1".into(),
                evaluator_results: vec![],
                latency_ms: start_time.elapsed().as_millis() as i64,
            };

            let mut require_approval = false;
            let mut require_mfa = false;
            let mut compliance_tags = vec![];

            for ob in &decision.obligations {
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
                }
            }
            compliance_tags.sort();
            compliance_tags.dedup();

            if let Some(telemetry) = &state.telemetry {
                let event = json!({
                    "schema_version": "1.0",
                    "event_id": uuid::Uuid::new_v4().to_string(),
                    "event_type": "decision_log",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "tenant_id": final_tenant_id.clone(),
                    "workspace_id": "default",
                    "environment_id": "local",
                    "device_id": metadata.device_id.clone(),
                    "redaction_applied": true,
                    "compliance_tags": if compliance_tags.is_empty() { serde_json::Value::Null } else { json!(compliance_tags) },
                    "payload": {
                        "decision_id": decision_req.decision_id.clone(),
                        "request_id": decision_req.request_id.clone(),
                        "trace_id": decision_req.request_id.clone(),
                        "decision": if decision.allow { "allow" } else { "deny" },
                        "reason": decision.reason.clone(),
                        "matched_policy_ids": [],
                        "matched_route_id": serde_json::Value::Null,
                        "adapter_results": [],
                        "obligations": [],
                        "latency_ms": 0,
                        "principal": principal.clone(),
                        "tool": normalized.tool_name.clone().unwrap_or_default(),
                        "method": normalized.request_type.clone()
                    }
                });
                telemetry.emit_async(event, dek_telemetry::spooler::Priority::Normal);
            }

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
                let final_response = json!({
                    "status": "allowed",
                    "message": "The request passed the PEP evaluation.",
                    "decision": response
                });

                // Apply PII redaction plugin if required
                if let Ok(redacted_bytes) = snapshot
                    .plugin_host
                    .invoke(
                        "default:pii-redactor:1.0:dev",
                        uuid::Uuid::new_v4().to_string(),
                        final_response.to_string().as_bytes(),
                    )
                    .await
                {
                    info!("Applied PII redaction plugin successfully.");
                    if let Ok(redacted_val) =
                        serde_json::from_slice::<serde_json::Value>(&redacted_bytes)
                    {
                        (StatusCode::OK, Json(redacted_val)).into_response()
                    } else {
                        (StatusCode::OK, Json(final_response)).into_response()
                    }
                } else {
                    (StatusCode::OK, Json(final_response)).into_response()
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

async fn handle_filter_response(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Response {
    let snapshot = state.snapshot.load();
    // Apply redaction plugin
    if let Ok(redacted_bytes) = snapshot
        .plugin_host
        .invoke(
            "default:pii-redactor:1.0:dev",
            uuid::Uuid::new_v4().to_string(),
            payload.to_string().as_bytes(),
        )
        .await
    {
        if let Ok(redacted_val) = serde_json::from_slice::<serde_json::Value>(&redacted_bytes) {
            tracing::info!("Applied PII redaction successfully.");
            return (StatusCode::OK, Json(redacted_val)).into_response();
        }
    }

    (StatusCode::OK, Json(payload)).into_response()
}
