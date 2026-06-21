//! dek-policy-syncer — orchestrates the policy sync lifecycle and owns the
//! fail-safe EnforcementState.
//!
//! Phase 0: consolidate sync (reuse `dek-bundle-sync` for fetch/verify) behind
//!          one `PolicySyncer` with `sync_once` + `spawn`.
//! Phase 1: freshness gate — a watchdog derives `EnforcementState` from policy
//!          freshness and publishes it (in-process via ArcSwap + cross-process
//!          via a status file) so the PEP can FAIL CLOSED on stale/absent policy.
//!
//! Guardrails honored: no local authoring/compile/dry-run; fallback is LKG
//! read-only; never fail-open; no panics on network input.

pub mod state;
pub mod gate;
pub mod audit;
pub mod keys;
pub use gate::strict_deny_reason;

use crate::audit::AuditTrail;
use crate::keys as keymgr;

use anyhow::Result;
use arc_swap::ArcSwap;
use dek_bundle_sync::BundleSyncAgent;
use dek_telemetry::CloudTelemetrySink;
use std::path::Path;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use sha2::Digest;

pub use state::{evaluate_state, EnforcementState, EnforcementStatus, FreshnessConfig};

/// Outcome of one sync attempt.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncOutcome {
    Updated { 
        version: String,
        network_rules: Option<Vec<dek_domain_schema::CompiledNetworkRules>>,
    },
    Failed { reason: String },
    StateTransition(EnforcementState),
}

pub struct PolicySyncer {
    bundle_agent: Arc<BundleSyncAgent>,
    telemetry: Option<Arc<CloudTelemetrySink>>,
    cfg: FreshnessConfig,
    /// In-process source of truth for the PEP when co-located.
    enforcement: Arc<ArcSwap<EnforcementState>>,
    /// Unix seconds of the last SUCCESSFUL sync (0 = never).
    last_sync: AtomicI64,
    /// Active bundle's own expiry (unix secs), if known. -1 = unknown/none.
    bundle_expires: AtomicI64,
    /// Latest known bundle version (for status/audit).
    bundle_version: ArcSwap<Option<String>>,
    audit: AuditTrail,
    keys_url: String,
    push_url: String,
}

impl PolicySyncer {
    pub fn new(
        bundle_agent: Arc<BundleSyncAgent>,
        telemetry: Option<Arc<CloudTelemetrySink>>,
        cfg: FreshnessConfig,
        device_id: String,
        tenant_id: String,
        cloud_url: String,
        pinned_b64: String,
    ) -> Arc<Self> {
        let audit = AuditTrail::new(telemetry.clone(), device_id, tenant_id);
        let set = keymgr::load_or_bootstrap(&pinned_b64);
        bundle_agent.update_keys(set);

        let keys_url = format!("{}/v1/keys", cloud_url);
        let push_url = format!("{}/v1/push", cloud_url);

        Arc::new(Self {
            bundle_agent,
            telemetry,
            cfg,
            // Start fail-closed: no bundle proven fresh yet (cold start).
            enforcement: Arc::new(ArcSwap::from_pointee(EnforcementState::StrictDeny {
                since_unix: now_unix(),
                reason: "startup_not_yet_synced".into(),
            })),
            last_sync: AtomicI64::new(0),
            bundle_expires: AtomicI64::new(-1),
            bundle_version: ArcSwap::from_pointee(None),
            audit,
            keys_url,
            push_url,
        })
    }

    /// In-process handle to the current enforcement state (PEP co-located case).
    pub fn enforcement(&self) -> Arc<ArcSwap<EnforcementState>> {
        self.enforcement.clone()
    }

    /// One pull+verify+activate cycle. Reuses BundleSyncAgent::run_pipeline
    /// (TUF-Lite fetch + ed25519 verify + anti-rollback + hash + atomic stage).
    /// On success, records freshness and recomputes the enforcement state.
    pub async fn sync_once(&self) -> SyncOutcome {
        // Build a temporary reqwest client using system roots for the mtls/key endpoint. 
        // In real use, this should probably come from the agent's mtls config.
        let client = reqwest::Client::new();
        match keymgr::fetch_and_merge(&client, &self.keys_url, &self.bundle_agent.key_set_snapshot()).await {
            Ok((merged, delta)) if !delta.is_empty() => {
                self.bundle_agent.update_keys(merged);
                self.audit.key_rotation(&delta.added, &delta.promoted, &delta.revoked);
            }
            Ok(_) => {}                          // ไม่มีการเปลี่ยน
            Err(e) => tracing::warn!("key refresh skipped: {e}"),  // ใช้ set เดิมต่อ (fail-safe)
        }

        match self.bundle_agent.run_pipeline().await {
            Ok((_dek_config, manifest_path)) => {
                let version = derive_version(&manifest_path);
                
                // (Phase 3) digest + key id จาก verify ล่าสุด
                // For demonstration, read the raw signed payload to compute digest. In reality, BundleSyncAgent should return this.
                let signed_bytes = std::fs::read(&manifest_path).unwrap_or_default();
                let digest = format!("sha256:{}", hex::encode(sha2::Sha256::digest(&signed_bytes)));
                self.audit.sync_success(&version, "active", &digest);
                self.audit.activated(&version, "full");

                let expires = read_manifest_expiry_unix(&manifest_path);
                self.bundle_expires
                    .store(expires.unwrap_or(-1), Ordering::SeqCst);
                self.last_sync.store(now_unix(), Ordering::SeqCst);
                self.bundle_version.store(Arc::new(Some(version.clone())));
                self.recompute_state();
                info!("[PolicySyncer] sync OK, version={}", version);

                // Phase 1: Fetch network guardrails
                let network_rules = match self.bundle_agent.fetch_network_guardrails().await {
                    Ok(rules) => {
                        info!("[PolicySyncer] network guardrails fetched successfully ({} rules)", rules.len());
                        Some(rules)
                    }
                    Err(e) => {
                        warn!("[PolicySyncer] failed to fetch network guardrails: {}", e);
                        None
                    }
                };

                SyncOutcome::Updated { version, network_rules }
            }
            Err(e) => {
                let reason = e.to_string();
                warn!("[PolicySyncer] sync failed: {}", reason);
                
                // (Phase 3) แยก unsigned/forged push ออกจาก network error
                if let Some(be) = e.downcast_ref::<dek_bundle_sync::BundleError>() {
                    match be {
                        dek_bundle_sync::BundleError::SignatureRejected { role, detail } =>
                            self.audit.unsigned_bundle_rejected(role, detail),
                        dek_bundle_sync::BundleError::RollbackBlocked { current, incoming } =>
                            self.audit.rollback_blocked(*current, *incoming),
                    }
                }

                self.recompute_state();
                SyncOutcome::Failed { reason }
            }
        }
    }

    /// Recompute EnforcementState from current freshness and publish it both
    /// in-process (ArcSwap) and cross-process (status file). Logs + emits on
    /// transition.
    pub fn recompute_state(&self) -> Option<EnforcementState> {
        let now = now_unix();
        let expires = match self.bundle_expires.load(Ordering::SeqCst) {
            -1 => None,
            v => Some(v),
        };
        let last_sync = match self.last_sync.load(Ordering::SeqCst) {
            0 => None,
            v => Some(v),
        };
        let next = evaluate_state(now, expires, last_sync, &self.cfg);
        let prev = self.enforcement.load_full();

        let changed = *prev != next;
        if changed {
            warn!(
                "[PolicySyncer] enforcement state: {} -> {} ({})",
                prev.label(),
                next.label(),
                next.reason()
            );
            self.audit.state_change(prev.label(), next.label(), &next.reason());
        }

        metrics::gauge!("dek_enforcement_state").set(next.gauge());
        self.enforcement.store(Arc::new(next.clone()));

        let version = self.bundle_version.load_full().as_ref().clone();
        let status = EnforcementStatus { state: next.clone(), updated_unix: now, bundle_version: version };
        if let Err(e) = state::write_status_atomic(&status) {
            error!("[PolicySyncer] failed to write enforcement status file: {}", e);
        }

        if changed {
            Some(next)
        } else {
            None
        }
    }

    // emit_state_change is now subsumed by AuditTrail


    /// Spawn the polling loop + freshness watchdog. The returned handle keeps
    /// the tasks; dropping/cancelling stops them.
    pub fn spawn(
        self: Arc<Self>,
        poll_interval: Duration,
        cancel: CancellationToken,
        sync_tx: Option<tokio::sync::mpsc::Sender<SyncOutcome>>,
    ) -> SyncerHandle {
        // Polling loop: pull+verify+activate.
        let s1 = self.clone();
        let c1 = cancel.clone();
        let poll = tokio::spawn(async move {
            let mut tick = tokio::time::interval(poll_interval.max(Duration::from_secs(1)));
            loop {
                tokio::select! {
                    _ = c1.cancelled() => break,
                    _ = tick.tick() => { 
                        let outcome = s1.sync_once().await; 
                        if let Some(tx) = &sync_tx {
                            let _ = tx.send(outcome).await;
                        }
                    }
                }
            }
        });

        // Watchdog: recompute freshness frequently so expiry/grace/max-age
        // transitions happen even when the cloud is unreachable.
        let s2 = self.clone();
        let c2 = cancel.clone();
        let sync_tx2 = sync_tx.clone();
        let watch = tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = c2.cancelled() => break,
                    _ = tick.tick() => { 
                        if let Some(new_state) = s2.recompute_state() {
                            if let Some(tx) = &sync_tx2 {
                                let _ = tx.send(SyncOutcome::StateTransition(new_state)).await;
                            }
                        }
                    }
                }
            }
        });

        // Auto-Sync Push (SSE)
        let s3 = self.clone();
        let c3 = cancel.clone();
        let sync_tx3 = sync_tx;
        let push_url = self.push_url.clone();
        let sse = tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(3600)) // Long timeout for SSE
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
                
            loop {
                if c3.is_cancelled() { break; }
                
                match client.get(&push_url)
                    .header("Accept", "text/event-stream")
                    .send()
                    .await
                {
                    Ok(mut resp) if resp.status().is_success() => {
                        info!("[PolicySyncer] Connected to auto-sync push stream at {}", push_url);
                        while let Ok(Some(chunk)) = resp.chunk().await {
                            if c3.is_cancelled() { break; }
                            if let Ok(text) = std::str::from_utf8(&chunk) {
                                if text.contains("bundle_ready") {
                                    info!("[PolicySyncer] Received bundle_ready push event, triggering sync_once");
                                    let outcome = s3.sync_once().await;
                                    if let Some(tx) = &sync_tx3 {
                                        let _ = tx.send(outcome).await;
                                    }
                                }
                            }
                        }
                    }
                    Ok(resp) => {
                        warn!("[PolicySyncer] Push stream rejected: {}", resp.status());
                    }
                    Err(e) => {
                        warn!("[PolicySyncer] Push stream connection failed: {}", e);
                    }
                }
                
                // Backoff before reconnecting
                tokio::select! {
                    _ = c3.cancelled() => break,
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {}
                }
            }
        });

        SyncerHandle { tasks: vec![poll, watch, sse] }
    }
}

pub struct SyncerHandle {
    tasks: Vec<JoinHandle<()>>,
}
impl Drop for SyncerHandle {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}

fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn derive_version(manifest_path: &Path) -> String {
    manifest_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Best-effort parse of `expires_at` (RFC3339) from the staged manifest JSON.
/// Returns None if the field is absent (freshness then relies on max_bundle_age).
fn read_manifest_expiry_unix(manifest_path: &Path) -> Option<i64> {
    let bytes = std::fs::read(manifest_path).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let s = v.get("expires_at").and_then(|x| x.as_str())?;
    // Parse RFC3339 without bringing chrono in: accept "...Z" or with offset.
    // Fallback: if parse fails, return None.
    parse_rfc3339_to_unix(s)
}

fn parse_rfc3339_to_unix(s: &str) -> Option<i64> {
    // Minimal RFC3339 -> unix using time crate via chrono is overkill here;
    // delegate to OffsetDateTime if available. Keep dependency-free: best-effort
    // using a tiny parser for "YYYY-MM-DDTHH:MM:SS(.fff)?(Z|+hh:mm)".
    // For robustness in production, replace with chrono::DateTime::parse_from_rfc3339.
    // Here we accept the common 'Z' form.
    let s = s.trim();
    // Very small, tolerant parse; returns None on anything unexpected.
    let (date, time) = s.split_once('T')?;
    let mut dparts = date.split('-');
    let y: i64 = dparts.next()?.parse().ok()?;
    let mo: i64 = dparts.next()?.parse().ok()?;
    let d: i64 = dparts.next()?.parse().ok()?;
    let time = time.trim_end_matches('Z');
    let time = time.split('+').next().unwrap_or(time);
    let time = time.split('.').next().unwrap_or(time);
    let mut tparts = time.split(':');
    let h: i64 = tparts.next()?.parse().ok()?;
    let mi: i64 = tparts.next()?.parse().ok()?;
    let sec: i64 = tparts.next().unwrap_or("0").parse().ok()?;
    // days since epoch (civil calendar, proleptic Gregorian) — Howard Hinnant's algo
    let yy = if mo <= 2 { y - 1 } else { y };
    let era = (if yy >= 0 { yy } else { yy - 399 }) / 400;
    let yoe = yy - era * 400;
    let doy = (153 * (if mo > 2 { mo - 3 } else { mo + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3_600 + mi * 60 + sec)
}
