// SPDX-License-Identifier: Apache-2.0
//! profile.rs — switch the DEK control-plane profile between Local and Cloud (L5).
//!
//! The DEK speaks ONE protocol/contract to both a Local control plane and Pollek
//! Cloud. Switching targets means rewriting `bootstrap.json` (cloud_url +
//! tenant_id) and pointing the trust store at the right signing key — nothing in
//! the DEK's enforcement code changes (invariant I1). After switching, the user
//! re-enrolls (or reuses certs) and the DEK syncs bundles from the new target.

use anyhow::{Context, Result};
use dek_config::{paths, BootstrapConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileMode {
    Local,
    Cloud,
    Sovereign,
}

impl std::str::FromStr for ProfileMode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "local" => Ok(ProfileMode::Local),
            "cloud" => Ok(ProfileMode::Cloud),
            "sovereign" => Ok(ProfileMode::Sovereign),
            other => anyhow::bail!(
                "unknown profile mode '{other}' (expected 'local', 'cloud', or 'sovereign')"
            ),
        }
    }
}

const DEFAULT_LOCAL_URL: &str = "http://127.0.0.1:3000";

/// Set the active control-plane profile. For Local, `tenant_id` is forced to
/// "local" and the local control plane's signing key is pinned as trust root.
pub fn set_profile(
    mode: ProfileMode,
    url: Option<String>,
    tenant_id: Option<String>,
    trusted_key_b64: Option<String>,
    token: Option<String>,
) -> Result<()> {
    let bootstrap_path = paths::get_bootstrap_path();
    let path_str = bootstrap_path.to_string_lossy().into_owned();
    let mut cfg = BootstrapConfig::load_or_default(&path_str).context("load bootstrap.json")?;

    match mode {
        ProfileMode::Local => {
            cfg.cloud_url = url.unwrap_or_else(|| DEFAULT_LOCAL_URL.to_string());
            cfg.tenant_id = Some("local".to_string());
            if let Some(key) = trusted_key_b64 {
                // pin the local control plane's bundle signing key as trust root
                cfg.pinned_bundle_public_key = key;
            } else {
                eprintln!(
                    "warning: no --trusted-key provided. Fetch it from the local control plane:\n  \
                     curl {}/v1/tenants/local/devices/_/trusted-keys\n  \
                     then re-run with --trusted-key <public_b64>",
                    cfg.cloud_url
                );
            }
        }
        ProfileMode::Sovereign => {
            cfg.cloud_url = "http://127.0.0.1:0".to_string(); // disable network
            cfg.tenant_id = Some("sovereign".to_string());
            if let Some(key) = trusted_key_b64 {
                cfg.pinned_bundle_public_key = key;
            }
            eprintln!("info: Running in sovereign mode. Cloud egress is completely blocked.");
        }
        ProfileMode::Cloud => {
            let cloud_url = url.context("--url is required for cloud profile")?;
            cfg.cloud_url = cloud_url;
            cfg.tenant_id = tenant_id; // cloud is multi-tenant; tenant supplied by operator
            if let Some(key) = trusted_key_b64 {
                cfg.pinned_bundle_public_key = key;
            }
            // Cloud trust root is normally seeded during enrollment / trusted-keys
            // fetch; we keep the existing pinned key if none provided.
        }
    }

    if let Some(t) = token {
        cfg.local_api_token = Some(t);
    }

    let json = serde_json::to_string_pretty(&cfg)?;
    if let Some(parent) = bootstrap_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&bootstrap_path, json).context("write bootstrap.json")?;
    println!(
        "DEBUG DEK-CLI: Wrote profile to {}, key is: {}",
        bootstrap_path.display(),
        cfg.pinned_bundle_public_key
    );
    println!("Switched to {:?} profile.", mode);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&bootstrap_path, std::fs::Permissions::from_mode(0o600));
    }

    println!(
        "Profile set: mode={:?} cloud_url={} tenant_id={}",
        mode,
        cfg.cloud_url,
        cfg.tenant_id.as_deref().unwrap_or("(none)")
    );
    println!(
        "Next: run `dek-cli enroll --cloud-url {}` then restart the DEK service.",
        cfg.cloud_url
    );
    Ok(())
}

/// Print the current profile.
pub fn show_profile() -> Result<()> {
    let path_str = paths::get_bootstrap_path().to_string_lossy().into_owned();
    let cfg = BootstrapConfig::load_or_default(&path_str)?;
    let mode = match cfg.tenant_id.as_deref() {
        Some("sovereign") => "sovereign",
        Some("local") => "local",
        _ if cfg.cloud_url.contains("127.0.0.1") || cfg.cloud_url.contains("localhost") => "local",
        _ => "cloud",
    };
    println!("mode:       {mode}");
    println!("cloud_url:  {}", cfg.cloud_url);
    println!(
        "tenant_id:  {}",
        cfg.tenant_id.as_deref().unwrap_or("(none)")
    );
    println!("device_id:  {}", cfg.device_id);
    println!("trust_key:  {}", cfg.pinned_bundle_public_key);
    Ok(())
}
