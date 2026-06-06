use anyhow::{Context, Result};
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reqwest::Client;
use std::fs;
use tracing::{info, error};

pub async fn run_update(
    client: &Client,
    download_url: &str,
    signature_b64: &str,
    pinned_public_key_b64: &str,
) -> Result<()> {
    info!("Starting two-phase health-gated binary update...");

    let public_key_bytes = base64::prelude::BASE64_STANDARD.decode(pinned_public_key_b64)
        .context("Failed to decode pinned public key")?;
    let signature_bytes = base64::prelude::BASE64_STANDARD.decode(signature_b64)
        .context("Failed to decode signature")?;

    let verifying_key = VerifyingKey::from_bytes(
        public_key_bytes
            .as_slice()
            .try_into()
            .context("Invalid public key length")?,
    )
    .context("Invalid public key format")?;

    let signature = Signature::from_bytes(
        signature_bytes
            .as_slice()
            .try_into()
            .context("Invalid signature length")?,
    );

    // Phase 1: Download to temporary file
    info!("Downloading new binary from {}", download_url);
    let response = client.get(download_url).send().await?.error_for_status()?;
    let binary_data = response.bytes().await?;

    // Verify signature
    info!("Verifying binary signature...");
    if verifying_key.verify(&binary_data, &signature).is_err() {
        error!("Signature verification failed for the downloaded binary.");
        return Err(anyhow::anyhow!("Signature verification failed. Aborting update."));
    }
    info!("Signature verified successfully.");

    // Write to a temporary file for self_replace
    let temp_path = std::env::temp_dir().join(format!("dek-core-update-{}", uuid::Uuid::new_v4()));
    fs::write(&temp_path, &binary_data)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o755)).ok();
    }

    // Phase 2: Perform self-replace
    info!("Applying update via self-replace...");
    match self_replace::self_replace(&temp_path) {
        Ok(_) => {
            info!("Binary replaced successfully. The new version will run on next restart.");
            let _ = fs::remove_file(&temp_path);
            
            // Wait for 2 seconds to allow logs to flush
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            
            // Optionally, we could exit here to trigger a restart via service manager
            // std::process::exit(0);
        }
        Err(e) => {
            error!("Self-replace failed: {}", e);
            let _ = fs::remove_file(&temp_path);
            return Err(e.into());
        }
    }

    Ok(())
}
