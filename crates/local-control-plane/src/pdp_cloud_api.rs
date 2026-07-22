// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::state::AppState;
use axum::{
    extract::Path,
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;

const CLOUD_RUNTIME_ID: &str = "pollek_cloud";
const DEV_CONTROL_SIGNING_KEY: &str = "local-dev-ephemeral-control-key";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudPdpProfile {
    pub tenant_id: Option<String>,
    pub device_id: Option<String>,
    pub pdp_endpoint: Option<String>,
    pub contract_version: Option<String>,
    pub auth_method: Option<String>,
    pub status: String,
    pub manual_override_enabled: bool,
    pub health: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedControlEnvelope {
    pub schema_version: Option<String>,
    pub control_id: String,
    pub tenant_id: Option<String>,
    pub issuer: Option<String>,
    pub audience: Option<String>,
    pub lcp_id: Option<String>,
    pub action: String,
    #[serde(default)]
    pub scope: Vec<String>,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    pub issued_at: Option<String>,
    pub expires_at: Option<String>,
    pub nonce: Option<String>,
    pub payload_hash: String,
    pub signer: Option<serde_json::Value>,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureControlMessage {
    pub schema_version: Option<String>,
    pub envelope: SignedControlEnvelope,
    pub payload: serde_json::Value,
    #[serde(default)]
    pub security_posture: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
struct ControlVerification {
    status: String,
    control_id: String,
    payload_hash: String,
    signature_verified: bool,
    signature_mode: String,
    replay_recorded: bool,
    warnings: Vec<String>,
}

impl Default for CloudPdpProfile {
    fn default() -> Self {
        Self {
            tenant_id: None,
            device_id: None,
            pdp_endpoint: None,
            contract_version: None,
            auth_method: None,
            status: "disconnected".to_string(),
            manual_override_enabled: false,
            health: None,
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/tenants/:tenant/pdp/cloud",
            get(get_cloud_profile)
                .patch(update_cloud_profile)
                .delete(disconnect_cloud_profile),
        )
        .route("/v1/tenants/:tenant/pdp/cloud/login", post(cloud_login))
        .route(
            "/v1/tenants/:tenant/pdp/cloud/discover",
            post(cloud_discover),
        )
        .route("/v1/tenants/:tenant/pdp/cloud/probe", post(cloud_probe))
        .route(
            "/v1/tenants/:tenant/bundles/hot-reload",
            post(apply_tenant_bundle_hot_reload),
        )
        .route(
            "/v1/tenants/:tenant/policy-bundles/hot-reload",
            post(apply_tenant_policy_bundle_hot_reload),
        )
        .route(
            "/v1/policy-bundles/:bundle_id/hot-reload",
            post(apply_named_policy_bundle_hot_reload),
        )
}

async fn get_cloud_profile(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> Json<CloudPdpProfile> {
    Json(load_cloud_profile(&st, &tenant).await)
}

async fn update_cloud_profile(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<CloudPdpProfile>, crate::error::ApiError> {
    let control_envelope = payload
        .get("control_envelope")
        .cloned()
        .and_then(|value| serde_json::from_value::<SignedControlEnvelope>(value).ok());
    let profile_payload = strip_control_envelope(payload);
    let mut profile = serde_json::from_value::<CloudPdpProfile>(profile_payload.clone())
        .unwrap_or_else(|_| CloudPdpProfile {
            status: profile_payload
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("configured")
                .to_string(),
            ..CloudPdpProfile::default()
        });

    let control = match control_envelope {
        Some(envelope) => validate_control_envelope(
            &st,
            &tenant,
            "/v1/tenants/local/pdp/cloud",
            "config.update",
            &profile_payload,
            &envelope,
        )
        .await
        .map(Some)?,
        None => None,
    };

    if profile.status.trim().is_empty() || profile.status == "disconnected" {
        profile.status = "configured".to_string();
    }
    if let Some(endpoint) = profile.pdp_endpoint.as_mut() {
        *endpoint = normalize_endpoint(endpoint);
    }
    if let Some(control) = control {
        profile.health = Some(merge_health_control(profile.health, control));
    }
    persist_cloud_profile(&st, &tenant, &profile)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(profile))
}

async fn disconnect_cloud_profile(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> Json<CloudPdpProfile> {
    let _ = st.pdp_store.delete_runtime(&tenant, CLOUD_RUNTIME_ID).await;
    Json(CloudPdpProfile::default())
}

async fn cloud_login(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> Json<CloudPdpProfile> {
    let profile = connect_cloud_profile(&st, &tenant).await;
    Json(profile)
}

async fn cloud_discover(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> Json<CloudPdpProfile> {
    cloud_login(Path(tenant), State(st)).await
}

async fn cloud_probe(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
) -> Json<CloudPdpProfile> {
    let profile = connect_cloud_profile(&st, &tenant).await;
    Json(profile)
}

async fn apply_tenant_bundle_hot_reload(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(message): Json<SecureControlMessage>,
) -> Result<(StatusCode, Json<serde_json::Value>), crate::error::ApiError> {
    apply_cloud_hot_reload(
        st,
        tenant,
        None,
        "/v1/tenants/local/bundles/hot-reload",
        message,
    )
    .await
}

async fn apply_tenant_policy_bundle_hot_reload(
    Path(tenant): Path<String>,
    State(st): State<AppState>,
    Json(message): Json<SecureControlMessage>,
) -> Result<(StatusCode, Json<serde_json::Value>), crate::error::ApiError> {
    apply_cloud_hot_reload(
        st,
        tenant,
        None,
        "/v1/tenants/local/policy-bundles/hot-reload",
        message,
    )
    .await
}

async fn apply_named_policy_bundle_hot_reload(
    Path(bundle_id): Path<String>,
    State(st): State<AppState>,
    Json(message): Json<SecureControlMessage>,
) -> Result<(StatusCode, Json<serde_json::Value>), crate::error::ApiError> {
    let expected_path = format!("/v1/policy-bundles/{bundle_id}/hot-reload");
    apply_cloud_hot_reload(
        st,
        "local".to_string(),
        Some(bundle_id),
        &expected_path,
        message,
    )
    .await
}

async fn apply_cloud_hot_reload(
    st: AppState,
    tenant: String,
    requested_bundle_id: Option<String>,
    expected_path: &str,
    message: SecureControlMessage,
) -> Result<(StatusCode, Json<serde_json::Value>), crate::error::ApiError> {
    let verification = validate_control_envelope(
        &st,
        &tenant,
        expected_path,
        "policy.hot_reload",
        &message.payload,
        &message.envelope,
    )
    .await?;

    let policy_bundle = message.payload.get("policy_bundle").cloned();
    let bundle_id = requested_bundle_id
        .or_else(|| {
            policy_bundle
                .as_ref()
                .and_then(|bundle| bundle.get("bundle_id"))
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "cloud-dispatched-bundle".to_string());

    let manifest_url = policy_bundle
        .as_ref()
        .and_then(|bundle| bundle.get("manifest_url"))
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned);
    let fetched_manifest = fetch_bundle_manifest(manifest_url.as_deref()).await;
    let manifest_status = match &fetched_manifest {
        Some(Ok(_)) => "fetched",
        Some(Err(_)) => "fetch_failed",
        None => "not_requested",
    };
    let manifest = fetched_manifest
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .cloned();

    let applied_at = chrono::Utc::now().to_rfc3339();
    let generation = st
        .build_number
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        + 1;

    let active_bundle = serde_json::json!({
        "schema_version": "pollek.local.cloud-hot-reload-applied.v1",
        "bundle_id": bundle_id,
        "tenant_id": tenant,
        "generation": generation,
        "status": "active",
        "source": "pollek_cloud_dispatch",
        "manifest_url": manifest_url,
        "manifest_status": manifest_status,
        "manifest": manifest,
        "policy_bundle": policy_bundle,
        "control": verification,
        "payload_hash": message.envelope.payload_hash,
        "applied_path": expected_path,
        "applied_at": applied_at
    });

    st.policy_store
        .upsert_policy_raw(&tenant, "bundle:latest", &active_bundle)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    st.policy_store
        .upsert_policy_raw(&tenant, &format!("bundle:{bundle_id}"), &active_bundle)
        .await
        .map_err(crate::error::ApiError::Internal)?;

    let runtime = cloud_hot_reload_runtime(&bundle_id, generation, &active_bundle);
    st.pdp_store
        .upsert_runtime(&tenant, "local.policy_hot_reload", &runtime)
        .await
        .map_err(crate::error::ApiError::Internal)?;

    let event_id = format!("hotreload_{}", uuid::Uuid::new_v4());
    let event = serde_json::json!({
        "schema_version": "pollek.local.hot-reload-event.v1",
        "event_id": event_id,
        "tenant_id": tenant,
        "bundle_id": bundle_id,
        "generation": generation,
        "status": "applied",
        "control_id": message.envelope.control_id,
        "payload_hash": message.envelope.payload_hash,
        "applied_path": expected_path,
        "manifest_status": manifest_status,
        "applied_at": applied_at
    });
    st.telemetry_store
        .put_telemetry(&tenant, "hot_reload", &event_id, &event)
        .await
        .map_err(crate::error::ApiError::Internal)?;

    let _ = st.bundle_tx.send(bundle_id.clone());

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "schema_version": "pollek.local.hot-reload-apply-response.v1",
            "status": "applied",
            "tenant_id": tenant,
            "bundle_id": bundle_id,
            "generation": generation,
            "applied_paths": [expected_path],
            "manifest_status": manifest_status,
            "control": active_bundle.get("control").cloned().unwrap_or(serde_json::json!({})),
            "event_id": event_id,
            "applied_at": applied_at
        })),
    ))
}

async fn load_cloud_profile(st: &AppState, tenant: &str) -> CloudPdpProfile {
    match st.pdp_store.get_runtime(tenant, CLOUD_RUNTIME_ID).await {
        Ok(Some(value)) => runtime_to_cloud_profile(&value),
        _ => CloudPdpProfile::default(),
    }
}

async fn connect_cloud_profile(st: &AppState, tenant: &str) -> CloudPdpProfile {
    let mut profile = load_cloud_profile(st, tenant).await;

    if profile.pdp_endpoint.is_none() {
        profile.pdp_endpoint =
            env_trimmed("POLLEK_CLOUD_URL").map(|value| normalize_endpoint(&value));
    }
    if profile.tenant_id.is_none() {
        profile.tenant_id = env_trimmed("POLLEK_CLOUD_TENANT_ID");
    }
    if profile.device_id.is_none() {
        profile.device_id = env_trimmed("POLLEK_DEVICE_ID").or_else(|| Some("local-device".into()));
    }

    let Some(endpoint) = profile.pdp_endpoint.clone() else {
        profile.status = "needs_configuration".to_string();
        profile.health = Some(serde_json::json!({
            "status": "not_configured",
            "detail": "Set a Pollek Cloud URL before enabling Enterprise Cloud mode."
        }));
        let _ = persist_cloud_profile(st, tenant, &profile).await;
        return profile;
    };

    match discover_contract(&endpoint).await {
        Ok((contract_version, health)) => {
            profile.status = "connected".to_string();
            profile.pdp_endpoint = Some(endpoint);
            profile.contract_version = Some(contract_version);
            profile.auth_method = profile
                .auth_method
                .or_else(|| Some("spiffe-oauth-mtls".to_string()));
            profile.health = Some(health);
        }
        Err(detail) => {
            profile.status = "connection_failed".to_string();
            profile.health = Some(serde_json::json!({
                "status": "unhealthy",
                "detail": detail
            }));
        }
    }

    let _ = persist_cloud_profile(st, tenant, &profile).await;
    profile
}

async fn discover_contract(endpoint: &str) -> Result<(String, serde_json::Value), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|err| format!("failed to build cloud probe client: {err}"))?;
    let url = format!(
        "{}/.well-known/pollek-contract",
        normalize_endpoint(endpoint)
    );
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|err| format!("contract discovery request failed: {err}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("contract discovery returned HTTP {status}"));
    }
    let json = response
        .json::<serde_json::Value>()
        .await
        .map_err(|err| format!("contract discovery returned invalid JSON: {err}"))?;
    let version = json
        .get("preferred")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown")
        .to_string();
    Ok((
        version,
        serde_json::json!({
            "status": "healthy",
            "detail": "contract discovery succeeded",
            "contract_url": url,
            "checked_at": chrono::Utc::now().to_rfc3339()
        }),
    ))
}

async fn persist_cloud_profile(
    st: &AppState,
    tenant: &str,
    profile: &CloudPdpProfile,
) -> anyhow::Result<()> {
    st.pdp_store
        .upsert_runtime(tenant, CLOUD_RUNTIME_ID, &cloud_profile_to_runtime(profile))
        .await
}

fn cloud_profile_to_runtime(profile: &CloudPdpProfile) -> serde_json::Value {
    serde_json::json!({
        "id": CLOUD_RUNTIME_ID,
        "name": "Pollek Cloud PDP",
        "category": "cloud_pdp",
        "kind": "pollek_cloud",
        "enabled": profile.status != "disconnected",
        "status": profile.status.as_str(),
        "endpoint": profile.pdp_endpoint.as_ref(),
        "auth_ref": profile.auth_method.as_ref(),
        "capabilities": [
            "contract_discovery",
            "policy_hot_reload",
            "telemetry_sync"
        ],
        "health": {
            "status": profile.health.as_ref()
                .and_then(|value| value.get("status"))
                .and_then(|value| value.as_str())
                .unwrap_or(profile.status.as_str()),
            "profile_status": profile.status.as_str(),
            "tenant_id": profile.tenant_id.as_ref(),
            "device_id": profile.device_id.as_ref(),
            "contract_version": profile.contract_version.as_ref(),
            "auth_method": profile.auth_method.as_ref(),
            "manual_override_enabled": profile.manual_override_enabled,
            "cloud_health": profile.health.as_ref()
        }
    })
}

fn cloud_hot_reload_runtime(
    bundle_id: &str,
    generation: u64,
    active_bundle: &serde_json::Value,
) -> serde_json::Value {
    let applied_at = active_bundle
        .get("applied_at")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    serde_json::json!({
        "id": "local.policy_hot_reload",
        "name": "Local Policy Hot Reload",
        "category": "local_engine",
        "kind": "policy_router",
        "enabled": true,
        "status": "ready",
        "capabilities": [
            { "name": "cloud_to_local_hot_reload", "version": "1.0", "supported": true },
            { "name": "signed_control_envelope", "version": "1.0", "supported": true }
        ],
        "health": {
            "health": "healthy",
            "readiness": "ready",
            "status": "applied",
            "message": "Cloud-dispatched hot reload was applied to the local bundle runtime.",
            "last_checked_at": applied_at,
            "active_bundle_id": bundle_id,
            "generation": generation,
            "control": active_bundle.get("control").cloned()
        }
    })
}

async fn validate_control_envelope(
    st: &AppState,
    tenant: &str,
    expected_path: &str,
    expected_action: &str,
    payload: &serde_json::Value,
    envelope: &SignedControlEnvelope,
) -> Result<ControlVerification, crate::error::ApiError> {
    let mut warnings = Vec::new();
    if envelope.tenant_id.as_deref().unwrap_or(tenant) != tenant {
        return Err(crate::error::ApiError::Forbidden(
            "control envelope tenant does not match the local tenant".to_string(),
        ));
    }
    if envelope.action != expected_action {
        return Err(crate::error::ApiError::Forbidden(format!(
            "control envelope action '{}' cannot be applied to '{}'",
            envelope.action, expected_action
        )));
    }
    if !envelope
        .allowed_paths
        .iter()
        .any(|path| path == expected_path)
    {
        return Err(crate::error::ApiError::Forbidden(format!(
            "control envelope does not allow path {expected_path}"
        )));
    }
    if let Some(expires_at) = envelope.expires_at.as_deref() {
        let expires_at = chrono::DateTime::parse_from_rfc3339(expires_at).map_err(|err| {
            crate::error::ApiError::BadRequest(format!("invalid control envelope expiry: {err}"))
        })?;
        if expires_at.with_timezone(&chrono::Utc) <= chrono::Utc::now() {
            return Err(crate::error::ApiError::Forbidden(
                "control envelope is expired".to_string(),
            ));
        }
    } else {
        warnings
            .push("missing expires_at; accepted only for local development compatibility".into());
    }

    let computed_hash = sha256_hex(stable_json(payload).as_bytes());
    if computed_hash != envelope.payload_hash {
        return Err(crate::error::ApiError::Forbidden(
            "control envelope payload hash mismatch".to_string(),
        ));
    }

    if st
        .policy_store
        .get_policy_raw(tenant, &format!("cloud-control:{}", envelope.control_id))
        .await
        .map_err(crate::error::ApiError::Internal)?
        .is_some()
    {
        return Err(crate::error::ApiError::Conflict(format!(
            "control envelope {} was already applied",
            envelope.control_id
        )));
    }

    let (signature_verified, signature_mode) = verify_control_signature(envelope);
    if !signature_verified {
        return Err(crate::error::ApiError::Forbidden(
            "control envelope signature verification failed".to_string(),
        ));
    }
    if std::env::var("POLLEK_CLOUD_CONTROL_SIGNING_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        warnings.push(
            "verified with local development control key; set POLLEK_CLOUD_CONTROL_SIGNING_KEY for production-like dispatch"
                .to_string(),
        );
    }

    let record = serde_json::json!({
        "schema_version": "pollek.local.cloud-control-replay-record.v1",
        "control_id": envelope.control_id,
        "tenant_id": tenant,
        "action": envelope.action,
        "allowed_path": expected_path,
        "payload_hash": envelope.payload_hash,
        "signature_mode": signature_mode,
        "recorded_at": chrono::Utc::now().to_rfc3339()
    });
    st.policy_store
        .upsert_policy_raw(
            tenant,
            &format!("cloud-control:{}", envelope.control_id),
            &record,
        )
        .await
        .map_err(crate::error::ApiError::Internal)?;

    Ok(ControlVerification {
        status: "verified".to_string(),
        control_id: envelope.control_id.clone(),
        payload_hash: envelope.payload_hash.clone(),
        signature_verified,
        signature_mode,
        replay_recorded: true,
        warnings,
    })
}

fn verify_control_signature(envelope: &SignedControlEnvelope) -> (bool, String) {
    let signing_key = std::env::var("POLLEK_CLOUD_CONTROL_SIGNING_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEV_CONTROL_SIGNING_KEY.to_string());
    let mut unsigned = serde_json::to_value(envelope).unwrap_or_else(|_| serde_json::json!({}));
    if let serde_json::Value::Object(ref mut object) = unsigned {
        object.remove("signature");
    }
    let expected = hmac_sha256_base64url(signing_key.as_bytes(), stable_json(&unsigned).as_bytes());
    let mode = if signing_key == DEV_CONTROL_SIGNING_KEY {
        "local_dev_hs256".to_string()
    } else {
        "env_hs256".to_string()
    };
    (expected == envelope.signature, mode)
}

fn strip_control_envelope(mut payload: serde_json::Value) -> serde_json::Value {
    if let serde_json::Value::Object(ref mut object) = payload {
        object.remove("control_envelope");
    }
    payload
}

fn merge_health_control(
    health: Option<serde_json::Value>,
    control: ControlVerification,
) -> serde_json::Value {
    let mut merged = health.unwrap_or_else(|| serde_json::json!({}));
    if let serde_json::Value::Object(ref mut object) = merged {
        object.insert("control".to_string(), serde_json::json!(control));
    }
    merged
}

async fn fetch_bundle_manifest(
    manifest_url: Option<&str>,
) -> Option<Result<serde_json::Value, String>> {
    let manifest_url = manifest_url?;
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
    {
        Ok(client) => client,
        Err(err) => return Some(Err(format!("failed to build manifest client: {err}"))),
    };
    let response = match client.get(manifest_url).send().await {
        Ok(response) => response,
        Err(err) => return Some(Err(format!("manifest fetch failed: {err}"))),
    };
    let status = response.status();
    if !status.is_success() {
        return Some(Err(format!("manifest fetch returned HTTP {status}")));
    }
    Some(
        response
            .json::<serde_json::Value>()
            .await
            .map_err(|err| format!("manifest response was invalid JSON: {err}")),
    )
}

fn stable_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
        }
        serde_json::Value::Array(items) => {
            let rendered = items.iter().map(stable_json).collect::<Vec<_>>().join(",");
            format!("[{rendered}]")
        }
        serde_json::Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let rendered = keys
                .into_iter()
                .map(|key| {
                    let encoded_key =
                        serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_string());
                    let encoded_value = object
                        .get(key)
                        .map(stable_json)
                        .unwrap_or_else(|| "null".into());
                    format!("{encoded_key}:{encoded_value}")
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{rendered}}}")
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn hmac_sha256_base64url(key: &[u8], data: &[u8]) -> String {
    const BLOCK_SIZE: usize = 64;
    let mut key_block = [0_u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let digest = Sha256::digest(key);
        key_block[..digest.len()].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut outer = [0x5c_u8; BLOCK_SIZE];
    let mut inner = [0x36_u8; BLOCK_SIZE];
    for index in 0..BLOCK_SIZE {
        outer[index] ^= key_block[index];
        inner[index] ^= key_block[index];
    }

    let mut inner_hash = Sha256::new();
    inner_hash.update(inner);
    inner_hash.update(data);
    let inner_result = inner_hash.finalize();

    let mut outer_hash = Sha256::new();
    outer_hash.update(outer);
    outer_hash.update(inner_result);
    URL_SAFE_NO_PAD.encode(outer_hash.finalize())
}

fn runtime_to_cloud_profile(value: &serde_json::Value) -> CloudPdpProfile {
    let health = value.get("health").cloned();
    let health_ref = health.as_ref();
    let read_health_str = |key: &str| {
        health_ref
            .and_then(|h| h.get(key))
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned)
    };
    CloudPdpProfile {
        tenant_id: read_health_str("tenant_id"),
        device_id: read_health_str("device_id"),
        pdp_endpoint: value
            .get("endpoint")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        contract_version: read_health_str("contract_version"),
        auth_method: read_health_str("auth_method").or_else(|| {
            value
                .get("auth_ref")
                .and_then(|v| v.as_str())
                .map(ToOwned::to_owned)
        }),
        status: read_health_str("profile_status").unwrap_or_else(|| {
            value
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("disconnected")
                .to_string()
        }),
        manual_override_enabled: health_ref
            .and_then(|h| h.get("manual_override_enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        health,
    }
}

fn normalize_endpoint(endpoint: &str) -> String {
    endpoint.trim().trim_end_matches('/').to_string()
}

fn env_trimmed(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
