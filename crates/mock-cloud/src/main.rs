pub mod admin;
pub mod assertions;
pub mod bundles;
pub mod compiler;
pub mod fixtures;
pub mod keys;
pub mod pdp;
pub mod registry;
pub mod scenarios;
pub mod spire;
pub mod state;
pub mod telemetry;
pub mod threats;
pub mod tuf;
pub mod update_server;
pub mod ui;

use anyhow::{Context, Result};
use askama::Template;
use axum::{
    extract::{Form, Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use chrono::Utc;
use ed25519_dalek::SigningKey;
use rustls::{server::WebPkiClientVerifier, RootCertStore, ServerConfig};
use rustls_pemfile::{certs, private_key};
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use serde::Deserialize;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use tracing::info;

use crate::state::{AppState, AuditLog, DeviceStatus, PolicyBundle, RolloutConfig, RegistryState};
use dek_domain_schema::TelemetryEvent;

// Static ed25519 seed used to sign policy bundles.
pub const BUNDLE_SEED: [u8; 32] = [
    0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10,
    0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10,
];

pub fn bundle_pubkey_b64() -> String {
    use base64::Engine;
    let sk = SigningKey::from_bytes(&BUNDLE_SEED);
    base64::prelude::BASE64_STANDARD.encode(sk.verifying_key().as_bytes())
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    seed: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    tracing_subscriber::fmt::init();
    info!("Starting Mock Pollen Cloud (mTLS API :43891 + HTTPS Enrollment :43892)...");

    let rsa_public_key_pem = "-----BEGIN PUBLIC KEY-----\n\
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAyP1z9L5h2dK+L2wXo9B3\n\
t0x/6e7S6t9A3q0V9Z6hZ+yR1q8Y/yB6fQ9Z0xK1Z6vR3V1N0Z7v1O1Y8y1T4wU9\n\
e2X0Y2k4X5P7Y5k1T4wU9e2X0Y2k4X5P7Y5k1T4wU9e2X0Y2k4X5P7Y5k1T4wU9\n\
e2X0Y2k4X5P7Y5k1T4wU9e2X0Y2k4X5P7Y5k1T4wU9e2X0Y2k4X5P7Y5k1T4wU9\n\
e2X0Y2k4X5P7Y5k1T4wU9e2X0Y2k4X5P7Y5k1T4wU9e2X0Y2k4X5P7Y5k1T4wU9\n\
e2X0Y2k4X5P7Y5k1T4wU9e2X0Y2k4X5P7Y5k1T4wU9e2X0Y2k4X5P7Y5k1T4wU9\n\
CwIDAQAB\n-----END PUBLIC KEY-----\n".to_string();

    let state = AppState {
        revision: Arc::new(AtomicUsize::new(1)),
        rsa_public_key_pem,
        pending: Arc::new(Mutex::new(HashMap::new())),
        devices: Arc::new(Mutex::new(HashMap::new())),
        telemetry_events: Arc::new(Mutex::new(VecDeque::with_capacity(1000))),
        rollout: Arc::new(Mutex::new(RolloutConfig {
            latest_bundle: PolicyBundle {
                version: "1.0.0".to_string(),
                cedar_src: "permit(\n  principal == User::\"user_bob\",\n  action == Action::\"tools/call\",\n  resource == Resource::\"mcp_tool\"\n);".to_string(),
                openfga_store: "store_rev_1".to_string(),
            },
            canary_bundle: None,
            canary_percentage: 0,
        })),
        audit_logs: Arc::new(Mutex::new(Vec::new())),
        pending_policies: Arc::new(Mutex::new(HashMap::new())),
        trusted_keys: Arc::new(Mutex::new(vec![
            serde_json::json!({
                "key_id": "bootstrap",
                "public_b64": crate::bundle_pubkey_b64(),
                "status": "active",
                "not_before_unix": 0,
                "not_after_unix": 0
            })
        ])),
        registry: Arc::new(Mutex::new(RegistryState::default())),
        chaos_config: Arc::new(Mutex::new(crate::state::ChaosConfig {
            outage_enabled: false,
            global_latency_ms: 0,
        })),
    };

    if let Some(profile) = args.seed {
        info!("Loading seed fixtures for profile: {}", profile);
        crate::fixtures::load_seed_data(&state, &profile);
    }

    // ---- mTLS API (post-enrollment): config / bundles / telemetry ----
    let api = Router::new()
        .merge(crate::registry::router())
        .merge(crate::compiler::router())
        .merge(crate::pdp::router())
        .merge(crate::scenarios::router())
        .route("/admin/registry", get(crate::admin::admin_dashboard))
        .route("/mock/admin/bundles/:bundle_id/poison", post(crate::admin::admin_bundle_poison))
        .merge(bundles::router())
        .merge(update_server::router())
        .merge(ui::router())
        .merge(tuf::router())
        .merge(keys::router())
        .route(
            "/v1/tenants/:tenant_id/devices/:device_id/config",
            get(get_config),
        )
        .route("/v1/push", get(handle_push_stream))
        // Order matters for middleware. Chaos middleware should wrap the v1 endpoints.
        // And telemetry / threats routes are merged before the layer so they are wrapped, BUT
        // the middleware internally explicitly filters to only act on `/v1/` routes.
        .merge(crate::telemetry::router())
        .merge(crate::threats::router())
        .layer(axum::middleware::from_fn_with_state(state.clone(), crate::threats::chaos_middleware))
        .with_state(state.clone());

    // ---- Enrollment listener (PRE-identity, NO client cert) ----
    let enroll = Router::new()
        .merge(spire::router())
        .route("/device", get(device_page_get).post(device_page_post))
        .route("/admin/dashboard", get(dashboard_page))
        .route(
            "/admin/devices/:device_id/revoke",
            post(admin_revoke_device),
        )
        .with_state(state.clone());

    // Load server certificate and key
    let certs_der = load_certs("certs/server.crt")?;
    let key_der = load_private_key("certs/server.key")?;

    // ---- :43891 mTLS Config ----
    let mut root_cert_store = RootCertStore::empty();
    let ca_certs = load_certs("certs/root_ca.crt")?;
    root_cert_store.add_parsable_certificates(ca_certs);
    let client_verifier = WebPkiClientVerifier::builder(Arc::new(root_cert_store))
        .allow_unauthenticated()
        .build()
        .context("build client verifier")?;

    let mut server_config_mtls = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs_der.clone(), key_der.clone_key())
        .context("server config mtls")?;
    server_config_mtls.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    let rustls_config_mtls = RustlsConfig::from_config(Arc::new(server_config_mtls));
    let addr_mtls = SocketAddr::from(([127, 0, 0, 1], 43891));

    // ---- :43892 HTTPS Self-Signed Config ----
    let mut server_config_https = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs_der, key_der)
        .context("server config https")?;
    server_config_https.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    let rustls_config_https = RustlsConfig::from_config(Arc::new(server_config_https));
    let addr_https = SocketAddr::from(([127, 0, 0, 1], 43892));

    info!("Mock Cloud mTLS API on https://127.0.0.1:43891");
    info!("Mock Cloud HTTPS Enrollment API on https://127.0.0.1:43892");
    info!("Dashboard: https://127.0.0.1:43892/admin/dashboard");

    let handle = Handle::new();
    let shutdown_handle = handle.clone();

    tokio::spawn(async move {
        let ctrl_c = async { tokio::signal::ctrl_c().await.expect("ctrl-c") };
        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("signal")
                .recv()
                .await;
        };
        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();
        tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
        info!("shutting down...");
        shutdown_handle.graceful_shutdown(None);
    });

    let mtls_server = axum_server::bind_rustls(addr_mtls, rustls_config_mtls)
        .handle(handle.clone())
        .serve(api.into_make_service());

    let https_server = axum_server::bind_rustls(addr_https, rustls_config_https)
        .handle(handle)
        .serve(enroll.into_make_service());

    let _ = tokio::try_join!(mtls_server, https_server)?;
    info!("Mock Cloud shut down gracefully.");
    Ok(())
}

fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    Ok(certs(&mut reader).collect::<Result<Vec<_>, _>>()?)
}

fn load_private_key(path: &str) -> Result<PrivateKeyDer<'static>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    private_key(&mut reader)?.context("No private key found")
}

// =========================== Templates ===========================
#[derive(Template)]
#[template(path = "device_approval.html")]
struct DeviceApprovalTemplate {
    code: String,
    error: Option<String>,
    success: Option<String>,
}

pub struct LogEntryView {
    pub timestamp: String,
    pub device_id: String,
    pub event_type: String,
    pub details: String,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    devices: Vec<DeviceStatus>,
    recent_logs: Vec<LogEntryView>,
    telemetry_count: usize,
    current_version: String,
    canary_info: String,
    audits: Vec<AuditLog>,
}

#[derive(Deserialize)]
struct DevicePageQuery {
    code: Option<String>,
}

async fn device_page_get(Query(query): Query<DevicePageQuery>) -> impl IntoResponse {
    let tpl = DeviceApprovalTemplate {
        code: query.code.unwrap_or_default(),
        error: None,
        success: None,
    };
    Html(tpl.render().unwrap())
}

#[derive(Deserialize)]
struct DevicePagePost {
    user_code: String,
    profile: String,
}

async fn device_page_post(
    State(state): State<AppState>,
    Form(form): Form<DevicePagePost>,
) -> impl IntoResponse {
    let mut devices = state.devices.lock().unwrap();
    devices.insert(
        "device-001".to_string(),
        DeviceStatus {
            id: "device-001".to_string(),
            tenant_id: "tenant-production-1".to_string(),
            profile: form.profile,
            revoked: false,
            last_health: "Pending Enrollment".to_string(),
            capabilities: dek_domain_schema::EnforcementCapabilities::default(),
        },
    );

    let tpl = DeviceApprovalTemplate {
        code: form.user_code,
        error: None,
        success: Some("Device approved and profile assigned.".to_string()),
    };
    Html(tpl.render().unwrap())
}

async fn dashboard_page(State(state): State<AppState>) -> impl IntoResponse {
    let devices: Vec<DeviceStatus> = state.devices.lock().unwrap().values().cloned().collect();
    let logs_guard = state.telemetry_events.lock().unwrap();
    let recent_logs: Vec<LogEntryView> = logs_guard.iter().take(50).map(|e| {
        match e {
            TelemetryEvent::Decision { timestamp, device_id, action, decision, .. } => {
                LogEntryView {
                    timestamp: timestamp.clone(),
                    device_id: device_id.clone(),
                    event_type: "Decision".to_string(),
                    details: format!("{} -> {}", action, decision),
                }
            },
            TelemetryEvent::Security { timestamp, device_id, severity, category, .. } => {
                LogEntryView {
                    timestamp: timestamp.clone(),
                    device_id: device_id.clone(),
                    event_type: "Security".to_string(),
                    details: format!("[{}] {}", severity, category),
                }
            },
            TelemetryEvent::Trace { trace_id, device_id, .. } => {
                LogEntryView {
                    timestamp: "N/A".to_string(), // Traces might not have top-level timestamp
                    device_id: device_id.clone(),
                    event_type: "Trace".to_string(),
                    details: format!("TraceID: {}", trace_id),
                }
            },
            TelemetryEvent::Metric { timestamp, device_id, .. } => {
                LogEntryView {
                    timestamp: timestamp.clone(),
                    device_id: device_id.clone(),
                    event_type: "Metric".to_string(),
                    details: "Metrics payload".to_string(),
                }
            },
            TelemetryEvent::EbpfGuardrail { timestamp, device_id, process_name, verdict, .. } => {
                LogEntryView {
                    timestamp: timestamp.clone(),
                    device_id: device_id.clone(),
                    event_type: "eBPF".to_string(),
                    details: format!("{} -> {}", process_name, verdict),
                }
            },
            TelemetryEvent::OsGuardrail { timestamp, device_id, os_platform, process_name, verdict, fqdn, .. } => {
                let proc = process_name.clone().unwrap_or_else(|| "unknown_proc".to_string());
                let dest = fqdn.clone().unwrap_or_else(|| "unknown_dest".to_string());
                LogEntryView {
                    timestamp: timestamp.clone(),
                    device_id: device_id.clone(),
                    event_type: format!("OS Guardrail ({})", os_platform),
                    details: format!("{} -> {} : {}", proc, dest, verdict),
                }
            },
            TelemetryEvent::OsLifecycle { timestamp, device_id, os_platform, component, event, .. } => {
                LogEntryView {
                    timestamp: timestamp.clone(),
                    device_id: device_id.clone(),
                    event_type: format!("OS Lifecycle ({})", os_platform),
                    details: format!("{} : {}", component, event),
                }
            }
        }
    }).collect();
    let count = logs_guard.len();

    let rollout_guard = state.rollout.lock().unwrap();
    let current_version = rollout_guard.latest_bundle.version.clone();
    let canary_info = rollout_guard
        .canary_bundle
        .as_ref()
        .map(|b| format!("{} ({}%)", b.version, rollout_guard.canary_percentage))
        .unwrap_or_else(|| "None".to_string());

    let audit_guard = state.audit_logs.lock().unwrap();
    let audits: Vec<AuditLog> = audit_guard.iter().rev().take(20).cloned().collect();

    let tpl = DashboardTemplate {
        devices,
        recent_logs,
        telemetry_count: count,
        current_version,
        canary_info,
        audits,
    };
    Html(tpl.render().unwrap())
}

async fn admin_revoke_device(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    let mut devices = state.devices.lock().unwrap();
    if let Some(dev) = devices.get_mut(&device_id) {
        dev.revoked = true;
        state.audit_logs.lock().unwrap().push(AuditLog {
            timestamp: Utc::now().to_rfc3339(),
            actor: "admin".to_string(),
            action: "REVOKE_DEVICE".to_string(),
            details: format!("Revoked access for device {}", device_id),
        });
    }
    Redirect::to("/admin/dashboard")
}

async fn get_config(
    Path((_tenant_id, device_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if crate::spire::is_device_revoked(&state, &device_id) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "device revoked"})),
        );
    }

    let devices = state.devices.lock().unwrap();
    let profile = devices
        .get(&device_id)
        .map(|d| d.profile.clone())
        .unwrap_or_else(|| "Developer".to_string());

    use std::sync::atomic::Ordering;
    let rev = state.revision.fetch_add(1, Ordering::SeqCst);
    let wasm_path = if std::path::Path::new("plugins/dummy_policy.wasm").exists() {
        "plugins/dummy_policy.wasm"
    } else if std::path::Path::new("target/wasm32-wasip1/release/dummy_policy.wasm").exists() {
        "target/wasm32-wasip1/release/dummy_policy.wasm"
    } else {
        "target/wasm32-wasip1/debug/dummy_policy.wasm"
    };
    let store_id = format!("store_rev_{}", rev);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "device_id": device_id,
            "tenant_id": "tenant-production-1",
            "profile": profile,
            "mtls": { "client_cert_path": "certs/client.crt", "client_key_path": "certs/client.key", "root_ca_path": "certs/root_ca.crt" },
            "spire_server": { "endpoint": "https://127.0.0.1:43891/spire" },
            "jwt_config": { "public_key_pem": state.rsa_public_key_pem.clone(), "issuer_url": "https://127.0.0.1:43891", "audience": ["pollen-dek"] },
            "policy_config": {
                "openfga": { "endpoint": "http://127.0.0.1:8080", "store_id": store_id },
                "cedar": { "policy_src": format!("permit(\n  principal == User::\"user_bob\",\n  action == Action::\"tools/call\",\n  resource == Resource::\"mcp_tool\"\n); // rev {}", rev) },
                "opa_wasm": { "policy_path": wasm_path },
                "routes": [
                    { "id": "route_tools_call", "priority": 100,
                      "match_rule": { "method": "tools/call", "tool_category": null },
                      "pdp_required": ["openfga", "opa_wasm"],
                      "pdp_conditional": [ { "evaluator": "cedar", "required_payload_key": "*" } ] },
                    { "id": "route_default", "priority": 10,
                      "match_rule": { "method": "*", "tool_category": null },
                      "pdp_required": ["openfga"], "pdp_conditional": [] }
                ]
            }
        })),
    )
}

async fn handle_push_stream() -> impl IntoResponse {
    use axum::response::sse::{Event, Sse};
    use futures_util::stream;
    use std::convert::Infallible;
    use std::time::Duration;
    
    let stream = stream::unfold(0, |count| async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        // Mock push event every 30 seconds
        let event = Event::default().data(format!("{{\"event\": \"bundle_ready\", \"version\": \"push-{}\"}}", count));
        Some((Ok::<_, Infallible>(event), count + 1))
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
}
