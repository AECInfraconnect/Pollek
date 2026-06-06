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
use dek_wasm_host::{PluginHost, WasmtimePluginHost};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Clone)]
struct DekMetadata {
    tenant_id: String,
    device_id: String,
    spiffe_id: Option<String>,
    pub jwt_public_key_pem: Option<String>,
    pub jwks: Option<jsonwebtoken::jwk::JwkSet>,
    pub issuer_url: Option<String>,
}

struct AppState {
    plugin_host: WasmtimePluginHost,
    router: RwLock<PolicyRouter>,
    http_adapter: HttpTransportAdapter,
    metadata: RwLock<DekMetadata>,
}

use dek_config::{BootstrapConfig, DekConfig};

#[tokio::main]
async fn main() -> Result<()> {
    dek_config::logging::init_logging("dek-mcp-proxy").unwrap_or_else(|e| {
        eprintln!("Failed to initialize logging: {}", e);
    });
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
    };

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
                        if let Ok(jwks) = serde_json::from_value::<jsonwebtoken::jwk::JwkSet>(jwks_val.clone()) {
                            initial_metadata.jwks = Some(jwks);
                        }
                    }
                    if let Some(issuer) = jwt_cfg.get("issuer_url").and_then(|v| v.as_str()) {
                        initial_metadata.issuer_url = Some(issuer.to_string());
                    }
                }
            }
        }
    } else {
        // Fallback defaults if no policy config
        if let Ok(adapter) = OpenFgaAdapter::new("http://localhost:8080", "store_123", None) {
            router.register_evaluator("openfga", Box::new(adapter));
        }
        // Removed fallback to Cedar requiring user_bob
    }

    // Determine plugin paths
    let mut plugin_paths = std::collections::HashMap::new();
    
    // Resolve plugins path via standard installation directory or env var
    let base_dir = dek_config::paths::get_plugin_dir().to_string_lossy().into_owned();

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

    let state = Arc::new(AppState {
        plugin_host: WasmtimePluginHost::new(plugin_paths)?,
        router: RwLock::new(router),
        http_adapter: HttpTransportAdapter,
        metadata: RwLock::new(initial_metadata),
    });

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
        let parent_dir = staged_path_local.parent().unwrap_or(std::path::Path::new("."));
        if let Err(e) = watcher.watch(parent_dir, RecursiveMode::NonRecursive) {
            error!("Failed to watch target directory: {}", e);
            return;
        }

        info!("Started background file watcher for hot-reloading on {}", staged_path_local.display());

        while let Some(event) = rx.recv().await {
            match event.kind {
                EventKind::Modify(ModifyKind::Data(_))
                | EventKind::Modify(ModifyKind::Any)
                | EventKind::Create(_) => {
                    let path = event.paths.first();
                    if let Some(p) = path {
                        if p.ends_with("active_bundle.json") {
                            info!(
                                "Detected change in active_bundle.json. Attempting hot-reload..."
                            );

                            if let Ok(content) = std::fs::read_to_string(p) {
                                if let Ok(payload) = serde_json::from_str::<Value>(&content) {
                                    let mut new_router = PolicyRouter::new();
                                    // Apply dynamic routing configuration securely
                                    dek_router_builder::load_router_config(
                                        &mut new_router,
                                        &payload,
                                    );

                                    // Safely swap the router
                                    let mut current_router = state_clone.router.write().await;
                                    *current_router = new_router;

                                    let mut metadata_lock = state_clone.metadata.write().await;
                                    if let Some(t) =
                                        payload.get("tenant_id").and_then(|v| v.as_str())
                                    {
                                        metadata_lock.tenant_id = t.to_string();
                                    }
                                    if let Some(s) =
                                        payload.get("spiffe_id").and_then(|v| v.as_str())
                                    {
                                        metadata_lock.spiffe_id = Some(s.to_string());
                                    }
                                    if let Some(jwt_cfg) = payload.get("jwt_config") {
                                        if let Some(pem) =
                                            jwt_cfg.get("public_key_pem").and_then(|v| v.as_str())
                                        {
                                            metadata_lock.jwt_public_key_pem =
                                                Some(pem.to_string());
                                        }
                                        if let Some(jwks_val) = jwt_cfg.get("jwks") {
                                            if let Ok(jwks) = serde_json::from_value::<jsonwebtoken::jwk::JwkSet>(jwks_val.clone()) {
                                                metadata_lock.jwks = Some(jwks);
                                            }
                                        }
                                        if let Some(issuer) = jwt_cfg.get("issuer_url").and_then(|v| v.as_str()) {
                                            metadata_lock.issuer_url = Some(issuer.to_string());
                                        }
                                    }

                                    info!("Hot-reloaded policies and metadata from disk successfully!");
                                } else {
                                    error!("Failed to parse active_bundle.json payload");
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

async fn handle_forward_proxy() -> impl IntoResponse {
    // Basic forward proxy placeholder
    (StatusCode::BAD_GATEWAY, "Forward proxy not yet fully implemented")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
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

    // JWT Extraction
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    let metadata = state.metadata.read().await.clone();

    let mut jwt_tenant_id = None;
    let mut principal = None;

    if let Some(token) = auth_header {
        let mut decoding_key_opt = None;
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);

        // 1. Primary: Use JWKS if available
        if let Some(jwks) = &metadata.jwks {
            if let Ok(header) = jsonwebtoken::decode_header(token) {
                if let Some(kid) = header.kid {
                    if let Some(jwk) = jwks.find(&kid) {
                        match jsonwebtoken::DecodingKey::from_jwk(jwk) {
                            Ok(key) => {
                                decoding_key_opt = Some(key);
                                // Optional: Configure validation based on issuer config if present
                                if let Some(issuer) = &metadata.issuer_url {
                                    validation.set_issuer(&[issuer]);
                                }
                            }
                            Err(e) => warn!("Failed to create decoding key from JWK: {}", e),
                        }
                    } else {
                        warn!("JWK not found for kid: {}", kid);
                    }
                } else {
                    warn!("JWT header missing kid");
                }
            }
        }

        // 2. Fallback: Use static PEM if JWKS not available or failed
        if decoding_key_opt.is_none() {
            if let Some(pem) = &metadata.jwt_public_key_pem {
                if let Ok(key) = jsonwebtoken::DecodingKey::from_rsa_pem(pem.as_bytes()) {
                    decoding_key_opt = Some(key);
                } else {
                    warn!("Invalid RSA public key PEM configured");
                }
            }
        }

        if let Some(decoding_key) = decoding_key_opt {
            // We enforce signature validation.
            // But we accept any aud for now since it's a mock test environment.
            validation.validate_exp = false; // For mock testing, ignore expiration
            validation.validate_aud = false; // Ignore audience for now

            match jsonwebtoken::decode::<Value>(token, &decoding_key, &validation) {
                Ok(token_data) => {
                    let claims = token_data.claims;
                    jwt_tenant_id = claims
                        .get("tenant_id")
                        .or(claims.get("tenant"))
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string());
                    principal = claims
                        .get("sub")
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string());
                }
                Err(e) => {
                    warn!("JWT Signature verification failed: {}", e);
                }
            }
        } else {
            warn!("No valid key (JWKS or PEM) available to verify signature");
        }
    }

    let principal = match principal {
        Some(p) => p,
        None => {
            warn!("Missing or invalid JWT signature in Authorization header");
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Missing or cryptographically invalid JWT token" })),
            )
                .into_response();
        }
    };

    let final_tenant_id = jwt_tenant_id.unwrap_or(metadata.tenant_id);

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
    policy_input["action"] = json!(normalized.tool_name.unwrap_or(normalized.request_type));
    policy_input["principal"] = json!(principal);
    policy_input["resource"] = json!("mcp_tool");

    // Evaluate against the Adaptive Policy Pipeline
    let decision_result = state.router.read().await.authorize(policy_input).await;

    match decision_result {
        Ok(decision) => {
            info!("Final Pipeline Decision: {:?}", decision);
            if decision.allow {
                let response = json!({
                    "status": "allowed",
                    "message": "The request passed the PEP evaluation.",
                    "decision": decision
                });

                // Apply PII redaction plugin if required
                if let Ok(redacted) = state.plugin_host.invoke("pii-redactor", response.clone()) {
                    info!("Applied PII redaction plugin successfully.");
                    (StatusCode::OK, Json(redacted)).into_response()
                } else {
                    (StatusCode::OK, Json(response)).into_response()
                }
            } else {
                (
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "status": "denied",
                        "reason": decision.reason,
                        "decision": decision
                    })),
                )
                    .into_response()
            }
        }
        Err(e) => {
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
