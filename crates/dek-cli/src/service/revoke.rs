// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::Result;
use tracing::{error, info};

pub async fn run(reason: &str) -> Result<()> {
    let bootstrap_path = dek_config::paths::get_bootstrap_path();
    if !bootstrap_path.exists() {
        anyhow::bail!("Device is not enrolled.");
    }

    let config: dek_config::BootstrapConfig =
        serde_json::from_str(&std::fs::read_to_string(&bootstrap_path)?)?;

    info!("Revoking device identity with Pollek Cloud...");
    let url = format!(
        "{}/v1/tenants/{}/devices/{}/revoke",
        config.cloud_url,
        config.tenant_id.unwrap_or_default(),
        config.device_id
    );

    // Get current mTLS client
    let certs_dir = dek_config::paths::get_config_dir().join("certs");
    let client_cert = certs_dir.join("client.crt");
    let client_key = certs_dir.join("client.key");
    let root_ca = certs_dir.join("root_ca.crt");

    let mut identity_buf = std::fs::read(&client_key).unwrap_or_default();
    identity_buf.extend_from_slice(b"\n");
    identity_buf.extend_from_slice(&std::fs::read(&client_cert).unwrap_or_default());
    let req_id = reqwest::Identity::from_pem(&identity_buf)?;

    let ca_cert = reqwest::Certificate::from_pem(&std::fs::read(&root_ca).unwrap_or_default())?;

    let client = reqwest::Client::builder()
        .identity(req_id)
        .add_root_certificate(ca_cert)
        .build()?;

    let payload = serde_json::json!({
        "reason": reason
    });

    let resp = client.post(&url).json(&payload).send().await?;

    if resp.status().is_success() {
        println!("✓ Cloud revocation successful.");
    } else {
        error!("Cloud revocation failed: HTTP {}", resp.status());
        println!("Proceeding to unenroll locally anyway...");
    }

    crate::service::unenroll::run(true)?;

    Ok(())
}
