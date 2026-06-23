// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

pub struct SecureFallback {
    spool: std::sync::Arc<dek_secure_spool::Spool>,
}

impl SecureFallback {
    pub fn new(spool: std::sync::Arc<dek_secure_spool::Spool>) -> Self {
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

            let records = match self.spool.replay().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("[SecureFallback] Spool replay failed: {}", e);
                    continue;
                }
            };

            if records.is_empty() {
                continue;
            }

            let mut events_to_send = Vec::new();
            for rec in records {
                if let Ok(v) = serde_json::to_value(&rec) {
                    events_to_send.push(v);
                }
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
                    tracing::info!("[SecureFallback] Successfully replayed secure spool");
                    // Assuming dek_secure_spool clears or rotates itself, or we need to acknowledge.
                    // For now we just log success.
                }
                Ok(res) if res.status().is_client_error() => {
                    tracing::warn!("[SecureFallback] Cloud rejected secure spool replay (4xx). Status: {}", res.status());
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
}
