// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::Result;
use dek_secure_spool::{
    crypto::AeadKey,
    key_manager::SpoolKeyManager,
    os::DefaultOsKeyStore,
    segment::{SegmentWriter, TelemetryEvent},
};
use std::path::PathBuf;
use uuid::Uuid;

pub struct SecureFallback {
    key: AeadKey,
    dir: PathBuf,
    tenant_id: String,
    device_id: String,
}

impl SecureFallback {
    pub fn new(tenant_id: String, device_id: String) -> Result<Self> {
        let key_dir = dek_config::paths::get_data_dir();
        std::fs::create_dir_all(&key_dir)?;
        
        #[cfg(windows)]
        let store = DefaultOsKeyStore::new(key_dir.join("secure_spool.key"));
        #[cfg(target_os = "linux")]
        let store = DefaultOsKeyStore::new(key_dir.join("secure_spool.key"));
        #[cfg(target_os = "macos")]
        let store = DefaultOsKeyStore::new();

        let key_mgr = SpoolKeyManager::new(store);
        let key = key_mgr.active_aead_key()?;
        
        let spool_dir = key_dir.join("secure_spool");
        std::fs::create_dir_all(&spool_dir)?;

        Ok(Self {
            key,
            dir: spool_dir,
            tenant_id,
            device_id,
        })
    }

    pub fn append_batch(&self, events: Vec<serde_json::Value>) -> Result<()> {
        let segment_id = Uuid::new_v4().to_string();
        let path = self.dir.join(format!("{}.pds", segment_id));
        let mut writer = SegmentWriter::create(&path, &self.tenant_id, &self.device_id, &segment_id)?;

        for body in events {
            let ev = TelemetryEvent {
                schema_version: "1.0".to_string(),
                event_id: Uuid::new_v4(),
                tenant_id: self.tenant_id.clone(),
                device_id: self.device_id.clone(),
                event_type: "fallback".to_string(),
                timestamp_unix_ms: chrono::Utc::now().timestamp_millis(),
                body,
            };
            writer.append_event(&self.key, &ev)?;
        }
        Ok(())
    }

    pub async fn start_replay(
        &self,
        endpoint_url: String,
        client: std::sync::Arc<tokio::sync::RwLock<reqwest::Client>>,
        api_token: Option<String>,
    ) {
        tracing::info!("[SecureFallback] Replay task started");
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(300)).await;

            let mut files = match std::fs::read_dir(&self.dir) {
                Ok(rd) => rd.filter_map(Result::ok).map(|d| d.path()).collect::<Vec<_>>(),
                Err(_) => continue,
            };

            // Retention / Quota limits: keep only latest 100 segments if disk is full
            files.sort();
            if files.len() > 100 {
                tracing::warn!("[SecureFallback] Quota exceeded, dropping oldest segments");
                for f in files.iter().take(files.len() - 100) {
                    let _ = std::fs::remove_file(f);
                }
                files.drain(0..files.len() - 100);
            }

            for path in files {
                if !path.extension().map_or(false, |ext| ext == "pds") {
                    continue;
                }

                let records = match dek_secure_spool::segment::read_encrypted_records(&path) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!("[SecureFallback] Corrupted frame in {:?}: {}; quarantining", path, e);
                        let mut quarantine_path = path.clone();
                        quarantine_path.set_extension("pds.quarantine");
                        let _ = std::fs::rename(&path, &quarantine_path);
                        continue;
                    }
                };

                let mut events_to_send = Vec::new();
                let mut decode_failed = false;

                for rec in records {
                    match self.key.decrypt_record(&rec) {
                        Ok(plaintext) => {
                            if let Ok(ev) = serde_json::from_slice::<TelemetryEvent>(&plaintext) {
                                events_to_send.push(ev.body);
                            }
                        }
                        Err(e) => {
                            tracing::error!("[SecureFallback] Decryption failed for record: {}", e);
                            decode_failed = true;
                        }
                    }
                }

                if decode_failed {
                    tracing::warn!("[SecureFallback] Some records failed decryption, moving {:?} to quarantine", path);
                    let mut quarantine_path = path.clone();
                    quarantine_path.set_extension("pds.quarantine");
                    let _ = std::fs::rename(&path, &quarantine_path);
                    continue;
                }

                if events_to_send.is_empty() {
                    let _ = std::fs::remove_file(&path);
                    continue;
                }

                let payload = serde_json::json!({ "events": events_to_send });
                let url = format!("{}/fallback", endpoint_url.trim_end_matches('/'));

                let bg_client = client.read().await.clone();
                let mut req = bg_client.post(&url).json(&payload);
                if let Some(t) = &api_token {
                    req = req.header("Authorization", format!("Bearer {}", t));
                }

                match req.send().await {
                    Ok(res) if res.status().is_success() => {
                        tracing::info!("[SecureFallback] Successfully replayed segment {:?}", path);
                        let _ = std::fs::remove_file(&path);
                    }
                    Ok(res) if res.status().is_client_error() => {
                        tracing::warn!("[SecureFallback] Cloud rejected fallback replay (4xx), dropping segment {:?}", path);
                        let _ = std::fs::remove_file(&path);
                    }
                    Ok(res) => {
                        tracing::warn!("[SecureFallback] Cloud returned {} for fallback, will retry later", res.status());
                    }
                    Err(e) => {
                        tracing::warn!("[SecureFallback] Network error during replay: {}", e);
                    }
                }
            }
        }
    }
}
