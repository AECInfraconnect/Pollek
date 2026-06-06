use crate::Keystore;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

// In a real production DPAPI implementation, we would call CryptProtectData and store the encrypted blob.
// For this scaffolding, we simulate it by storing a base64 encoded file representing the encrypted blob.
// A full implementation requires unsafe blocks interacting with winapi::um::dpapi::CryptProtectData.

pub struct DpapiKeystore {
    store_dir: PathBuf,
}

impl DpapiKeystore {
    pub fn new() -> Self {
        let mut dir = dirs_next::data_local_dir().unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
        dir.push("pollen-dek");
        dir.push("keystore");
        let _ = fs::create_dir_all(&dir);
        Self { store_dir: dir }
    }
}

impl Keystore for DpapiKeystore {
    fn store_key(&self, alias: &str, data: &[u8]) -> Result<()> {
        tracing::info!("(Simulated) Encrypting {} with DPAPI", alias);
        let path = self.store_dir.join(alias);
        // SIMULATION: encode instead of encrypt
        use base64::Engine;
        let encoded = base64::prelude::BASE64_STANDARD.encode(data);
        fs::write(&path, encoded).context("Failed to write to DPAPI store path")?;
        Ok(())
    }

    fn load_key(&self, alias: &str) -> Result<Vec<u8>> {
        tracing::info!("(Simulated) Decrypting {} with DPAPI", alias);
        let path = self.store_dir.join(alias);
        let encoded = fs::read_to_string(&path).context("Failed to read from DPAPI store path")?;
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
