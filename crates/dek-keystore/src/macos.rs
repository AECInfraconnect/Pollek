use crate::Keystore;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

// In a real implementation, this would use security-framework to store items in the macOS Keychain.
// For scaffolding without a complex keychain setup, we simulate it similarly to Windows DPAPI.

pub struct KeychainKeystore {
    store_dir: PathBuf,
}

impl KeychainKeystore {
    pub fn new() -> Self {
        let mut dir = dirs_next::data_local_dir().unwrap_or_else(|| PathBuf::from("/Library/Application Support"));
        dir.push("pollen-dek");
        dir.push("keychain-sim");
        let _ = fs::create_dir_all(&dir);
        Self { store_dir: dir }
    }
}

impl Keystore for KeychainKeystore {
    fn store_key(&self, alias: &str, data: &[u8]) -> Result<()> {
        tracing::info!("(Simulated) Writing {} to macOS Keychain", alias);
        let path = self.store_dir.join(alias);
        use base64::Engine;
        let encoded = base64::prelude::BASE64_STANDARD.encode(data);
        fs::write(&path, encoded).context("Failed to write to Keychain sim")?;
        Ok(())
    }

    fn load_key(&self, alias: &str) -> Result<Vec<u8>> {
        tracing::info!("(Simulated) Reading {} from macOS Keychain", alias);
        let path = self.store_dir.join(alias);
        let encoded = fs::read_to_string(&path).context("Failed to read from Keychain sim")?;
        use base64::Engine;
        let decoded = base64::prelude::BASE64_STANDARD.decode(encoded.trim())?;
        Ok(decoded)
    }

    fn delete_key(&self, alias: &str) -> Result<()> {
        let path = self.store_dir.join(alias);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }
}
