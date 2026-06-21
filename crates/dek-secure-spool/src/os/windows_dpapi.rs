use crate::key_manager::{KeyStoreError, OsKeyStore};
use rand::{rngs::OsRng, RngCore};
use std::{fs::{self, OpenOptions}, io::{Read, Write}, path::PathBuf};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_LOCAL_MACHINE,
};

pub struct WindowsDpapiStore {
    key_path: PathBuf,
}

impl WindowsDpapiStore {
    pub fn new(key_path: PathBuf) -> Self {
        Self { key_path }
    }

    fn protect(&self, data: &[u8]) -> Result<Vec<u8>, KeyStoreError> {
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut _,
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();

        unsafe {
            CryptProtectData(
                &data_in,
                windows::core::w!("Pollen DEK spool master key"),
                None,
                None,
                None,
                CRYPTPROTECT_LOCAL_MACHINE,
                &mut data_out,
            )
            .map_err(|e| KeyStoreError::Os(e.to_string()))?;
        }

        let result = unsafe { std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize) }.to_vec();
        unsafe {
            windows::Win32::Foundation::LocalFree(windows::Win32::Foundation::HLOCAL(data_out.pbData as _));
        }

        Ok(result)
    }

    fn unprotect(&self, data: &[u8]) -> Result<Vec<u8>, KeyStoreError> {
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut _,
        };
        let mut data_out = CRYPT_INTEGER_BLOB::default();

        unsafe {
            CryptUnprotectData(&data_in, None, None, None, None, 0, &mut data_out)
                .map_err(|e| KeyStoreError::Os(e.to_string()))?;
        }

        let result = unsafe { std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize) }.to_vec();
        
        // Zero memory before freeing
        unsafe {
            std::ptr::write_bytes(data_out.pbData, 0, data_out.cbData as usize);
            windows::Win32::Foundation::LocalFree(windows::Win32::Foundation::HLOCAL(data_out.pbData as _));
        }

        Ok(result)
    }
}

impl OsKeyStore for WindowsDpapiStore {
    fn load_or_create_master_key(&self) -> Result<[u8; 32], KeyStoreError> {
        if self.key_path.exists() {
            let mut file = File::open(&self.key_path).map_err(|e| KeyStoreError::Os(e.to_string()))?;
            let mut encrypted = Vec::new();
            file.read_to_end(&mut encrypted).map_err(|e| KeyStoreError::Os(e.to_string()))?;
            let decrypted = self.unprotect(&encrypted)?;
            if decrypted.len() != 32 {
                return Err(KeyStoreError::Invalid);
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&decrypted);
            return Ok(key);
        }

        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);

        let encrypted = self.protect(&key)?;

        if let Some(parent) = self.key_path.parent() {
            fs::create_dir_all(parent).map_err(|e| KeyStoreError::Os(e.to_string()))?;
        }

        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&self.key_path)
            .map_err(|e| KeyStoreError::Os(e.to_string()))?;
        file.write_all(&encrypted).map_err(|e| KeyStoreError::Os(e.to_string()))?;
        file.sync_data().map_err(|e| KeyStoreError::Os(e.to_string()))?;

        Ok(key)
    }

    fn rotate_master_key(&self) -> Result<[u8; 32], KeyStoreError> {
        // Simple rotation by overwriting (in production, support multiple keys).
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        let encrypted = self.protect(&key)?;

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.key_path)
            .map_err(|e| KeyStoreError::Os(e.to_string()))?;
        file.write_all(&encrypted).map_err(|e| KeyStoreError::Os(e.to_string()))?;
        file.sync_data().map_err(|e| KeyStoreError::Os(e.to_string()))?;

        Ok(key)
    }
}

use std::fs::File;
