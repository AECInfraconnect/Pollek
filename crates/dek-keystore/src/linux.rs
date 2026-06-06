use crate::Keystore;
use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

pub struct FileKeystore {
    store_dir: PathBuf,
}

impl FileKeystore {
    pub fn new() -> Self {
        let dir = PathBuf::from("/var/lib/pollen-dek/keystore");
        if !dir.exists() {
            // Ideally we also set 0700 on the directory
            let _ = fs::create_dir_all(&dir);
        }
        Self { store_dir: dir }
    }
}

impl Keystore for FileKeystore {
    fn store_key(&self, alias: &str, data: &[u8]) -> Result<()> {
        tracing::info!("Writing {} to Linux FileKeystore with 0600 perms", alias);
        let path = self.store_dir.join(alias);
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .context("Failed to open keystore file with strict permissions")?;
        
        file.write_all(data)?;
        Ok(())
    }

    fn load_key(&self, alias: &str) -> Result<Vec<u8>> {
        tracing::info!("Reading {} from Linux FileKeystore", alias);
        let path = self.store_dir.join(alias);
        let data = fs::read(&path).context("Failed to read from keystore file")?;
        Ok(data)
    }

    fn delete_key(&self, alias: &str) -> Result<()> {
        let path = self.store_dir.join(alias);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }
}
