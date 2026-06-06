use anyhow::{Context, Result};
use dek_config::MtlsConfig;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpireAttestRequest {
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpireAttestResponse {
    pub spiffe_id: String,
}

pub struct SpireNodeAgent {
    endpoint: String,
    mtls_client: reqwest::Client,
}

impl SpireNodeAgent {
    pub fn new(endpoint: &str, mtls: &MtlsConfig) -> Result<Self> {
        let mtls_client = mtls
            .build_client(None)
            .context("Failed to build mTLS client for SPIRE Node Agent")?;
        Ok(Self {
            endpoint: endpoint.to_string(),
            mtls_client,
        })
    }

    pub async fn attest_and_fetch_svid(&self, device_id: &str) -> Result<String> {
        let url = format!("{}/node/attest", self.endpoint);
        info!("Attesting node to SPIRE Server at {}", url);

        let req_body = SpireAttestRequest {
            device_id: device_id.to_string(),
        };

        let res = self.mtls_client.post(&url).json(&req_body).send().await?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            warn!("Failed to attest node. Status: {}, Body: {}", status, text);
            anyhow::bail!("SPIRE node attestation failed: {}", status);
        }

        let resp: SpireAttestResponse = res.json().await?;
        info!("Successfully attested. Received SVID: {}", resp.spiffe_id);

        Ok(resp.spiffe_id)
    }
}
