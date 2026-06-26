// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::state::AppState;
use axum::{
    extract::Path,
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const CLOUD_RUNTIME_ID: &str = "pollek_cloud";

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
    Json(payload): Json<CloudPdpProfile>,
) -> Json<CloudPdpProfile> {
    let mut profile = payload;
    if profile.status.trim().is_empty() || profile.status == "disconnected" {
        profile.status = "configured".to_string();
    }
    if let Some(endpoint) = profile.pdp_endpoint.as_mut() {
        *endpoint = normalize_endpoint(endpoint);
    }
    let _ = persist_cloud_profile(&st, &tenant, &profile).await;
    Json(profile)
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

async fn load_cloud_profile(st: &AppState, tenant: &str) -> CloudPdpProfile {
    match st.pdp_store.get_runtime(tenant, CLOUD_RUNTIME_ID).await {
        Ok(Some(value)) => runtime_to_cloud_profile(&value),
        _ => CloudPdpProfile::default(),
    }
}

async fn connect_cloud_profile(st: &AppState, tenant: &str) -> CloudPdpProfile {
    let mut profile = load_cloud_profile(st, tenant).await;

    if profile.pdp_endpoint.is_none() {
        profile.pdp_endpoint = env_trimmed("POLLEK_CLOUD_URL")
            .or_else(|| env_trimmed("POLLEK_MOCK_CLOUD_URL"))
            .map(|value| normalize_endpoint(&value));
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
