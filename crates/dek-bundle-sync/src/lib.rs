use anyhow::{Context, Result};
use dek_config::MtlsConfig;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ArtifactState {
    Discovered,
    Downloaded,
    HashVerified,
    SignatureVerified,
    SchemaValidated,
    CompatibilityChecked,
    Staged,
    Warmed,
    Shadow,
    Active,
    LastKnownGood,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub version: String,
    pub state: ArtifactState,
}

#[derive(Deserialize)]
struct BundleInfo {
    bundle_id: String,
    version: String,
    signature: String,
    #[allow(dead_code)]
    public_key: String,
    payload: serde_json::Value,
}

use dek_config::DekConfig;

pub struct BundleSyncAgent {
    cloud_url: String,
    device_id: String,
    pinned_public_key: String,
    client: RwLock<reqwest::Client>,
}

impl BundleSyncAgent {
    pub fn new(
        cloud_url: &str,
        device_id: &str,
        mtls: &MtlsConfig,
        pinned_public_key: &str,
        client_key_override: Option<&[u8]>
    ) -> Result<Self> {
        let client = mtls.build_client(client_key_override)?;
        Ok(Self {
            cloud_url: cloud_url.to_string(),
            device_id: device_id.to_string(),
            pinned_public_key: pinned_public_key.to_string(),
            client: RwLock::new(client),
        })
    }

    pub async fn update_mtls(&self, mtls: &MtlsConfig) -> Result<()> {
        let new_client = mtls.build_client(None)?;
        let mut client_lock = self.client.write().await;
        *client_lock = new_client;
        info!("[BundleSync] Successfully updated internal HTTP client with new mTLS configuration");
        Ok(())
    }

    pub async fn run_pipeline(&self) -> Result<DekConfig> {
        let client = self.client.read().await.clone();

        info!("[BundleSync] Starting Unified Sync Pipeline...");

        // 1. Fetch Device Config
        let config_url = format!("{}/config/{}", self.cloud_url, self.device_id);
        let res = client.get(&config_url).send().await?;
        if !res.status().is_success() {
            error!(
                "[BundleSync] Failed to fetch device config: HTTP {}",
                res.status()
            );
            return Err(anyhow::anyhow!("Config fetch failed"));
        }
        let dek_config: DekConfig = res.json().await.context("Failed to parse DekConfig")?;
        info!(
            "[BundleSync] Successfully fetched device config for tenant: {}",
            dek_config.tenant_id
        );

        // 2. Fetch Latest Bundle
        let bundle_url = format!("{}/bundles/latest", self.cloud_url);
        let res = client.get(&bundle_url).send().await?;
        let bundle_payload = if res.status().is_success() {
            let bundle_info: BundleInfo =
                res.json().await.context("Failed to parse bundle info")?;

            let mut artifact = Artifact {
                id: bundle_info.bundle_id.clone(),
                version: bundle_info.version.clone(),
                state: ArtifactState::Discovered,
            };
            info!(
                "[BundleSync] Discovered new bundle: {} v{}",
                artifact.id, artifact.version
            );

            // Transition to Downloaded
            artifact.state = ArtifactState::Downloaded;

            // Verify Signature using Pinned Trust Anchor
            use base64::Engine;
            let public_key_bytes =
                base64::prelude::BASE64_STANDARD.decode(&self.pinned_public_key)?;
            let signature_bytes =
                base64::prelude::BASE64_STANDARD.decode(&bundle_info.signature)?;

            let verifying_key = VerifyingKey::from_bytes(
                public_key_bytes
                    .as_slice()
                    .try_into()
                    .context("Invalid pinned public key length")?,
            )?;
            let signature = Signature::from_bytes(
                signature_bytes
                    .as_slice()
                    .try_into()
                    .context("Invalid signature length")?,
            );

            let payload_string = serde_json::to_string(&bundle_info.payload)?;

            if verifying_key
                .verify(payload_string.as_bytes(), &signature)
                .is_ok()
            {
                artifact.state = ArtifactState::SignatureVerified;
                info!(
                    "[BundleSync] State: {:?} - Signature valid!",
                    artifact.state
                );
                Some(bundle_info.payload)
            } else {
                error!("[BundleSync] Signature verification failed! Discarding bundle.");
                None
            }
        } else {
            warn!("[BundleSync] No updates available or cloud is unreachable. Proceeding with just config.");
            None
        };

        // 3. Merge Policies
        let mut merged_payload = json!({});

        // Base from config
        if let Some(policy_config) = &dek_config.policy_config {
            if let Ok(config_val) = serde_json::to_value(policy_config) {
                if let Some(obj) = config_val.as_object() {
                    for (k, v) in obj {
                        merged_payload[k] = v.clone();
                    }
                }
            }
        }

        // Override with bundle
        if let Some(bundle) = bundle_payload {
            if let Some(obj) = bundle.as_object() {
                for (k, v) in obj {
                    merged_payload[k] = v.clone();
                }
            }
        }

        // Also inject tenant_id and device_id for proxy to use
        merged_payload["tenant_id"] = json!(dek_config.tenant_id);
        merged_payload["device_id"] = json!(dek_config.device_id);

        if let Some(spire_config) = &dek_config.spire_server {
            match dek_spire_node::SpireNodeAgent::new(&spire_config.endpoint, &dek_config.mtls) {
                Ok(agent) => match agent.attest_and_fetch_svid(&dek_config.device_id).await {
                    Ok(spiffe_id) => {
                        merged_payload["spiffe_id"] = json!(spiffe_id);
                    }
                    Err(e) => {
                        warn!("[BundleSync] Failed to attest with SPIRE server: {}", e);
                    }
                },
                Err(e) => {
                    warn!("[BundleSync] Failed to initialize SPIRE Node Agent: {}", e);
                }
            }
        }

        // 4. Staging
        let target_dir = PathBuf::from("target");
        fs::create_dir_all(&target_dir)?;
        let active_bundle_path = target_dir.join("active_bundle.json");

        let payload_string = serde_json::to_string_pretty(&merged_payload)?;

        // Write to temporary file and rename for atomic update
        let tmp_path = target_dir.join("active_bundle.tmp.json");
        fs::write(&tmp_path, payload_string)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600));
        }
        fs::rename(&tmp_path, &active_bundle_path)?;

        info!(
            "[BundleSync] State: {:?} - Staged combined config atomically to {:?}",
            ArtifactState::Staged,
            active_bundle_path
        );

        // Activation is implicit via dek-mcp-proxy's notify watcher
        info!(
            "[BundleSync] State: {:?} - Pipeline complete. Proxy will activate implicitly.",
            ArtifactState::Active
        );

        Ok(dek_config)
    }
}
