// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::{Context, Result};

pub fn run(wipe_local_secrets: bool) -> Result<()> {
    let bootstrap_path = dek_config::paths::get_bootstrap_path();
    let config_dir = dek_config::paths::get_config_dir();
    let certs_dir = config_dir.join("certs");

    println!("Unenrolling Pollek DEK...");

    let mut removed_something = false;

    if bootstrap_path.exists() {
        std::fs::remove_file(&bootstrap_path).context("remove bootstrap.json")?;
        println!("✓ Removed bootstrap config.");
        removed_something = true;
    }

    if certs_dir.exists() {
        std::fs::remove_dir_all(&certs_dir).context("remove certs directory")?;
        println!("✓ Removed identity certificates and private key files.");
        removed_something = true;
    }

    if wipe_local_secrets {
        let ks = dek_keystore::get_keystore();
        let _ = ks.delete_key("mtls_client_key");
        let _ = ks.delete_key("pinned_bundle_public_key");
        println!("✓ Wiped local secrets from secure keystore.");
        removed_something = true;
    }

    if !removed_something {
        println!("Device was not enrolled. Nothing to do.");
    } else {
        println!("\nUnenrollment complete.");
        println!("DEK Core will no longer be able to connect to Pollek Cloud.");
        println!("Restart the service for changes to take effect.");
    }

    Ok(())
}
