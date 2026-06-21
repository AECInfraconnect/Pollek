use crate::key_manager::{KeyStoreError, OsKeyStore};
use rand::{rngs::OsRng, RngCore};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::PathBuf,
};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

pub struct LinuxFileFallbackStore {
    key_path: PathBuf,
}

impl LinuxFileFallbackStore {
    pub fn new(key_path: PathBuf) -> Self {
        Self { key_path }
    }
}

impl OsKeyStore for LinuxFileFallbackStore {
    fn load_or_create_master_key(&self) -> Result<[u8; 32], KeyStoreError> {
        if self.key_path.exists() {
            let mut file = OpenOptions::new()
                .read(true)
                .open(&self.key_path)
                .map_err(|e| KeyStoreError::Os(e.to_string()))?;
            let mut key = [0u8; 32];
            file.read_exact(&mut key)
                .map_err(|e| KeyStoreError::Os(e.to_string()))?;
            return Ok(key);
        }

        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);

        let mut options = OpenOptions::new();
        options.create_new(true).write(true);

        #[cfg(unix)]
        options.mode(0o600);

        let mut file = options
            .open(&self.key_path)
            .map_err(|e| KeyStoreError::Os(e.to_string()))?;
        file.write_all(&key)
            .map_err(|e| KeyStoreError::Os(e.to_string()))?;
        file.sync_data()
            .map_err(|e| KeyStoreError::Os(e.to_string()))?;
        Ok(key)
    }

    fn rotate_master_key(&self) -> Result<[u8; 32], KeyStoreError> {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);

        let mut options = OpenOptions::new();
        options.create(true).write(true).truncate(true);

        #[cfg(unix)]
        options.mode(0o600);

        let mut file = options
            .open(&self.key_path)
            .map_err(|e| KeyStoreError::Os(e.to_string()))?;
        file.write_all(&key)
            .map_err(|e| KeyStoreError::Os(e.to_string()))?;
        file.sync_data()
            .map_err(|e| KeyStoreError::Os(e.to_string()))?;
        Ok(key)
    }
}
