pub mod audit;
pub mod crypto;
pub mod key_manager;
pub mod os;
pub mod segment;

use std::path::PathBuf;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SpoolError {
    #[error("spool is full: used={used} limit={limit}")]
    Full { used: u64, limit: u64 },
    #[error("crypto failure")]
    Crypto,
    #[error("io failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization failure: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("key manager error: {0}")]
    KeyManager(String),
    #[error("empty payload")]
    EmptyPayload,
    #[error("tampering detected")]
    Tampered,
}

pub struct SpoolState {
    writer: Option<segment::SegmentWriter>,
    current_segment_id: String,
    current_size: u64,
    last_hash: String,
    seq: u64,
}

pub struct Spool {
    dir: PathBuf,
    max_bytes: u64,
    key_manager: Option<key_manager::SpoolKeyManager<os::DefaultOsKeyStore>>,
    tenant_id: String,
    device_id: String,
    state: Mutex<SpoolState>,
}

impl Default for Spool {
    fn default() -> Self {
        Self::new(
            std::env::temp_dir().join("pollek-spool"),
            100 * 1024 * 1024,
            None,
            "local".to_string(),
            "default".to_string(),
        )
    }
}

impl Spool {
    pub fn new(
        dir: PathBuf,
        max_bytes: u64,
        key_manager: Option<key_manager::SpoolKeyManager<os::DefaultOsKeyStore>>,
        tenant_id: String,
        device_id: String,
    ) -> Self {
        Self {
            dir,
            max_bytes,
            key_manager,
            tenant_id,
            device_id,
            state: Mutex::new(SpoolState {
                writer: None,
                current_segment_id: "".to_string(),
                current_size: 0,
                last_hash: "GENESIS".to_string(),
                seq: 0,
            }),
        }
    }

    pub async fn enqueue(&self, data: Vec<u8>) -> Result<(), SpoolError> {
        if data.is_empty() {
            return Err(SpoolError::EmptyPayload);
        }

        self.ensure_capacity().await?;

        let key = if let Some(km) = &self.key_manager {
            km.active_aead_key().map_err(|e| SpoolError::KeyManager(e.to_string()))?
        } else {
            return Err(SpoolError::KeyManager("No key manager provided".to_string()));
        };

        let (prev_hash, seq) = {
            let mut state = self.state.lock().await;
            state.seq += 1;
            (state.last_hash.clone(), state.seq)
        };

        let payload_json = String::from_utf8(data.clone()).unwrap_or_default();
        let audit_entry = audit::AuditEntry::new(
            seq,
            chrono::Utc::now().to_rfc3339(),
            payload_json,
            &prev_hash,
        );

        let event = segment::TelemetryEvent {
            schema_version: "1.0".to_string(),
            event_id: Uuid::new_v4(),
            tenant_id: self.tenant_id.clone(),
            device_id: self.device_id.clone(),
            event_type: "raw".to_string(),
            timestamp_unix_ms: chrono::Utc::now().timestamp_millis(),
            body: serde_json::to_value(&audit_entry).map_err(SpoolError::Serde)?,
        };

        let mut state = self.state.lock().await;
        if state.writer.is_none() {
            let segment_id = Uuid::new_v4().to_string();
            let mut file_path = self.dir.clone();
            file_path.push(format!("{}.pds", segment_id));

            if !self.dir.exists() {
                std::fs::create_dir_all(&self.dir)?;
            }

            let writer = segment::SegmentWriter::create(
                &file_path,
                self.tenant_id.clone(),
                self.device_id.clone(),
                segment_id.clone(),
            )?;
            state.writer = Some(writer);
            state.current_segment_id = segment_id;
        }

        let writer = state.writer.as_mut().unwrap();
        writer
            .append_event(&key, &event)?;

        state.last_hash = audit_entry.entry_hash;
        state.current_size += data.len() as u64;

        Ok(())
    }

    async fn ensure_capacity(&self) -> Result<(), SpoolError> {
        let used = self.current_size().await?;
        if used > self.max_bytes {
            return Err(SpoolError::Full {
                used,
                limit: self.max_bytes,
            });
        }
        Ok(())
    }

    pub async fn current_size(&self) -> Result<u64, SpoolError> {
        let mut total_size = 0;
        if self.dir.exists() {
            if let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Ok(meta) = entry.metadata().await {
                        total_size += meta.len();
                    }
                }
            }
        }
        Ok(total_size)
    }

    pub async fn replay(&self) -> Result<Vec<audit::AuditEntry>, SpoolError> {
        let key = if let Some(km) = &self.key_manager {
            km.active_aead_key()
                .map_err(|e| SpoolError::KeyManager(e.to_string()))?
        } else {
            return Err(SpoolError::KeyManager("No key manager provided".to_string()));
        };

        let mut results = Vec::new();
        if self.dir.exists() {
            if let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("pds") {
                        if let Ok(records) = segment::read_encrypted_records(&path) {
                            for record in records {
                                if let Ok(plaintext) = key.decrypt_record(&record) {
                                    if let Ok(event) =
                                        serde_json::from_slice::<segment::TelemetryEvent>(
                                            &plaintext,
                                        )
                                    {
                                        if let Ok(audit_entry) =
                                            serde_json::from_value::<audit::AuditEntry>(event.body)
                                        {
                                            results.push(audit_entry);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Ensure chain validity
        if !results.is_empty() {
            results.sort_by_key(|e| e.seq); // Simplistic order for replay
            if audit::verify_chain(&results).is_err() {
                // Quarantine segments
                if let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("pds") {
                            let new_path = path.with_extension("quarantine");
                            let _ = tokio::fs::rename(path, new_path).await;
                        }
                    }
                }
                return Err(SpoolError::Tampered);
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct DummyKeyStore;
    impl crate::key_manager::OsKeyStore for DummyKeyStore {
        fn load_or_create_master_key(&self) -> Result<[u8; 32], crate::key_manager::KeyStoreError> {
            Ok([0u8; 32])
        }
        fn rotate_master_key(&self) -> Result<[u8; 32], crate::key_manager::KeyStoreError> {
            Ok([0u8; 32])
        }
    }

    #[tokio::test]
    async fn test_spool_enqueue_and_replay() {
        let dir = std::env::temp_dir().join(format!("test_spool_{}", Uuid::new_v4()));
        let km = key_manager::SpoolKeyManager::new(DummyKeyStore);
        let spool = Spool::new(
            dir.clone(),
            1024 * 1024,
            Some(km),
            "test".to_string(),
            "test".to_string(),
        );

        spool.enqueue(b"event1".to_vec()).await.unwrap();
        spool.enqueue(b"event2".to_vec()).await.unwrap();

        let replays = spool.replay().await.unwrap();
        assert_eq!(replays.len(), 2);
        assert_eq!(replays[0].payload_json, "event1");
        assert_eq!(replays[1].payload_json, "event2");

        let _ = fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn test_spool_full() {
        let dir = std::env::temp_dir().join(format!("test_spool_{}", Uuid::new_v4()));
        let km = key_manager::SpoolKeyManager::new(DummyKeyStore);
        let spool = Spool::new(
            dir.clone(),
            10,
            Some(km),
            "test".to_string(),
            "test".to_string(),
        ); // Very small limit

        // First might succeed or fail depending on metadata, but eventually it hits the limit
        let _ = spool.enqueue(b"event1".to_vec()).await;
        let err = spool
            .enqueue(b"very long event string to fill up spool size quickly".to_vec())
            .await;
        assert!(err.is_err());

        let _ = fs::remove_dir_all(dir);
    }
}
