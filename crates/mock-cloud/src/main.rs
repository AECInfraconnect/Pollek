use anyhow::{Context, Result};
use askama::Template;
use axum::{
    extract::{Form, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use base64::Engine;
use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey};
use rustls::{server::WebPkiClientVerifier, RootCertStore, ServerConfig};
use rustls_pemfile::{certs, private_key};
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::BufReader;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

// Static ed25519 seed used to sign policy bundles.
const BUNDLE_SEED: [u8; 32] = [
    0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10,
    0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10,
];

fn bundle_pubkey_b64() -> String {
    let sk = SigningKey::from_bytes(&BUNDLE_SEED);
    base64::prelude::BASE64_STANDARD.encode(sk.verifying_key().as_bytes())
}

#[derive(Clone, Debug)]
struct DeviceStatus {
    id: String,
    profile: String,
    revoked: bool,
    last_health: String,
}

#[derive(Clone, Debug)]
struct LogEntry {
    device_id: String,
    timestamp: String,
    action: String,
    decision: String,
}

#[derive(Clone, Debug)]
struct PolicyBundle {
    version: String,
    cedar_src: String,
    openfga_store: String,
}

#[derive(Clone, Debug)]
struct RolloutConfig {
    latest_bundle: PolicyBundle,
    canary_bundle: Option<PolicyBundle>,
    canary_percentage: u8, // 0-100
}

#[derive(Clone)]
struct AppState {
    revision: Arc<AtomicUsize>,
    rsa_public_key_pem: String,
    /// device_code -> poll count
    pending: Arc<Mutex<HashMap<String, u32>>>,
    /// device_id -> DeviceStatus
    devices: Arc<Mutex<HashMap<String, DeviceStatus>>>,
    /// decision logs buffer
    decision_logs: Arc<Mutex<VecDeque<LogEntry>>>,
    /// rollout config
    rollout: Arc<Mutex<RolloutConfig>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider().install_default().expect("Failed to install rustls crypto provider");
    tracing_subscriber::fmt::init();
    info!("Starting Mock Pollen Cloud (mTLS API :43891 + HTTPS Enrollment :43892)...");

    let mut rng = rand_core::OsRng;
    let priv_key = rsa::RsaPrivateKey::new(&mut rng, 2048).expect("rsa keygen");
    let pub_key = rsa::RsaPublicKey::from(&priv_key);
    let rsa_public_key_pem =
        rsa::pkcs8::EncodePublicKey::to_public_key_pem(&pub_key, rsa::pkcs8::LineEnding::LF)
            .expect("encode pub key");

    let state = AppState {
        revision: Arc::new(AtomicUsize::new(1)),
        rsa_public_key_pem,
        pending: Arc::new(Mutex::new(HashMap::new())),
        devices: Arc::new(Mutex::new(HashMap::new())),
        decision_logs: Arc::new(Mutex::new(VecDeque::with_capacity(100))),
        rollout: Arc::new(Mutex::new(RolloutConfig {
            latest_bundle: PolicyBundle {
                version: "1.0.0".to_string(),
                cedar_src: "permit(\n  principal == User::\"user_bob\",\n  action == Action::\"tools/call\",\n  resource == Resource::\"mcp_tool\"\n);".to_string(),
                openfga_store: "store_rev_1".to_string(),
            },
            canary_bundle: None,
            canary_percentage: 0,
        })),
    };

    // ---- mTLS API (post-enrollment): config / bundles / telemetry ----
    let api = Router::new()
        .route("/v1/tenants/:tenant_id/devices/:device_id/telemetry", post(ingest_telemetry))
        .route("/v1/tenants/:tenant_id/devices/:device_id/bundles/latest", get(get_latest_bundle))
        .route("/v1/tenants/:tenant_id/devices/:device_id/config", get(get_config))
        .route("/v1/tenants/:tenant_id/devices/:device_id/spire/svid/renew", post(renew_csr))
        .route("/v1/tenants/:tenant_id/devices/:device_id/decision-logs", post(ingest_decision_logs))
        .route("/v1/tenants/:tenant_id/devices/:device_id/health", post(report_health))
        .with_state(state.clone());

    // ---- Enrollment listener (PRE-identity, NO client cert) ----
    let enroll = Router::new()
        .route("/oauth/device_authorization", post(device_authorization))
        .route("/oauth/token", post(token))
        .route("/enroll", post(enroll_device))
        .route("/spire/node/attest", post(attest_csr))
        .route("/device", get(device_page_get).post(device_page_post))
        .route("/admin/dashboard", get(dashboard_page))
        .route("/admin/devices/:device_id/revoke", post(admin_revoke_device))
        .route("/admin/policies/publish", post(admin_publish_policy))
        .route("/admin/policies/rollout", post(admin_set_rollout))
        .with_state(state.clone());

    // Load server certificate and key
    let certs_der = load_certs("../../certs/server.crt")?;
    let key_der = load_private_key("../../certs/server.key")?;

    // ---- :43891 mTLS Config ----
    let mut root_cert_store = RootCertStore::empty();
    let ca_certs = load_certs("../../certs/root_ca.crt")?;
    root_cert_store.add_parsable_certificates(ca_certs);
    let client_verifier = WebPkiClientVerifier::builder(Arc::new(root_cert_store))
        .build()
        .context("build client verifier")?;

    let mut server_config_mtls = ServerConfig::builder()
        .with_client_cert_verifier(client_verifier)
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
                .expect("signal").recv().await;
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

// =========================== Templates ===========================
#[derive(Template)]
#[template(path = "device_approval.html")]
struct DeviceApprovalTemplate {
    code: String,
    error: Option<String>,
    success: Option<String>,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    devices: Vec<DeviceStatus>,
    recent_logs: Vec<LogEntry>,
    telemetry_count: usize,
}

// =========================== Handlers ===========================

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

async fn device_page_post(State(state): State<AppState>, Form(form): Form<DevicePagePost>) -> impl IntoResponse {
    // In a real system, we'd lookup device_code by user_code.
    // Here we just mark the next polling attempt as successful by removing it from `pending`?
    // Actually the token handler checks `pending`. For mock simplicity, let's just create a dummy device in registry
    // and wait for `/enroll` to pick it up. Or we could just record the profile for the *next* device that enrolls.
    // Let's create a placeholder device or just globally set next profile.
    // For MVP, we'll assign the profile to "device-001" which is hardcoded in /enroll.
    
    let mut devices = state.devices.lock().unwrap();
    devices.insert("device-001".to_string(), DeviceStatus {
        id: "device-001".to_string(),
        profile: form.profile,
        revoked: false,
        last_health: "Pending Enrollment".to_string(),
    });

    let tpl = DeviceApprovalTemplate {
        code: form.user_code,
        error: None,
        success: Some("Device approved and profile assigned.".to_string()),
    };
    Html(tpl.render().unwrap())
}

async fn dashboard_page(State(state): State<AppState>) -> impl IntoResponse {
    let devices: Vec<DeviceStatus> = state.devices.lock().unwrap().values().cloned().collect();
    let logs_guard = state.decision_logs.lock().unwrap();
    let recent_logs: Vec<LogEntry> = logs_guard.iter().take(50).cloned().collect();
    let count = logs_guard.len();

    let tpl = DashboardTemplate {
        devices,
        recent_logs,
        telemetry_count: count,
    };
    Html(tpl.render().unwrap())
}

async fn admin_revoke_device(State(state): State<AppState>, Path(device_id): Path<String>) -> impl IntoResponse {
    let mut devices = state.devices.lock().unwrap();
    if let Some(dev) = devices.get_mut(&device_id) {
        dev.revoked = true;
    }
    Redirect::to("/admin/dashboard")
}


#[derive(Deserialize)]
struct PublishPolicyReq {
    version: String,
    cedar_src: String,
    openfga_store: String,
}

async fn admin_publish_policy(State(state): State<AppState>, Json(req): Json<PublishPolicyReq>) -> impl IntoResponse {
    let mut rollout = state.rollout.lock().unwrap();
    rollout.latest_bundle = PolicyBundle {
        version: req.version,
        cedar_src: req.cedar_src,
        openfga_store: req.openfga_store,
    };
    Json(json!({"status": "published"}))
}

#[derive(Deserialize)]
struct RolloutReq {
    canary_percentage: u8,
    canary_bundle_version: String,
    canary_cedar_src: String,
    canary_openfga_store: String,
}

async fn admin_set_rollout(State(state): State<AppState>, Json(req): Json<RolloutReq>) -> impl IntoResponse {
    let mut rollout = state.rollout.lock().unwrap();
    rollout.canary_percentage = req.canary_percentage;
    rollout.canary_bundle = Some(PolicyBundle {
        version: req.canary_bundle_version,
        cedar_src: req.canary_cedar_src,
        openfga_store: req.canary_openfga_store,
    });
    Json(json!({"status": "rollout_updated"}))
}

#[derive(Deserialize)]
struct DeviceAuthForm {
    #[allow(dead_code)]
    client_id: Option<String>,
    #[allow(dead_code)]
    scope: Option<String>,
}

async fn device_authorization(
    State(state): State<AppState>,
    Form(_form): Form<DeviceAuthForm>,
) -> Json<Value> {
    let device_code = rand_hex(16);
    let user_code = rand_user_code();
    state.pending.lock().unwrap().insert(device_code.clone(), 0);
    info!("CLOUD: device_authorization -> user_code {}", user_code);
    Json(json!({
        "device_code": device_code,
        "user_code": user_code,
        "verification_uri": "https://127.0.0.1:43892/device",
        "verification_uri_complete": format!("https://127.0.0.1:43892/device?code={}", user_code),
        "expires_in": 300,
        "interval": 1
    }))
}

#[derive(Deserialize)]
struct TokenForm {
    #[allow(dead_code)]
    grant_type: Option<String>,
    device_code: Option<String>,
    #[allow(dead_code)]
    client_id: Option<String>,
}

async fn token(
    State(state): State<AppState>,
    Form(form): Form<TokenForm>,
) -> (StatusCode, Json<Value>) {
    let dc = form.device_code.unwrap_or_default();
    let mut m = state.pending.lock().unwrap();
    match m.get_mut(&dc) {
        None => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "expired_token" })),
        ),
        Some(count) => {
            *count += 1;
            if *count < 2 {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "authorization_pending" })),
                )
            } else {
                m.remove(&dc);
                info!("CLOUD: token granted for device_code {}", dc);
                (
                    StatusCode::OK,
                    Json(json!({
                        "access_token": format!("mock-access-{}", dc),
                        "token_type": "Bearer",
                        "expires_in": 3600
                    })),
                )
            }
        }
    }
}

async fn enroll_device(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    let has_bearer = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|h| h.starts_with("Bearer "))
        .unwrap_or(false);
    if !has_bearer {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "missing bearer token" })),
        );
    }

    let trust_bundle = std::fs::read_to_string("../../certs/root_ca.crt").unwrap_or_default();
    let device_id = "device-001";
    let join_token = rand_hex(16);
    info!("CLOUD: enroll -> issuing join_token for {}", device_id);

    // Register device if not exists
    let mut devices = state.devices.lock().unwrap();
    if !devices.contains_key(device_id) {
        devices.insert(device_id.to_string(), DeviceStatus {
            id: device_id.to_string(),
            profile: "Developer".to_string(),
            revoked: false,
            last_health: Utc::now().to_rfc3339(),
        });
    }

    (
        StatusCode::OK,
        Json(json!({
            "join_token": join_token,
            "spire_endpoint": "https://127.0.0.1:43892/spire",
            "trust_bundle_pem": trust_bundle,
            "pinned_bundle_public_key": bundle_pubkey_b64(),
            "tenant_id": "tenant-production-1",
            "device_id": device_id,
            "spiffe_id": format!("spiffe://pollen.cloud/tenant-production-1/device/{}", device_id),
            "cloud_url": "https://127.0.0.1:43891"
        })),
    )
}

#[derive(Deserialize)]
struct JoinAttest {
    #[allow(dead_code)]
    join_token: String,
    device_id: String,
    csr_pem: String,
}

async fn attest_csr(State(state): State<AppState>, Json(req): Json<JoinAttest>) -> (StatusCode, Json<Value>) {
    // Check revocation
    if is_device_revoked(&state, &req.device_id) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "device revoked"})));
    }

    let spiffe_id = format!(
        "spiffe://pollen.cloud/tenant-production-1/device/{}",
        req.device_id
    );
    match sign_csr(&req.csr_pem, &spiffe_id) {
        Ok((cert_pem, trust_bundle)) => {
            info!("CLOUD: signed X.509-SVID for {}", spiffe_id);
            (
                StatusCode::OK,
                Json(json!({
                    "svid_cert_pem": cert_pem,
                    "spiffe_id": spiffe_id,
                    "trust_bundle_pem": trust_bundle
                })),
            )
        }
        Err(e) => {
            warn!("CLOUD: CSR signing failed: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("csr signing failed: {e}") })),
            )
        }
    }
}

async fn renew_csr(Path((_tenant_id, device_id)): Path<(String, String)>, State(state): State<AppState>, Json(req): Json<JoinAttest>) -> (StatusCode, Json<Value>) {
    if is_device_revoked(&state, &device_id) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "device revoked"})));
    }

    let spiffe_id = format!(
        "spiffe://pollen.cloud/tenant-production-1/device/{}",
        req.device_id
    );
    match sign_csr(&req.csr_pem, &spiffe_id) {
        Ok((cert_pem, trust_bundle)) => {
            info!("CLOUD: renewed X.509-SVID for {}", spiffe_id);
            (
                StatusCode::OK,
                Json(json!({
                    "svid_cert_pem": cert_pem,
                    "spiffe_id": spiffe_id,
                    "trust_bundle_pem": trust_bundle
                })),
            )
        }
        Err(e) => {
            warn!("CLOUD: CSR renewal failed: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("csr renewal failed: {e}") })),
            )
        }
    }
}

fn is_device_revoked(state: &AppState, device_id: &str) -> bool {
    let devices = state.devices.lock().unwrap();
    if let Some(dev) = devices.get(device_id) {
        return dev.revoked;
    }
    false
}

fn sign_csr(csr_pem: &str, spiffe_id: &str) -> Result<(String, String)> {
    use rcgen::{Certificate, CertificateParams, CertificateSigningRequest, KeyPair, SanType};

    let ca_key_pem =
        std::fs::read_to_string("../../certs/root_ca.key").context("read root_ca.key")?;
    let ca_cert_pem =
        std::fs::read_to_string("../../certs/root_ca.crt").context("read root_ca.crt")?;

    let ca_key = KeyPair::from_pem(&ca_key_pem).context("parse CA key")?;
    let ca_params =
        CertificateParams::from_ca_cert_pem(&ca_cert_pem, ca_key).context("CA params")?;
    let ca = Certificate::from_params(ca_params).context("CA cert")?;

    let mut csr = CertificateSigningRequest::from_pem(csr_pem).context("parse CSR")?;
    csr.params.subject_alt_names.push(SanType::URI(spiffe_id.to_string()));

    let cert_pem = csr.serialize_pem_with_signer(&ca).context("sign CSR")?;

    Ok((cert_pem, ca_cert_pem))
}


fn rand_hex(n_bytes: usize) -> String {
    use rand_core::RngCore;
    let mut b = vec![0u8; n_bytes];
    rand_core::OsRng.fill_bytes(&mut b);
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

fn rand_user_code() -> String {
    use rand_core::RngCore;
    const ALPHA: &[u8] = b"BCDFGHJKLMNPQRSTVWXZ"; 
    let mut b = [0u8; 8];
    rand_core::OsRng.fill_bytes(&mut b);
    let c: String = b.iter().map(|x| ALPHA[(*x as usize) % ALPHA.len()] as char).collect();
    format!("{}-{}", &c[0..4], &c[4..8])
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

async fn ingest_telemetry(Path((_tenant_id, device_id)): Path<(String, String)>, State(_state): State<AppState>, Json(payload): Json<Value>) -> Json<Value> {
    info!("CLOUD RECEIVED TELEMETRY from {}: {}", device_id, payload);
    Json(json!({ "status": "ingested" }))
}

async fn ingest_decision_logs(Path((_tenant_id, device_id)): Path<(String, String)>, State(state): State<AppState>, Json(payload): Json<Value>) -> Json<Value> {
    info!("CLOUD RECEIVED DECISION LOGS from {}: {}", device_id, payload);
    
    // Parse decision logs
    let mut logs = state.decision_logs.lock().unwrap();
    if let Some(events) = payload.as_array() {
        for ev in events {
            let action = ev.get("action").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            let decision = ev.get("decision").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            let ts = ev.get("timestamp").and_then(|v| v.as_str()).unwrap_or_default().to_string();
            
            logs.push_front(LogEntry {
                device_id: device_id.clone(),
                timestamp: ts,
                action,
                decision,
            });
            
            if logs.len() > 1000 {
                logs.pop_back();
            }
        }
    }
    
    Json(json!({ "status": "ingested" }))
}

async fn report_health(Path((_tenant_id, device_id)): Path<(String, String)>, State(state): State<AppState>, Json(payload): Json<Value>) -> Json<Value> {
    info!("CLOUD RECEIVED HEALTH REPORT from {}: {}", device_id, payload);
    let mut devices = state.devices.lock().unwrap();
    if let Some(dev) = devices.get_mut(&device_id) {
        dev.last_health = Utc::now().to_rfc3339();
    }
    Json(json!({ "status": "ok" }))
}

async fn get_latest_bundle(Path((_tenant_id, device_id)): Path<(String, String)>, State(state): State<AppState>) -> impl IntoResponse {
    if is_device_revoked(&state, &device_id) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "device revoked"})));
    }

    let bundle = {
        let rollout = state.rollout.lock().unwrap();
        
        // Simple hash logic to determine canary inclusion
        let mut hash_val: usize = 0;
        for b in device_id.bytes() {
            hash_val = hash_val.wrapping_add(b as usize);
        }
        let dev_pct = (hash_val % 100) as u8;

        if let Some(ref canary) = rollout.canary_bundle {
            if dev_pct < rollout.canary_percentage {
                canary.clone()
            } else {
                rollout.latest_bundle.clone()
            }
        } else {
            rollout.latest_bundle.clone()
        }
    };

    let signing_key = SigningKey::from_bytes(&BUNDLE_SEED);
    let public_key = signing_key.verifying_key();

    let wasm_path = if std::path::Path::new("plugins/dummy_policy.wasm").exists() {
        "plugins/dummy_policy.wasm"
    } else if std::path::Path::new("target/wasm32-wasip1/release/dummy_policy.wasm").exists() {
        "target/wasm32-wasip1/release/dummy_policy.wasm"
    } else {
        "target/wasm32-wasip1/debug/dummy_policy.wasm"
    };

    let payload = json!({
        "jwt_config": {
            "public_key_pem": state.rsa_public_key_pem.clone(),
            "issuer_url": "https://127.0.0.1:43891",
            "audience": ["pollen-dek"]
        },
        "openfga": { "endpoint": "http://127.0.0.1:8080", "store_id": bundle.openfga_store },
        "cedar": { "policy_src": bundle.cedar_src },
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
    });

    let payload_string = serde_json::to_string(&payload).unwrap();
    let signature = signing_key.sign(payload_string.as_bytes());
    (StatusCode::OK, Json(json!({
        "bundle_id": format!("bnd-mcp-authz-{}", bundle.version),
        "version": bundle.version,
        "signature": base64::prelude::BASE64_STANDARD.encode(signature.to_bytes()),
        "public_key": base64::prelude::BASE64_STANDARD.encode(public_key.as_bytes()),
        "payload": payload
    })))
}

async fn get_config(Path((_tenant_id, device_id)): Path<(String, String)>, State(state): State<AppState>) -> impl IntoResponse {
    if is_device_revoked(&state, &device_id) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "device revoked"})));
    }

    let devices = state.devices.lock().unwrap();
    let profile = devices.get(&device_id).map(|d| d.profile.clone()).unwrap_or_else(|| "Developer".to_string());

    let rev = state.revision.fetch_add(1, Ordering::SeqCst);
    let wasm_path = if std::path::Path::new("plugins/dummy_policy.wasm").exists() {
        "plugins/dummy_policy.wasm"
    } else if std::path::Path::new("target/wasm32-wasip1/release/dummy_policy.wasm").exists() {
        "target/wasm32-wasip1/release/dummy_policy.wasm"
    } else {
        "target/wasm32-wasip1/debug/dummy_policy.wasm"
    };
    let store_id = format!("store_rev_{}", rev);
    (StatusCode::OK, Json(json!({
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
    })))
}
