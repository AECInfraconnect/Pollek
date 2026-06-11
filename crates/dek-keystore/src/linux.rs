// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::Keystore;
use anyhow::{Context, Result};
use linux_keyutils::{Key, KeyRing, KeyRingIdentifier};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

pub struct KernelKeystore {
    store_dir: PathBuf,
}

impl KernelKeystore {
    pub fn new() -> Self {
        let mut dir = dirs_next::data_local_dir().unwrap_or_else(|| PathBuf::from("/var/lib"));
        dir.push("pollen-dek");
        dir.push("keystore");
        let _ = fs::create_dir_all(&dir);
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
        Self { store_dir: dir }
    }
}

impl Keystore for KernelKeystore {
    fn store_key(&self, alias: &str, data: &[u8]) -> Result<()> {
        let key_desc = format!("pollen_dek_{}", alias);

        // Try Kernel Keyring first
        match KeyRing::from_special_id(KeyRingIdentifier::User, false) {
            Ok(keyring) => match keyring.add_key(&key_desc, data) {
                Ok(_) => {
                    // Remove fallback file if it exists, to ensure keyring takes precedence
                    let path = self.store_dir.join(alias);
                    if path.exists() {
                        let _ = fs::remove_file(&path);
                    }
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to store key '{}' in Linux Keyring: {}. Falling back to 0600 file.",
                        alias,
                        e
                    );
                }
            },
            Err(e) => {
                tracing::warn!(
                    "Failed to access User Keyring: {}. Falling back to 0600 file.",
                    e
                );
            }
        }

        // Fallback to file-based
        let path = self.store_dir.join(alias);
        fs::write(&path, data).context("Failed to write to keystore file fallback")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .context("Failed to set 0600 permissions")?;
        Ok(())
    }

    fn load_key(&self, alias: &str) -> Result<Vec<u8>> {
        let key_desc = format!("pollen_dek_{}", alias);

        // Try Kernel Keyring first
        let keyring = KeyRing::from_special_id(KeyRingIdentifier::User, false)
            .context("Failed to access User Keyring")?;

        match keyring.search(&key_desc) {
            Ok(key) => {
                let mut buf = vec![0u8; 8192];
                match key.read(&mut buf) {
                    Ok(len) => {
                        buf.truncate(len);
                        return Ok(buf);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read key '{}' from Linux Keyring: {}", alias, e);
                    }
                }
            }
            Err(_) => {
                // Key not found in keyring, proceed to fallback
            }
        }

        // Fallback to file-based
        let path = self.store_dir.join(alias);
        if !path.exists() {
            anyhow::bail!("Key {} not found in Keyring or Fallback", alias);
        }
        fs::read(&path).context("Failed to read from keystore file fallback")
    }

    fn delete_key(&self, alias: &str) -> Result<()> {
        let key_desc = format!("pollen_dek_{}", alias);

        // Try deleting from Kernel Keyring
        let keyring = KeyRing::from_special_id(KeyRingIdentifier::User, false)
            .context("Failed to access User Keyring")?;

        if let Ok(key) = keyring.search(&key_desc) {
            let _ = key.invalidate();
        }

        // Also delete from fallback
        let path = self.store_dir.join(alias);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }
}
