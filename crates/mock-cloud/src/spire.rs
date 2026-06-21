use crate::state::{rand_hex, AppState, DeviceStatus};
use anyhow::{Context, Result};
use axum::{
    extract::{Form, Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde_json::{json, Value};
use tracing::{info, warn};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/oauth/device_authorization", post(device_authorization))
        .route("/oauth/token", post(token))
        .route("/enroll", post(enroll_device))
        .route("/spire/node/attest", post(attest_csr))
        .route(
            "/v1/tenants/:tenant_id/devices/:device_id/spire/svid/renew",
            post(renew_csr),
        )
        .route(
            "/v1/tenants/:tenant_id/devices/:device_id/rotate",
            post(rotate_device),
        )
        .route(
            "/v1/tenants/:tenant_id/devices/:device_id/revoke",
            post(revoke_device),
        )
        .route(
            "/v1/tenants/:tenant_id/devices/:device_id/status",
            get(get_device_status),
        )
}

#[derive(serde::Deserialize)]
struct DeviceAuthForm {
    #[allow(dead_code)]
    client_id: Option<String>,
    #[allow(dead_code)]
    scope: Option<String>,
}

fn rand_user_code() -> String {
    use rand_core::RngCore;
    const ALPHA: &[u8] = b"BCDFGHJKLMNPQRSTVWXZ";
    let mut b = [0u8; 8];
    rand_core::OsRng.fill_bytes(&mut b);
    let c: String = b
        .iter()
        .map(|x| ALPHA[(*x as usize) % ALPHA.len()] as char)
        .collect();
    format!("{}-{}", &c[0..4], &c[4..8])
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

#[derive(serde::Deserialize)]
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

    let trust_bundle = std::fs::read_to_string("certs/root_ca.crt").unwrap_or_default();
    let device_id = "device-001";
    let join_token = rand_hex(16);
    info!("CLOUD: enroll -> issuing join_token for {}", device_id);

    let mut devices = state.devices.lock().unwrap();
    if !devices.contains_key(device_id) {
        devices.insert(
            device_id.to_string(),
            DeviceStatus {
                id: device_id.to_string(),
                tenant_id: "tenant-production-1".to_string(),
                profile: "Developer".to_string(),
                revoked: false,
                last_health: Utc::now().to_rfc3339(),
                capabilities: dek_domain_schema::EnforcementCapabilities::default(),
            },
        );
    }

    (
        StatusCode::OK,
        Json(json!({
            "join_token": join_token,
            "spire_endpoint": "https://127.0.0.1:43892/spire",
            "trust_bundle_pem": trust_bundle,
            "pinned_bundle_public_key": crate::bundle_pubkey_b64(),
            "tenant_id": "tenant-production-1",
            "device_id": device_id,
            "spiffe_id": format!("spiffe://pollen.cloud/tenant-production-1/device/{}", device_id),
            "cloud_url": "https://127.0.0.1:43891"
        })),
    )
}

#[derive(serde::Deserialize)]
struct JoinAttest {
    #[allow(dead_code)]
    join_token: Option<String>,
    device_id: String,
    csr_pem: String,
}

pub fn is_device_revoked(state: &AppState, device_id: &str) -> bool {
    let devices = state.devices.lock().unwrap();
    if let Some(dev) = devices.get(device_id) {
        return dev.revoked;
    }
    false
}

pub fn verify_device_tenant(
    state: &AppState,
    tenant_id: &str,
    device_id: &str,
) -> Result<(), &'static str> {
    let devices = state.devices.lock().unwrap();
    if let Some(dev) = devices.get(device_id) {
        if dev.tenant_id == tenant_id {
            return Ok(());
        }
        return Err("Tenant mismatch");
    }
    Err("Device not found")
}

async fn attest_csr(
    State(state): State<AppState>,
    Json(req): Json<JoinAttest>,
) -> (StatusCode, Json<Value>) {
    if is_device_revoked(&state, &req.device_id) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "device revoked"})),
        );
    }

    let devices = state.devices.lock().unwrap();
    let tenant_id = devices
        .get(&req.device_id)
        .map(|d| d.tenant_id.clone())
        .unwrap_or_else(|| "tenant-production-1".to_string());
    drop(devices);

    let spiffe_id = format!(
        "spiffe://pollen.cloud/{}/device/{}",
        tenant_id, req.device_id
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

async fn renew_csr(
    Path((tenant_id, device_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(req): Json<JoinAttest>,
) -> (StatusCode, Json<Value>) {
    if is_device_revoked(&state, &device_id) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "device revoked"})),
        );
    }
    if verify_device_tenant(&state, &tenant_id, &device_id).is_err() {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "tenant mismatch"})),
        );
    }

    let spiffe_id = format!(
        "spiffe://pollen.cloud/{}/device/{}",
        tenant_id, req.device_id
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

fn sign_csr(csr_pem: &str, spiffe_id: &str) -> Result<(String, String)> {
    use rcgen::{Certificate, CertificateParams, CertificateSigningRequest, KeyPair, SanType};

    let ca_key_pem =
        std::fs::read_to_string("certs/root_ca.key").context("read root_ca.key")?;
    let ca_cert_pem =
        std::fs::read_to_string("certs/root_ca.crt").context("read root_ca.crt")?;

    let ca_key = KeyPair::from_pem(&ca_key_pem).context("parse CA key")?;
    let ca_params =
        CertificateParams::from_ca_cert_pem(&ca_cert_pem, ca_key).context("CA params")?;
    let ca = Certificate::from_params(ca_params).context("CA cert")?;

    let mut csr = CertificateSigningRequest::from_pem(csr_pem).context("parse CSR")?;
    csr.params
        .subject_alt_names
        .push(SanType::URI(spiffe_id.to_string()));

    let cert_pem = csr.serialize_pem_with_signer(&ca).context("sign CSR")?;

    Ok((cert_pem, ca_cert_pem))
}

async fn rotate_device(
    Path((_tenant_id, device_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    info!(
        "CLOUD RECEIVED ROTATE REQUEST from {}: {}",
        device_id, payload
    );
    if is_device_revoked(&state, &device_id) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "device revoked"})),
        );
    }
    let join_token = rand_hex(16);
    (StatusCode::OK, Json(json!({ "join_token": join_token })))
}

async fn revoke_device(
    Path((_tenant_id, device_id)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    info!(
        "CLOUD RECEIVED REVOKE REQUEST from {}: {}",
        device_id, payload
    );
    let mut devices = state.devices.lock().unwrap();
    if let Some(dev) = devices.get_mut(&device_id) {
        dev.revoked = true;
    }
    (StatusCode::OK, Json(json!({ "status": "revoked" })))
}

async fn get_device_status(
    Path((_tenant_id, device_id)): Path<(String, String)>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let devices = state.devices.lock().unwrap();
    if let Some(dev) = devices.get(&device_id) {
        (
            StatusCode::OK,
            Json(json!({ "status": if dev.revoked { "revoked" } else { "active" } })),
        )
    } else {
        (StatusCode::NOT_FOUND, Json(json!({ "error": "not found" })))
    }
}
