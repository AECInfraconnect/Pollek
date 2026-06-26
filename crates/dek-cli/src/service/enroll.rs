// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! enroll.rs — `dekctl enroll`
//!
//! First-run onboarding. Orchestrates:
//!   1. dek-enroll device flow (user approves in a browser on any device)
//!   2. join-token node attestation -> X.509-SVID (key never leaves the device)
//!   3. write certs (client.crt/.key, root_ca.crt) + bootstrap.json (remembers
//!      cloud_url + spiffe_id + tenant_id) + store the key in the OS keystore
//!
//! After this, every subsequent boot uses the SVID for mTLS with no env vars.
//!
//! Register in dek-cli:  Commands::Enroll { cloud_url: String }
//!   Commands::Enroll { cloud_url } => enroll::run(&cloud_url).await?,

use anyhow::{Context, Result};
use dek_enroll::{EnrollClient, UserPrompt};
use std::path::Path;
use tracing::info;

const CLIENT_ID: &str = "pollek-dek";
const SCOPE: &str = "dek.enroll";

pub async fn run(cloud_url: &str) -> Result<()> {
    let config_dir = dek_config::paths::get_config_dir();
    let certs_dir = config_dir.join("certs");
    std::fs::create_dir_all(&certs_dir).context("create certs dir")?;

    // Idempotency guard: don't clobber an existing identity by accident.
    let bootstrap_path = dek_config::paths::get_bootstrap_path();
    if bootstrap_path.exists() {
        anyhow::bail!(
            "bootstrap already exists at {:?}. Remove it (and certs/) to re-enroll.",
            bootstrap_path
        );
    }

    // 1) Device flow.
    println!("Enrolling this device with Pollek Cloud at {cloud_url}...\n");
    let ca_pem = if cloud_url.contains("127.0.0.1") || cloud_url.contains("localhost") {
        // Automatically load test CA for mock cloud
        std::fs::read_to_string("certs/root_ca.crt").ok()
    } else {
        None
    };
    let client = EnrollClient::new(cloud_url, CLIENT_ID, SCOPE, ca_pem.as_deref())?;
    let enrollment = client
        .run(|p: &UserPrompt| {
            println!("──────────────────────────────────────────────");
            let url = p.verification_uri_complete.as_deref().unwrap_or(&p.verification_uri);
            println!("  Open: {}", url);
            println!("  Enter code: {}", p.user_code);
            println!("  (expires in {}s)", p.expires_in);
            println!("──────────────────────────────────────────────\n");

            if webbrowser::open(url).is_ok() {
                println!("(Opened browser automatically. If it didn't open, please click the link above.)");
            }
        })
        .await
        .map_err(|e| anyhow::Error::new(e.into_envelope()))
        .context("device-flow enrollment failed")?;

    info!(tenant = %enrollment.tenant_id, device = %enrollment.device_id, "enrollment approved");

    // 2) Join-token node attestation -> X.509-SVID.
    let svid = dek_spire_node::attest_with_join_token(
        &enrollment.spire_endpoint,
        &enrollment.join_token,
        &enrollment.device_id,
        &enrollment.trust_bundle_pem,
    )
    .await
    .context("SPIRE join-token attestation failed")?;

    // 3) Persist identity material.
    let client_cert_path = certs_dir.join("client.crt");
    let client_key_path = certs_dir.join("client.key");
    let root_ca_path = certs_dir.join("root_ca.crt");

    write_secret(&client_key_path, svid.key_pem.as_bytes()).context("write client key")?;
    std::fs::write(&client_cert_path, &svid.cert_pem).context("write client cert")?;
    std::fs::write(&root_ca_path, &svid.trust_bundle_pem).context("write trust bundle")?;

    // Best-effort: stash the key in the OS keystore now (next boot's
    // keystore_migration will also handle this, but doing it here closes the
    // plaintext-on-disk window sooner).
    let ks = dek_keystore::get_keystore();
    if let Err(e) = ks.store_key("mtls_client_key", svid.key_pem.as_bytes()) {
        tracing::warn!("could not store key in OS keystore now (will migrate on next boot): {e}");
    }
    let _ = ks.store_key(
        "pinned_bundle_public_key",
        enrollment.pinned_bundle_public_key.as_bytes(),
    );

    // Parse trust domain from SPIFFE ID if possible (e.g. spiffe://tenant.example/...)
    let trust_domain = svid
        .spiffe_id
        .strip_prefix("spiffe://")
        .and_then(|s| s.split('/').next())
        .unwrap_or("unknown");

    // 4) Write bootstrap.json — remembers cloud_url + identity. No env vars needed.
    let bootstrap = serde_json::json!({
        "bootstrap_version": "1.0",
        "device_id": enrollment.device_id,
        "cloud_url": enrollment.cloud_url,
        "tenant_id": enrollment.tenant_id,
        "spiffe_id": svid.spiffe_id,
        "trust_domain": trust_domain,
        "pinned_bundle_public_key": enrollment.pinned_bundle_public_key.clone(),
        "pinned_root_key_id": enrollment.pinned_bundle_public_key,
        "root_ca_fingerprint": "computed-at-runtime", // We could hash the PEM here
        "mtls": {
            "client_cert_path": client_cert_path.to_string_lossy(),
            "client_key_path":  client_key_path.to_string_lossy(),
            "root_ca_path":     root_ca_path.to_string_lossy(),
        }
    });
    if let Some(parent) = bootstrap_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&bootstrap_path, serde_json::to_string_pretty(&bootstrap)?)
        .context("write bootstrap.json")?;
    set_owner_only(&bootstrap_path);

    println!("\n✓ Enrolled successfully.");
    println!("  SPIFFE ID : {}", svid.spiffe_id);
    println!("  Tenant    : {}", enrollment.tenant_id);
    println!("  Cloud URL : {}", enrollment.cloud_url);
    println!("  Config    : {}", bootstrap_path.display());
    println!("\nStart the service:  dekctl service start   (or: systemctl start pollek-dek)");
    Ok(())
}

/// Write a file with 0600 perms (owner-only). On Windows, relies on the
/// ProgramData ACL (tighten via icacls in the installer).
fn write_secret(path: &Path, data: &[u8]) -> Result<()> {
    std::fs::write(path, data)?;
    set_owner_only(path);
    Ok(())
}

fn set_owner_only(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path; // installer applies ACLs on Windows
    }
}
