// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::print_stdout, clippy::print_stderr)]
// Copyright (c) 2026 AEC Infraconnect

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::Archive;

mod github;

use std::process::Command;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "dek-updater", about = "Pollen DEK Auto-Updater")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check for updates
    Check {
        #[arg(long, default_value = "stable")]
        channel: String,
    },
    /// Verify an executable against TUF metadata
    Verify {
        target_exe: PathBuf,
        metadata_path: PathBuf,
    },
    /// Perform the update by replacing the current executable
    Update {
        target_exe: PathBuf,
        new_exe: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    /// Upgrade to the latest release
    Upgrade {
        #[arg(long, default_value = "stable")]
        channel: String,
        target_exe: PathBuf,
    },
    /// Fully automated manifest-driven update with rollback
    AutoUpdate {
        #[arg(long, default_value = "https://127.0.0.1:43891/v1/updater")]
        update_url: String,
        #[arg(long)]
        service_name: Option<String>,
        #[arg(long, default_value_t = 43890)]
        health_port: u16,
        target_exe: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { channel } => {
            println!("Checking for updates on channel: {}", channel);
            let current_version = env!("CARGO_PKG_VERSION");
            match github::latest_release(&channel) {
                Ok(release) => {
                    if github::is_newer(current_version, &release.tag_name)? {
                        println!("Update available: {}", release.tag_name);
                    } else {
                        println!(
                            "No updates available. Current version {} is up-to-date.",
                            current_version
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Failed to check for updates: {}", e);
                }
            }
        }
        Commands::Verify {
            target_exe,
            metadata_path,
        } => {
            let content =
                fs::read_to_string(&metadata_path).context("Failed to read TUF metadata")?;
            let metadata: serde_json::Value = serde_json::from_str(&content)?;
            verify_tuf_signature(&target_exe, &metadata)?;
            println!("TUF Verification Successful.");
        }
        Commands::Update {
            target_exe,
            new_exe,
            dry_run,
        } => {
            if !new_exe.exists() {
                anyhow::bail!("New executable not found: {:?}", new_exe);
            }
            if dry_run {
                println!("DRY RUN: Would replace {:?} with {:?}", target_exe, new_exe);
                return Ok(());
            }

            apply_with_rollback(&target_exe, &new_exe)?;
            println!("Update successful. Please restart the service.");
        }
        Commands::Upgrade {
            channel,
            target_exe,
        } => {
            let current_version = env!("CARGO_PKG_VERSION");
            println!("Checking for updates on channel: {}", channel);
            let release = github::latest_release(&channel)?;

            if !github::is_newer(current_version, &release.tag_name)? {
                println!("Already up to date (version {}).", current_version);
                return Ok(());
            }

            println!("Update found: {}. Downloading...", release.tag_name);
            let temp_dir = tempfile::tempdir()?;

            let (archive_path, sum_path, sig_path, pem_path) =
                github::download_update(&release, temp_dir.path())?;
            println!("Downloaded artifacts. Verifying signatures...");

            github::verify_all(&archive_path, &sum_path, &sig_path, &pem_path)?;
            println!("Signature verification passed. Extracting...");

            let extracted_exe = extract_binary(&archive_path, temp_dir.path())?;

            println!("Applying update...");
            apply_with_rollback(&target_exe, &extracted_exe)?;

            println!(
                "Upgrade successful to {}! Please restart the service.",
                release.tag_name
            );
        }
        Commands::AutoUpdate {
            update_url,
            service_name,
            health_port,
            target_exe,
        } => {
            println!("Starting manifest-driven Auto-Update from {}", update_url);

            // 1. Fetch TUF targets.json
            let metadata_url =
                format!("{}/metadata/targets.json", update_url.trim_end_matches('/'));
            let resp = ureq::get(&metadata_url)
                .call()
                .context("Failed to fetch TUF metadata")?;
            let metadata: serde_json::Value = resp.into_json()?;

            // Handle full TUF format { "signed": { "targets": ... } } or raw payload
            let signed = if metadata.get("signed").is_some() {
                &metadata["signed"]
            } else {
                &metadata
            };

            // Expected executable name
            let target_name = if cfg!(windows) {
                "dek-core.exe"
            } else {
                "dek-core"
            };

            let target_info = signed["targets"][target_name].as_object().ok_or_else(|| {
                anyhow::anyhow!("Target {} not found in TUF metadata", target_name)
            })?;

            let expected_hash = target_info["hashes"]["sha256"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing sha256 hash in metadata for target"))?;

            println!(
                "Update available for {}. Hash: {}",
                target_name, expected_hash
            );

            // 2. Download executable
            let artifact_url = format!(
                "{}/artifacts/{}",
                update_url.trim_end_matches('/'),
                expected_hash
            );
            let temp_dir = tempfile::tempdir()?;
            let downloaded_exe = temp_dir.path().join(target_name);

            println!("Downloading update payload...");
            let resp = ureq::get(&artifact_url)
                .call()
                .context("Failed to download payload")?;
            let mut reader = resp.into_reader();
            let mut file = fs::File::create(&downloaded_exe)?;
            std::io::copy(&mut reader, &mut file)?;

            // 3. Verify Hash
            verify_tuf_signature(&downloaded_exe, &metadata)?;
            println!("Payload verified successfully.");

            // 4. Service Stop
            if let Some(ref svc) = service_name {
                println!("Stopping service: {}", svc);
                manage_service(svc, "stop")?;
                std::thread::sleep(Duration::from_secs(2)); // wait for complete stop
            }

            // 5. Atomic Swap
            println!("Swapping executable...");
            if let Err(e) = apply_with_rollback(&target_exe, &downloaded_exe) {
                eprintln!("Swap failed: {}", e);
                if let Some(ref svc) = service_name {
                    let _ = manage_service(svc, "start");
                }
                anyhow::bail!("Swap failed.");
            }

            // 6. Service Start
            if let Some(ref svc) = service_name {
                println!("Starting service: {}", svc);
                manage_service(svc, "start")?;
            } else {
                println!("No service_name provided. Assuming manual restart.");
                return Ok(());
            }

            // 7. Health Check
            println!("Waiting for health check on port {}...", health_port);
            let health_url = format!("http://127.0.0.1:{}/v1/healthz", health_port);

            let mut is_healthy = false;
            for _ in 0..15 {
                std::thread::sleep(Duration::from_secs(2));
                match ureq::get(&health_url).call() {
                    Ok(res) if res.status() == 200 => {
                        is_healthy = true;
                        break;
                    }
                    _ => {}
                }
            }

            // 8. Rollback if unhealthy
            if !is_healthy {
                eprintln!("Health check failed! Rolling back...");
                if let Some(ref svc) = service_name {
                    let _ = manage_service(svc, "stop");
                    std::thread::sleep(Duration::from_secs(2));
                }

                let backup_exe = target_exe.with_extension("exe.bak");
                if backup_exe.exists() {
                    let _ = fs::rename(&backup_exe, &target_exe);
                }

                if let Some(ref svc) = service_name {
                    let _ = manage_service(svc, "start");
                }
                anyhow::bail!("Update aborted due to health check failure.");
            }

            println!("Auto-Update complete and healthy!");
        }
    }
    Ok(())
}

fn manage_service(name: &str, action: &str) -> Result<()> {
    #[cfg(windows)]
    {
        let status = Command::new("sc").arg(action).arg(name).status()?;
        if !status.success() {
            eprintln!("Warning: sc {} {} returned non-zero status", action, name);
        }
    }
    #[cfg(not(windows))]
    {
        // Simple fallback for linux using systemctl
        let status = Command::new("systemctl").arg(action).arg(name).status()?;
        if !status.success() {
            eprintln!(
                "Warning: systemctl {} {} returned non-zero status",
                action, name
            );
        }
    }
    Ok(())
}

fn apply_with_rollback(target_exe: &Path, new_exe: &Path) -> Result<()> {
    let backup_exe = target_exe.with_extension("exe.bak");
    if backup_exe.exists() {
        let _ = fs::remove_file(&backup_exe);
    }
    if target_exe.exists() {
        fs::rename(target_exe, &backup_exe).context("Failed to rename active executable")?;
    }
    if let Err(e) = fs::rename(new_exe, target_exe) {
        eprintln!("Failed to move new executable into place: {e}");
        if backup_exe.exists() {
            let _ = fs::rename(&backup_exe, target_exe);
        }
        anyhow::bail!("Update failed and rolled back");
    }
    Ok(())
}

fn extract_binary(archive_path: &Path, out_dir: &Path) -> Result<PathBuf> {
    let tar_gz = fs::File::open(archive_path)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);

    archive.unpack(out_dir)?;

    // Find the executable.
    // Wait, the tarball contains dek-core, dek-mcp-proxy, dek-ext-authz.
    // We are the updater updating dek-core? Let's assume dek-core for now.
    let exe_name = if cfg!(windows) {
        "dek-core.exe"
    } else {
        "dek-core"
    };
    let exe_path = out_dir.join(exe_name);

    if !exe_path.exists() {
        anyhow::bail!("Could not find {} inside the downloaded archive.", exe_name);
    }

    Ok(exe_path)
}

fn verify_tuf_signature(new_exe: &PathBuf, metadata: &serde_json::Value) -> Result<()> {
    // Handle full TUF format { "signed": { "targets": ... } } or raw payload
    let signed = if metadata.get("signed").is_some() {
        &metadata["signed"]
    } else {
        metadata
    };

    // 1. Check expiration
    #[allow(clippy::collapsible_if)]
    if let Some(expires_str) = signed["expires"].as_str() {
        if let Ok(expires_dt) = expires_str.parse::<DateTime<Utc>>() {
            if Utc::now() > expires_dt {
                anyhow::bail!("TUF metadata has expired on {}", expires_dt);
            }
        }
    }

    let target_name = new_exe
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let target_info = signed["targets"][&target_name]
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Target {} not found in TUF metadata", target_name))?;

    let expected_hash = target_info["hashes"]["sha256"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing sha256 hash in metadata for target"))?;
    let expected_size = target_info["length"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("Missing length in metadata for target"))?;

    // 2. Hash and Size check
    let mut file = fs::File::open(new_exe)?;
    let meta = file.metadata()?;
    if meta.len() != expected_size {
        anyhow::bail!(
            "File size mismatch: expected {}, got {}",
            expected_size,
            meta.len()
        );
    }

    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let actual_hash = hex::encode(hasher.finalize());

    if actual_hash != expected_hash {
        anyhow::bail!(
            "Hash mismatch: expected {}, got {}",
            expected_hash,
            actual_hash
        );
    }

    // 3. Platform checking (mock constraint)
    if let Some(platform) = target_info
        .get("custom")
        .and_then(|c| c.get("platform"))
        .and_then(|p| p.as_str())
    {
        let current_platform = if cfg!(windows) {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        };
        if !platform.contains(current_platform) {
            anyhow::bail!(
                "Platform mismatch: target is for {}, but we are on {}",
                platform,
                current_platform
            );
        }
    }

    Ok(())
}
