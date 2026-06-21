use crate::merge::FingerprintDb;
use crate::model::*;
use std::sync::{Arc, RwLock};

pub struct FingerprintService {
    db: Arc<RwLock<FingerprintDb>>,
    prev_db: Arc<RwLock<Option<FingerprintDb>>>,
    engine_version: semver::Version,
    keys: dek_bundle_sync::keys::TrustedKeySet,
}

impl FingerprintService {
    pub fn new(engine_version: semver::Version, keys: dek_bundle_sync::keys::TrustedKeySet) -> Self {
        let baseline = crate::embedded_baseline();
        Self {
            db: Arc::new(RwLock::new(FingerprintDb::from_baseline(baseline))),
            prev_db: Arc::new(RwLock::new(None)),
            engine_version,
            keys,
        }
    }

    pub async fn update_from_bytes(&self, raw: &[u8], sig_b64: &str) -> anyhow::Result<u64> {
        let def: FingerprintDefinition = serde_json::from_slice(raw)?;
        
        crate::verify::verify_definition(raw, sig_b64, &def, &self.engine_version, &self.keys)?;
        
        let mut candidate = {
            let cur = self.db.read().map_err(|_| anyhow::anyhow!("lock"))?;
            FingerprintDb { version: cur.version, by_id: cur.by_id.clone() }
        };
        
        let prev_version = candidate.version;
        if let Err(e) = candidate.apply(def.clone()) {
            let err_msg = e.to_string();
            if err_msg.contains("delta base") || err_msg.contains("ต้องดึง full") {
                return Err(anyhow::anyhow!("DELTA_BASE_MISMATCH: {e}"));
            }
            return Err(anyhow::anyhow!("apply failed: {e}"));
        }
        
        let new_version = candidate.version;
        {
            let mut w = self.db.write().map_err(|_| anyhow::anyhow!("lock"))?;
            
            // Save current state to prev_db for rollback
            if let Ok(mut prev_w) = self.prev_db.write() {
                *prev_w = Some(FingerprintDb { version: w.version, by_id: w.by_id.clone() });
            }

            *w = candidate;
        }
        
        tracing::info!(prev_version, new_version, "fingerprint definition updated");
        Ok(new_version)
    }

    pub async fn update_with_fallback<F, Fut>(&self, raw: &[u8], sig_b64: &str, fetch_full: F) -> anyhow::Result<u64>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<(Vec<u8>, String)>>,
    {
        match self.update_from_bytes(raw, sig_b64).await {
            Ok(v) => Ok(v),
            Err(e) if e.to_string().contains("DELTA_BASE_MISMATCH") => {
                tracing::warn!("Delta base mismatch detected. Falling back to full fetch...");
                let (full_raw, full_sig) = fetch_full().await?;
                self.update_from_bytes(&full_raw, &full_sig).await
            }
            Err(e) => Err(e),
        }
    }

    pub fn rollback(&self) -> anyhow::Result<u64> {
        let mut prev_w = self.prev_db.write().map_err(|_| anyhow::anyhow!("lock"))?;
        if let Some(prev) = prev_w.take() {
            let mut w = self.db.write().map_err(|_| anyhow::anyhow!("lock"))?;
            let rb_version = prev.version;
            *w = prev;
            tracing::info!("Rolled back fingerprint definition to version {}", rb_version);
            Ok(rb_version)
        } else {
            anyhow::bail!("No previous definition available for rollback");
        }
    }

    pub fn active_version(&self) -> u64 {
        self.db.read().map(|d| d.version).unwrap_or(0)
    }

    pub fn snapshot(&self) -> Vec<AgentSignatureV2> {
        self.db.read().map(|d| d.by_id.values().cloned().collect()).unwrap_or_default()
    }
}

pub enum UpdateStrategy { Full, Delta }

pub fn choose_update_strategy(current: u64, latest: u64, delta_count: usize) -> UpdateStrategy {
    if current == 0 { return UpdateStrategy::Full; }
    if latest > current + 10 { return UpdateStrategy::Full; }
    if delta_count > 50 { return UpdateStrategy::Full; }
    UpdateStrategy::Delta
}
