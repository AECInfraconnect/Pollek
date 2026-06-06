use dek_config::BootstrapConfig;
use dek_keystore::get_keystore;
use reqwest::{Certificate, Identity};
use std::fs;
use std::path::Path;
use tracing::{error, info, warn};

pub async fn run_migration(bootstrap: &BootstrapConfig, pollen_cloud_url: &str) -> bool {
    info!("Starting Keystore Migration and Validation...");
    let keystore = get_keystore();

    // 1. Check/Import client.key
    let client_key_alias = "mtls_client_key";
    let client_key_path = Path::new(&bootstrap.mtls.client_key_path);
    let mut key_in_keystore = keystore.load_key(client_key_alias).ok();

    if key_in_keystore.is_none() {
        if client_key_path.exists() {
            info!("client.key not in Keystore. Importing...");
            if let Ok(key_data) = fs::read(client_key_path) {
                if let Err(e) = keystore.store_key(client_key_alias, &key_data) {
                    error!("Failed to import client.key to keystore: {}. Falling back to file.", e);
                    return false;
                }
                key_in_keystore = Some(key_data);
            } else {
                error!("Failed to read client.key for import. Falling back.");
                return false;
            }
        } else {
            error!("client.key not found on disk and not in keystore. Cannot proceed with mTLS.");
            return false;
        }
    }

    // 2. Check/Import Pinned Bundle Public Key
    let bundle_key_alias = "pinned_bundle_public_key";
    let bundle_key_in_keystore = keystore.load_key(bundle_key_alias).ok();
    
    if bundle_key_in_keystore.is_none() {
        info!("pinned_bundle_public_key not in Keystore. Importing from bootstrap.json...");
        let pk_data = bootstrap.pinned_bundle_public_key.as_bytes();
        if let Err(e) = keystore.store_key(bundle_key_alias, pk_data) {
            error!("Failed to import pinned bundle public key to keystore: {}. Falling back to config.", e);
        } else {
            let _ = pk_data.to_vec(); // Just to avoid unused warning
        }
    }

    // 3. Verify by Use (mTLS Handshake)
    if let Some(key_data) = key_in_keystore {
        info!("Verifying keystore material with mTLS handshake to {}...", pollen_cloud_url);
        
        let root_ca_der = fs::read(&bootstrap.mtls.root_ca_path).unwrap_or_default();
        let client_cert = fs::read(&bootstrap.mtls.client_cert_path).unwrap_or_default();
        
        if root_ca_der.is_empty() || client_cert.is_empty() {
            error!("Root CA or Client Cert missing. Cannot verify.");
            return false;
        }

        let mut id_pem = client_cert;
        id_pem.extend_from_slice(b"\n");
        id_pem.extend_from_slice(&key_data);

        let identity = match Identity::from_pem(&id_pem) {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to parse Identity from keystore key: {}", e);
                return false;
            }
        };

        let root_ca = match Certificate::from_pem(&root_ca_der) {
            Ok(ca) => ca,
            Err(e) => {
                error!("Failed to parse Root CA: {}", e);
                return false;
            }
        };

        let client = match reqwest::Client::builder()
            .add_root_certificate(root_ca)
            .identity(identity)
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to build mTLS client: {}", e);
                return false;
            }
        };

        // Try a simple request to verify the handshake
        let test_url = format!("{}/health", pollen_cloud_url);
        match client.get(&test_url).send().await {
            Ok(resp) => {
                info!("Verify-by-use successful! (Status: {})", resp.status());
                
                // 4. Quarantine original file
                if client_key_path.exists() {
                    let quarantine_path = client_key_path.with_extension("key.migrated");
                    info!("Quarantining original client.key to {:?}", quarantine_path);
                    
                    if let Err(e) = fs::rename(client_key_path, &quarantine_path) {
                        warn!("Failed to quarantine client.key: {}", e);
                    } else {
                        // Set strict permissions on quarantine file on Unix
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if let Ok(mut perms) = fs::metadata(&quarantine_path).map(|m| m.permissions()) {
                                perms.set_mode(0o600);
                                let _ = fs::set_permissions(&quarantine_path, perms);
                            }
                        }
                    }
                }
                return true;
            }
            Err(e) => {
                error!("mTLS handshake verification failed: {}. Keystore key might be invalid. Falling back to file.", e);
                return false;
            }
        }
    }

    false
}
