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

    // Phase 2: Perform self-replace with A/B Rollback Support
    info!("Preparing A/B Update: backing up current binary...");
    let exe_path = std::env::current_exe()?;
    let backup_path = exe_path.with_extension("bak");
    
    // Copy current executable to backup
    if let Err(e) = fs::copy(&exe_path, &backup_path) {
        error!("Failed to create backup at {:?}: {}", backup_path, e);
        let _ = fs::remove_file(&temp_path);
        return Err(e.into());
    }

    info!("Applying update via self-replace...");
    match self_replace::self_replace(&temp_path) {
        Ok(_) => {
            let _ = fs::remove_file(&temp_path);
            
            // Phase 3: Write Probation Marker File
            let config_dir = dek_config::paths::get_config_dir();
            let marker_path = config_dir.join("update_pending.json");
            
            let marker_data = serde_json::json!({
                "target_version": "pending",
                "backup_path": backup_path.to_string_lossy().to_string(),
                "target_path": exe_path.to_string_lossy().to_string(),
                "timestamp": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
            });
            
            fs::write(&marker_path, serde_json::to_string_pretty(&marker_data)?)?;
            info!("Marker written to {:?}. Update staged.", marker_path);

            // Phase 4: Request Service Restart
            info!("Requesting service manager to restart pollen-dek...");
            
            // Wait briefly to allow logs to flush
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            
            #[cfg(windows)]
            {
                // Trigger an asynchronous service restart by spawning a detached powershell script
                // We use Start-Process to decouple from the dying process tree
                let script = "Start-Sleep -Seconds 2; Restart-Service -Name PollenDEK -Force".to_string();
                // Use absolute path to prevent PATH hijacking
                let powershell_path = "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe";
                let _ = std::process::Command::new(powershell_path)
                    .args(["-Command", &script])
                    .spawn();
            }
            #[cfg(unix)]
            {
                // Trigger an asynchronous service restart via systemctl (use absolute path to prevent PATH hijack)
                let _ = std::process::Command::new("/usr/bin/systemctl")
                    .args(&["restart", "pollen-dek"])
                    .spawn();
            }

            // Exit cleanly so the service manager can immediately begin the restart process
            std::process::exit(0);
        }
        Err(e) => {
            error!("Self-replace failed: {}", e);
            let _ = fs::remove_file(&temp_path);
            let _ = fs::remove_file(&backup_path); // Cleanup backup on failure
            Err(e.into())
        }
    }
}
