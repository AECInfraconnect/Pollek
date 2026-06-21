// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! probation.rs — health-gated A/B update verification for dek-core.
//!
//! Replaces the previous logic in `main.rs` (a detached `sleep(15s)` that
//! committed unconditionally and never rolled back). This module turns
//! "unhealthy" into an actionable outcome:
//!
//!   detect() -> Some(marker)   (a staged update is on probation)
//!        |
//!   finalize():  loop until deadline {
//!        health_probe() && mTLS-to-cloud OK && active_bundle parses
//!        -> need N consecutive passes -> COMMIT (drop marker + .bak)
//!   }  on deadline  -> ABORT: restore .bak via self_replace, exit(1)
//!                       so the service manager restarts the *old* binary.
//!
//! Philosophy: fail predictably, never silently. A broken-but-not-crashing
//! new binary (e.g. can't reach cloud, can't load bundle) is rolled back
//! instead of being committed.

use dek_config::BootstrapConfig;
use serde::Deserialize;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

/// Written by the updater (`updater.rs`) when an update is staged.
#[derive(Debug, Clone, Deserialize)]
pub struct ProbationMarker {
    /// Absolute path to the backed-up previous binary (`dek-core.bak`).
    #[serde(default)]
    pub backup_path: String,
    #[serde(default)]
    pub target_version: String,
}

#[derive(Debug, Clone)]
pub struct ProbationSettings {
    /// Total wall-clock budget for the new binary to prove itself.
    pub deadline: Duration,
    /// Gap between checks.
    pub interval: Duration,
    /// Consecutive all-green checks required before committing.
    pub required_successes: u32,
}

impl Default for ProbationSettings {
    fn default() -> Self {
        Self {
            deadline: Duration::from_secs(60),
            interval: Duration::from_secs(3),
            required_successes: 3,
        }
    }
}

pub fn marker_path(config_dir: &Path) -> PathBuf {
    config_dir.join("update_pending.json")
}

/// Detect a pending probation marker. Returns `None` for a normal boot.
pub fn detect(config_dir: &Path) -> Option<ProbationMarker> {
    let p = marker_path(config_dir);
    if !p.exists() {
        return None;
    }
    match std::fs::read_to_string(&p) {
        Ok(s) => {
            match serde_json::from_str::<ProbationMarker>(&s) {
                Ok(m) => Some(m),
                Err(e) => {
                    // Marker exists but is corrupt: still treat as pending so we don't
                    // accidentally "commit" a possibly-broken binary, but we have no
                    // backup path to roll back to. Surface loudly.
                    warn!("probation marker present but unparsable ({e}); cannot roll back automatically");
                    Some(ProbationMarker::default_corrupt())
                }
            }
        }
        Err(e) => {
            warn!("probation marker present but unreadable ({e})");
            Some(ProbationMarker::default_corrupt())
        }
    }
}

impl ProbationMarker {
    fn default_corrupt() -> Self {
        Self {
            backup_path: String::new(),
            target_version: "unknown".into(),
        }
    }
}

/// Run the probation loop to completion. Call this AFTER core services
/// (IPC server, proxy, bundle-sync) have been started, so the health checks
/// reflect a fully-running service.
///
/// `health_probe` is injected so this module stays decoupled from the IPC
/// schema; `main.rs` supplies a probe that round-trips the local IPC/health
/// endpoint (see the wiring snippet).
///
/// This function either returns (commit path) or terminates the process
/// (abort path) — it never returns on rollback.
pub async fn finalize<H, Fut>(
    config_dir: PathBuf,
    cloud_url: String,
    bootstrap: BootstrapConfig,
    active_bundle_path: PathBuf,
    settings: ProbationSettings,
    marker: ProbationMarker,
    health_probe: H,
) where
    H: Fn() -> Fut,
    Fut: Future<Output = bool>,
{
    info!(
        target_version = %marker.target_version,
        deadline_s = settings.deadline.as_secs(),
        "A/B probation started: verifying new binary before commit"
    );

    let start = Instant::now();
    let mut streak: u32 = 0;

    loop {
        if start.elapsed() > settings.deadline {
            error!(
                "probation deadline ({}s) exceeded without {} consecutive healthy checks; ABORTING",
                settings.deadline.as_secs(),
                settings.required_successes
            );
            abort_and_rollback(&config_dir, &marker); // -> never returns
        }

        let health = health_probe().await;
        let mtls = check_mtls(&bootstrap, &cloud_url).await;
        let bundle = check_bundle(&active_bundle_path);

        if health && mtls && bundle {
            streak += 1;
            info!(
                streak,
                "probation check passed ({}/{})", streak, settings.required_successes
            );
            if streak >= settings.required_successes {
                commit(&config_dir, &marker);
                info!("probation PASSED — update committed");
                return;
            }
        } else {
            if streak > 0 {
                warn!("probation streak reset after a failing check");
            }
            streak = 0;
            warn!(health, mtls, bundle, "probation check failed");
        }

        tokio::time::sleep(settings.interval).await;
    }
}

/// mTLS reachability to Pollen Cloud, reusing the existing client builder.
async fn check_mtls(bootstrap: &BootstrapConfig, cloud_url: &str) -> bool {
    let client = match bootstrap.mtls.build_client(None) {
        Ok(c) => c,
        Err(e) => {
            warn!("probation: failed to build mTLS client: {e}");
            return false;
        }
    };
    let url = format!("{}/health", cloud_url.trim_end_matches('/'));
    match client.get(&url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(e) => {
            warn!("probation: mTLS health request failed: {e}");
            false
        }
    }
}

/// The staged bundle must be present and parse as a non-empty JSON object.
fn check_bundle(path: &Path) -> bool {
    let data = match std::fs::read_to_string(path) {
        Ok(d) => d,
        Err(e) => {
            warn!("probation: active_bundle unreadable at {path:?}: {e}");
            return false;
        }
    };
    match serde_json::from_str::<serde_json::Value>(&data) {
        Ok(v) => v.as_object().map(|o| !o.is_empty()).unwrap_or(false),
        Err(e) => {
            warn!("probation: active_bundle parse failed: {e}");
            false
        }
    }
}

/// Success: drop the marker and the now-unneeded backup.
fn commit(config_dir: &Path, marker: &ProbationMarker) {
    let mp = marker_path(config_dir);
    if let Err(e) = std::fs::remove_file(&mp) {
        warn!("probation: failed to remove marker {mp:?}: {e}");
    }
    if !marker.backup_path.is_empty() {
        if let Err(e) = std::fs::remove_file(&marker.backup_path) {
            warn!(
                "probation: failed to remove backup {}: {e}",
                marker.backup_path
            );
        }
    }
}

/// Failure: restore the previous binary and exit non-zero so the service
/// manager restarts the (now-restored) old version. Never returns.
fn abort_and_rollback(config_dir: &Path, marker: &ProbationMarker) -> ! {
    // Remove the marker FIRST so the restored binary boots normally and does
    // not re-enter probation.
    let mp = marker_path(config_dir);
    let _ = std::fs::remove_file(&mp);

    if marker.backup_path.is_empty() {
        error!("probation: no backup recorded — cannot roll back. Exiting non-zero for service-manager recovery.");
        std::process::exit(1);
    }

    let backup = PathBuf::from(&marker.backup_path);
    if !backup.exists() {
        error!(
            "probation: backup {} is missing — cannot roll back. Exiting non-zero.",
            marker.backup_path
        );
        std::process::exit(1);
    }

    // self_replace handles the Windows "can't overwrite a running exe" case by
    // renaming the current image out of the way first.
    match self_replace::self_replace(&backup) {
        Ok(_) => {
            let _ = std::fs::remove_file(&backup);
            error!("probation: rolled back to previous binary; exiting non-zero to restart the restored version.");
            std::process::exit(1);
        }
        Err(e) => {
            // Last-resort: the new binary stays in place. Exit non-zero anyway;
            // systemd StartLimit* / SCM recovery limits will contain the loop
            // and stop flapping, surfacing the failure to operators.
            error!("probation: CRITICAL rollback failed ({e}); exiting non-zero. Service-manager restart limits will contain the crash loop.");
            std::process::exit(1);
        }
    }
}
