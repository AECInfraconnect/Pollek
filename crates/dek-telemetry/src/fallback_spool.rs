// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

pub struct SecureFallback<
    K: dek_secure_spool::key_manager::OsKeyStore = dek_secure_spool::os::DefaultOsKeyStore,
> {
    spool: std::sync::Arc<dek_secure_spool::Spool<K>>,
}

impl<K: dek_secure_spool::key_manager::OsKeyStore> SecureFallback<K> {
    pub fn new(spool: std::sync::Arc<dek_secure_spool::Spool<K>>) -> Self {
        Self { spool }
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
            let bg_client = client.read().await.clone();
            self.replay_once(&endpoint_url, &bg_client, api_token.as_deref())
                .await;
        }
    }

    /// One replay cycle: read the spool, POST the records to
    /// `{endpoint}/fallback` and acknowledge them only on a 2xx response.
    /// Any other outcome keeps the records for the next cycle.
    pub async fn replay_once(
        &self,
        endpoint_url: &str,
        client: &reqwest::Client,
        api_token: Option<&str>,
    ) {
        let records = match self.spool.replay().await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("[SecureFallback] Spool replay failed: {}", e);
                return;
            }
        };

        if records.is_empty() {
            return;
        }

        let mut events_to_send = Vec::new();
        for rec in records {
            if let Ok(v) = serde_json::to_value(&rec) {
                events_to_send.push(v);
            }
        }

        let payload = serde_json::json!({ "events": events_to_send });
        let base = endpoint_url
            .trim_end_matches("/v1/telemetry/events")
            .trim_end_matches('/');
        let url = format!("{}/fallback", base);

        let mut req = client.post(&url).json(&payload);
        if let Some(t) = api_token {
            req = req.header("Authorization", format!("Bearer {}", t));
        }

        match req.send().await {
            Ok(res) if res.status().is_success() => {
                // Cloud confirmed receipt: ack the replayed records so they
                // are not replayed forever. The spool's truncate-after-ack
                // drops the delivered segments and restarts the hash chain.
                match self.spool.ack_all().await {
                    Ok(()) => tracing::info!(
                        "[SecureFallback] Replayed secure spool; records acknowledged"
                    ),
                    Err(e) => tracing::error!(
                        "[SecureFallback] Replay delivered but spool ack failed: {}",
                        e
                    ),
                }
            }
            Ok(res) if res.status().is_client_error() => {
                tracing::warn!(
                    "[SecureFallback] Cloud rejected secure spool replay (4xx). Status: {}",
                    res.status()
                );
            }
            Ok(res) => {
                tracing::warn!(
                    "[SecureFallback] Cloud returned {} for fallback, will retry later",
                    res.status()
                );
            }
            Err(e) => {
                tracing::warn!("[SecureFallback] Network error during replay: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use dek_secure_spool::key_manager::{KeyStoreError, OsKeyStore, SpoolKeyManager};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct DummyKeyStore;
    impl OsKeyStore for DummyKeyStore {
        fn load_or_create_master_key(&self) -> Result<[u8; 32], KeyStoreError> {
            Ok([7u8; 32])
        }
        fn rotate_master_key(&self) -> Result<[u8; 32], KeyStoreError> {
            Ok([7u8; 32])
        }
    }

    fn test_fallback(
        name: &str,
    ) -> (
        SecureFallback<DummyKeyStore>,
        Arc<dek_secure_spool::Spool<DummyKeyStore>>,
        PathBuf,
    ) {
        let dir = std::env::temp_dir().join(format!(
            "fallback_spool_test_{}_{}",
            std::process::id(),
            name
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let km = SpoolKeyManager::new(DummyKeyStore);
        let spool = Arc::new(dek_secure_spool::Spool::new(
            dir.clone(),
            1024 * 1024,
            Some(km),
            "test-tenant".to_string(),
            "test-device".to_string(),
        ));
        (SecureFallback::new(spool.clone()), spool, dir)
    }

    /// Minimal HTTP stub: accepts connections, discards the request bytes and
    /// replies with the configured status code.
    async fn spawn_stub_server(status: u16) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind stub server");
        let addr = listener.local_addr().expect("stub server addr");
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = vec![0u8; 8192];
                let _ = socket.read(&mut buf).await;
                let body = b"{}";
                let header = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.write_all(body).await;
            }
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn successful_replay_acks_and_clears_records() {
        let (fallback, spool, dir) = test_fallback("ack");
        spool.enqueue(b"event1".to_vec()).await.unwrap();
        spool.enqueue(b"event2".to_vec()).await.unwrap();
        assert_eq!(spool.replay().await.unwrap().len(), 2);

        let url = spawn_stub_server(200).await;
        let client = reqwest::Client::new();
        fallback.replay_once(&url, &client, None).await;

        let remaining = spool.replay().await.unwrap();
        assert!(
            remaining.is_empty(),
            "acked records must not replay again, got {} left",
            remaining.len()
        );

        // The spool stays usable: new records start a fresh, valid chain.
        spool.enqueue(b"event3".to_vec()).await.unwrap();
        let chain = spool.replay().await.unwrap();
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].payload_json, "event3");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn failed_replay_retains_records() {
        let (fallback, spool, dir) = test_fallback("retain");
        spool.enqueue(b"event1".to_vec()).await.unwrap();
        spool.enqueue(b"event2".to_vec()).await.unwrap();

        let url = spawn_stub_server(500).await;
        let client = reqwest::Client::new();
        fallback.replay_once(&url, &client, None).await;

        let remaining = spool.replay().await.unwrap();
        assert_eq!(
            remaining.len(),
            2,
            "records must be kept for the next cycle on non-2xx"
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn network_error_retains_records() {
        let (fallback, spool, dir) = test_fallback("neterr");
        spool.enqueue(b"event1".to_vec()).await.unwrap();

        // Nothing listens on this port: connection refused.
        let client = reqwest::Client::new();
        fallback.replay_once("http://127.0.0.1:1", &client, None).await;

        let remaining = spool.replay().await.unwrap();
        assert_eq!(remaining.len(), 1, "records must survive network errors");

        let _ = std::fs::remove_dir_all(dir);
    }
}
