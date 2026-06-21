#![allow(unsafe_code)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::Keystore;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::ptr;
use winapi::um::dpapi::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN};
use winapi::um::winbase::LocalFree;
use winapi::um::wincrypt::CRYPTOAPI_BLOB;

pub struct DpapiKeystore {
    store_dir: PathBuf,
}

impl DpapiKeystore {
    pub fn new() -> Self {
        let mut dir =
            dirs_next::data_local_dir().unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
        dir.push("pollen-dek");
        dir.push("keystore");
        let _ = fs::create_dir_all(&dir);
        Self { store_dir: dir }
    }
}

impl Keystore for DpapiKeystore {
    fn store_key(&self, alias: &str, data: &[u8]) -> Result<()> {
        let path = self.store_dir.join(alias);

        let mut data_blob = CRYPTOAPI_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };

        let mut out_blob = CRYPTOAPI_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };

        let success = unsafe {
            CryptProtectData(
                &mut data_blob,
                ptr::null(),               // description
                ptr::null_mut(),           // entropy
                ptr::null_mut(),           // reserved
                ptr::null_mut(),           // prompt struct
                CRYPTPROTECT_UI_FORBIDDEN, // flags
                &mut out_blob,
            )
        };

        if success == 0 {
            anyhow::bail!(
                "CryptProtectData failed with error code: {}",
                std::io::Error::last_os_error()
            );
        }

        let encrypted =
            unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize) };
        let res = fs::write(&path, encrypted).context("Failed to write DPAPI encrypted blob");

        unsafe {
            LocalFree(out_blob.pbData as _);
        }

        res
    }

    fn load_key(&self, alias: &str) -> Result<Vec<u8>> {
        let path = self.store_dir.join(alias);
        if !path.exists() {
            anyhow::bail!("Key {} not found", alias);
        }

        let encrypted = fs::read(&path).context("Failed to read from DPAPI store path")?;

        let mut data_blob = CRYPTOAPI_BLOB {
            cbData: encrypted.len() as u32,
            pbData: encrypted.as_ptr() as *mut u8,
        };

        let mut out_blob = CRYPTOAPI_BLOB {
            cbData: 0,
            pbData: ptr::null_mut(),
        };

        let success = unsafe {
            CryptUnprotectData(
                &mut data_blob,
                ptr::null_mut(),           // description
                ptr::null_mut(),           // entropy
                ptr::null_mut(),           // reserved
                ptr::null_mut(),           // prompt struct
                CRYPTPROTECT_UI_FORBIDDEN, // flags
                &mut out_blob,
            )
        };

        if success == 0 {
            anyhow::bail!(
                "CryptUnprotectData failed with error code: {}",
                std::io::Error::last_os_error()
            );
        }

        let decrypted = unsafe {
            std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec()
        };

        unsafe {
            LocalFree(out_blob.pbData as _);
        }

        Ok(decrypted)
    }

    fn delete_key(&self, alias: &str) -> Result<()> {
        let path = self.store_dir.join(alias);
        if path.exists() {
            fs::remove_file(&path)?;
        }
        Ok(())
    }
}
