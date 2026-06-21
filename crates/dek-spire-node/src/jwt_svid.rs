use anyhow::{Context, Result};
use base64::Engine;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct JwtSvidCache {
    client: Arc<RwLock<Client>>,
    endpoint: String,
    /// audience -> (jwt, expiry_unix)
    cache: Arc<RwLock<HashMap<String, (String, i64)>>>,
}

impl JwtSvidCache {
    pub fn new(client: Arc<RwLock<Client>>, endpoint: String) -> Self {
        Self {
            client,
            endpoint,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update the mTLS client on renewal
    pub async fn update_mtls(&self, client: Client) {
        *self.client.write().await = client;
    }

    /// Invalidates a specific audience (e.g. after a 401)
    pub async fn invalidate(&self, audience: &str) {
        self.cache.write().await.remove(audience);
    }

    /// Gets a JWT SVID for the target audience. Refetches if <= 30s before exp.
    pub async fn get(&self, audience: &str) -> Result<String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // Fast path: Check cache
        {
            let cache = self.cache.read().await;
            if let Some((jwt, exp)) = cache.get(audience) {
                if *exp > now + 30 {
                    return Ok(jwt.clone());
                }
            }
        }

        // Slow path: Fetch new JWT
        let new_jwt = self.fetch_from_spire(audience).await?;

        // Parse expiry from the token payload to cache it
        let exp = Self::parse_exp(&new_jwt)?;

        self.cache
            .write()
            .await
            .insert(audience.to_string(), (new_jwt.clone(), exp));

        Ok(new_jwt)
    }

    async fn fetch_from_spire(&self, audience: &str) -> Result<String> {
        let client = self.client.read().await.clone();

        let body = serde_json::json!({
            "audience": [audience]
        });

        let resp = client
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await
            .context("POST /svid/jwt request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "SPIRE JWT SVID fetch failed: {} - {}",
                status,
                err_text
            ));
        }

        #[derive(serde::Deserialize)]
        struct SpireJwtResponse {
            svid: String,
        }

        let parsed: SpireJwtResponse = resp.json().await.context("Parse SpireJwtResponse")?;
        Ok(parsed.svid)
    }

    fn parse_exp(jwt: &str) -> Result<i64> {
        let parts: Vec<&str> = jwt.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow::anyhow!("Invalid JWT format"));
        }

        let payload_b64 = parts[1];
        // Standard or URL-safe base64 depending on JWT issuer
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload_b64)
            .or_else(|_| base64::engine::general_purpose::STANDARD.decode(payload_b64))
            .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload_b64))
            .context("base64 decode jwt payload")?;

        let payload: serde_json::Value =
            serde_json::from_slice(&decoded).context("parse jwt json")?;

        if let Some(exp) = payload.get("exp").and_then(|v| v.as_i64()) {
            Ok(exp)
        } else {
            Err(anyhow::anyhow!("No 'exp' claim in JWT"))
        }
    }
}
