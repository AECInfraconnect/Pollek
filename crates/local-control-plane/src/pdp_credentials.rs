// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::error::{ApiError, ApiResult};
use std::path::PathBuf;
use tokio::fs;

// No direct import needed, we use windows_dpapi::protect

pub struct PdpCredentialsStore {
    data_dir: PathBuf,
}

impl PdpCredentialsStore {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    fn get_key_path(&self, pdp_id: &str) -> PathBuf {
        self.data_dir
            .join("credentials")
            .join(format!("{}.enc", pdp_id))
    }

    #[cfg(target_os = "windows")]
    pub async fn store_credential(&self, pdp_id: &str, secret: &str) -> ApiResult<()> {
        let path = self.get_key_path(pdp_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
        }

        // DPAPI encrypt with Machine/User scope. We'll use default CurrentUser scope.
        let encrypted =
            windows_dpapi::encrypt_data(secret.as_bytes(), windows_dpapi::Scope::User, None)
                .map_err(|e| {
                    ApiError::Internal(anyhow::anyhow!("DPAPI encrypt failed: {:?}", e))
                })?;

        fs::write(&path, encrypted)
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
        tracing::info!(
            "Stored secure credential for PDP {} via Windows DPAPI",
            pdp_id
        );
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    pub async fn store_credential(&self, pdp_id: &str, secret: &str) -> ApiResult<()> {
        let path = self.get_key_path(pdp_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
        }

        // Fallback for non-windows platforms. In production, use Keychain or Keyring.
        tracing::warn!(
            "DPAPI not available on this platform. Storing credential as plaintext fallback for {}",
            pdp_id
        );
        fs::write(&path, secret.as_bytes())
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    pub async fn retrieve_credential(&self, pdp_id: &str) -> ApiResult<Option<String>> {
        let path = self.get_key_path(pdp_id);
        if !path.exists() {
            return Ok(None);
        }

        let encrypted = fs::read(&path)
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
        let decrypted = windows_dpapi::decrypt_data(&encrypted, windows_dpapi::Scope::User, None)
            .map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("DPAPI decrypt failed: {:?}", e))
        })?;

        let secret = String::from_utf8(decrypted).map_err(|e| {
            ApiError::Internal(anyhow::anyhow!("Invalid UTF-8 in decrypted secret: {}", e))
        })?;
        Ok(Some(secret))
    }

    #[cfg(not(target_os = "windows"))]
    pub async fn retrieve_credential(&self, pdp_id: &str) -> ApiResult<Option<String>> {
        let path = self.get_key_path(pdp_id);
        if !path.exists() {
            return Ok(None);
        }

        let secret_bytes = fs::read(&path)
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
        let secret = String::from_utf8(secret_bytes)
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Invalid UTF-8 in secret: {}", e)))?;
        Ok(Some(secret))
    }

    pub async fn delete_credential(&self, pdp_id: &str) -> ApiResult<()> {
        let path = self.get_key_path(pdp_id);
        if path.exists() {
            fs::remove_file(&path)
                .await
                .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
            tracing::info!("Deleted secure credential for PDP {}", pdp_id);
        }
        Ok(())
    }
}
