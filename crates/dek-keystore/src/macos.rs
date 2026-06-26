// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::Keystore;
use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

pub struct KeychainKeystore {
    store_dir: PathBuf,
}

impl KeychainKeystore {
    pub fn new() -> Self {
        // TODO: Integrate with security-framework Keychain.
        // For beta, fallback to 0600 file-based storage.
        tracing::warn!("macOS secure Keystore not fully implemented. Falling back to 0600 file-based storage. Hardened key storage will follow in the next Phase.");
        let mut dir = dirs_next::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("/Library/Application Support"));
        dir.push("pollek-dek");
        dir.push("keystore");
        let _ = fs::create_dir_all(&dir);
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
        Self { store_dir: dir }
    }
}

impl Keystore for KeychainKeystore {
    fn store_key(&self, alias: &str, data: &[u8]) -> Result<()> {
        let path = self.store_dir.join(alias);
        fs::write(&path, data).context("Failed to write to keystore file")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .context("Failed to set 0600 permissions")?;
        Ok(())
    }

    fn load_key(&self, alias: &str) -> Result<Vec<u8>> {
        let path = self.store_dir.join(alias);
        if !path.exists() {
            anyhow::bail!("Key {} not found", alias);
        }
        fs::read(&path).context("Failed to read from keystore file")
    }

    fn delete_key(&self, alias: &str) -> Result<()> {
        let path = self.store_dir.join(alias);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }
}
