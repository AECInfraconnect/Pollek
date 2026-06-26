// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! rollback.rs — `dekctl rollback`
//!
//! Restores the previous dek-core binary from the `.bak` left by the updater,
//! using the probation marker (`update_pending.json`) as the source of truth.
//!
//! This runs when the service is STOPPED (systemd `OnFailure=dek-rollback.service`
//! or Windows SCM "run program" failure action), so the target binary is not
//! locked and a plain copy works on every OS. After restoring it best-effort
//! restarts the service.
//!
//! Register in dek-cli's command enum, e.g.:
//!     #[derive(Subcommand)]
//!     enum Commands { Health, Status, Reload, Rollback, Service { .. } }
//! and in the dispatcher:
//!     Commands::Rollback => rollback::run()?,

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Debug, Deserialize)]
struct ProbationMarker {
    #[serde(default)]
    backup_path: String,
    /// Optional explicit target; if absent we derive it from backup_path.
    #[serde(default)]
    target_path: Option<String>,
    #[serde(default)]
    target_version: String,
}

pub fn run() -> Result<()> {
    let config_dir = dek_config::paths::get_config_dir();
    let marker_path = config_dir.join("update_pending.json");

    if !marker_path.exists() {
        info!("No probation marker found — nothing to roll back.");
        return Ok(());
    }

    let raw = std::fs::read_to_string(&marker_path).context("read probation marker")?;
    let marker: ProbationMarker = serde_json::from_str(&raw).context("parse probation marker")?;

    if marker.backup_path.is_empty() {
        // Remove the useless marker so we don't loop, then fail loudly.
        let _ = std::fs::remove_file(&marker_path);
        bail!("probation marker has no backup_path; cannot roll back");
    }

    let backup = PathBuf::from(&marker.backup_path);
    if !backup.exists() {
        let _ = std::fs::remove_file(&marker_path);
        bail!("backup binary {:?} is missing; cannot roll back", backup);
    }

    let target = match &marker.target_path {
        Some(t) if !t.is_empty() => PathBuf::from(t),
        _ => derive_target(&backup),
    };

    info!(
        "Rolling back: restoring {:?} -> {:?} (failed version: {})",
        backup, target, marker.target_version
    );

    // Atomic-ish: write to a temp next to target, then rename over it.
    let tmp = target.with_extension("rollback-tmp");
    std::fs::copy(&backup, &tmp).with_context(|| format!("copy {:?} -> {:?}", backup, tmp))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755));
    }
    std::fs::rename(&tmp, &target).with_context(|| format!("rename {:?} -> {:?}", tmp, target))?;

    // Cleanup: drop backup + marker so the restored binary boots normally.
    let _ = std::fs::remove_file(&backup);
    let _ = std::fs::remove_file(&marker_path);

    info!("Rollback complete. Restarting service (best-effort)...");
    if let Err(e) = restart_service() {
        warn!("Could not auto-restart service: {e}. The service manager should restart it per its policy.");
    }

    Ok(())
}

/// Derive the original binary path from the backup path in an OS-correct way.
/// updater used `exe.with_extension("bak")`, so:
///   - Windows: `dek-core.bak` -> `dek-core.exe`
///   - Unix:    `dek-core.bak` -> `dek-core`
fn derive_target(backup: &Path) -> PathBuf {
    backup.with_extension(std::env::consts::EXE_EXTENSION)
}

#[cfg(target_os = "linux")]
fn restart_service() -> Result<()> {
    // absolute path to avoid PATH hijack (P2 security note)
    let status = std::process::Command::new("/usr/bin/systemctl")
        .args(["restart", "pollek-dek"])
        .status()
        .context("spawn systemctl")?;
    if !status.success() {
        tracing::error!("systemctl restart returned {:?}", status.code());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn restart_service() -> Result<()> {
    // sc.exe is in System32 (on PATH for services); use explicit verb sequence.
    let _ = std::process::Command::new("sc")
        .args(["start", "PollekDEK"])
        .status();
    Ok(())
}

#[cfg(target_os = "macos")]
fn restart_service() -> Result<()> {
    let _ = std::process::Command::new("/bin/launchctl")
        .args(["kickstart", "-k", "system/com.pollek.dek"])
        .status();
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn restart_service() -> Result<()> {
    Ok(())
}
