// SPDX-License-Identifier: Apache-2.0
//! trust_bundle.rs — F3.2: poll & pin the SPIRE trust bundle (root rotation).
//!
//! Matches the Cloud/mock `/v1/trust-bundle` response shape:
//!   { "trust_bundle_pem": "<PEM>", "jwt_authorities": [...], "refresh_hint": 3600 }
//!
//! The DEK refreshes on `refresh_hint` so Cloud can rotate its root CA / JWT
//! keys WITHOUT a re-enroll. New roots are written atomically to the mTLS
//! `root_ca_path`; the renewal task rebuilds its client when the root changes.
//! JWKS (jwt_authorities) is published to the JWT-SVID verifier.
//!
//! Fail-closed: on refresh failure, keep the LAST-KNOWN-GOOD root and retry with
//! backoff — never drop to an empty/unverified trust state.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Matches mock-cloud `trust_bundle_handler` / real Cloud trust-bundle endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct TrustBundleResponse {
    pub trust_bundle_pem: String,
    #[serde(default)]
    pub jwt_authorities: Vec<serde_json::Value>,
    #[serde(default = "default_refresh")]
    pub refresh_hint: u64,
}
fn default_refresh() -> u64 {
    3600
}

/// Fetch the trust bundle once (mTLS-authenticated client supplied by caller).
pub async fn fetch_trust_bundle(client: &reqwest::Client, base_url: &str) -> Result<TrustBundleResponse> {
    let url = format!("{}/v1/trust-bundle", base_url.trim_end_matches('/'));
    let res = client.get(&url).send().await.context("request trust bundle")?;
    anyhow::ensure!(res.status().is_success(), "trust-bundle fetch failed: {}", res.status());
    let tb: TrustBundleResponse = res.json().await.context("parse trust bundle")?;
    anyhow::ensure!(!tb.trust_bundle_pem.trim().is_empty(), "trust bundle PEM is empty");
    Ok(tb)
}

/// Atomically write the root PEM to `root_ca_path`. Returns true if it changed
/// (caller rebuilds the mTLS client on change).
pub fn install_root(tb: &TrustBundleResponse, root_ca_path: &str) -> Result<bool> {
    let new_pem = tb.trust_bundle_pem.trim();
    let prev = std::fs::read_to_string(root_ca_path).unwrap_or_default();
    if prev.trim() == new_pem {
        return Ok(false);
    }
    let tmp = format!("{root_ca_path}.tmp");
    std::fs::write(&tmp, format!("{new_pem}\n")).context("write tmp root CA")?;
    std::fs::rename(&tmp, root_ca_path).context("rename root CA")?;
    info!("trust bundle: installed new root CA at {root_ca_path}");
    Ok(true)
}

/// Background poller. Signals a root change via `roots_changed_tx` (monotonic
/// counter) so the renewal/mTLS layer can hot-rebuild; publishes JWKS via
/// `jwks_tx`. Fail-closed: keep LKG on error, exponential backoff.
#[allow(clippy::too_many_arguments)]
pub fn spawn_trust_bundle_poller(
    client: reqwest::Client,
    base_url: String,
    root_ca_path: String,
    jwks_tx: watch::Sender<Vec<serde_json::Value>>,
    roots_changed_tx: watch::Sender<u64>,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        info!("trust bundle poller started");
        let mut backoff = Duration::from_secs(30);
        let mut change_seq: u64 = 0;
        loop {
            let sleep_for = match fetch_trust_bundle(&client, &base_url).await {
                Ok(tb) => {
                    match install_root(&tb, &root_ca_path) {
                        Ok(true) => {
                            change_seq += 1;
                            let _ = roots_changed_tx.send(change_seq);
                        }
                        Ok(false) => {}
                        Err(e) => warn!("trust bundle: install failed: {e} (keeping LKG)"),
                    }
                    let _ = jwks_tx.send(tb.jwt_authorities.clone());
                    backoff = Duration::from_secs(30);
                    Duration::from_secs(tb.refresh_hint.max(60))
                }
                Err(e) => {
                    error!("trust bundle refresh failed: {e}; keeping LKG, retry in {:?}", backoff);
                    let b = backoff;
                    backoff = (backoff * 2).min(Duration::from_secs(600));
                    b
                }
            };
            tokio::select! {
                _ = cancel.cancelled() => { info!("trust bundle poller shutting down"); break; }
                _ = tokio::time::sleep(sleep_for) => {}
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_detects_change() {
        let dir = std::env::temp_dir().join(format!("tb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("root.crt").to_string_lossy().to_string();

        let tb1 = TrustBundleResponse { trust_bundle_pem: "AAA".into(), jwt_authorities: vec![], refresh_hint: 3600 };
        assert!(install_root(&tb1, &p).unwrap(), "first = changed");
        assert!(!install_root(&tb1, &p).unwrap(), "same = no change");
        let tb2 = TrustBundleResponse { trust_bundle_pem: "BBB".into(), ..tb1 };
        assert!(install_root(&tb2, &p).unwrap(), "new root = changed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_mock_shape() {
        let j = serde_json::json!({ "trust_bundle_pem": "X", "jwt_authorities": [], "refresh_hint": 1800 });
        let tb: TrustBundleResponse = serde_json::from_value(j).unwrap();
        assert_eq!(tb.refresh_hint, 1800);
    }
}
