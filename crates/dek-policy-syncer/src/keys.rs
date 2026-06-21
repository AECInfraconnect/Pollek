// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! keys.rs — key distribution & rotation orchestration (Phase 2).
//!
//! Fetches `/v1/keys` over mTLS, verifies the payload is signed by a CURRENTLY
//! trusted key (chain of trust — a rogue cannot inject keys), merges the
//! rotation, and persists the set. The verify primitive lives in
//! dek-bundle-sync::keys (TrustedKeySet); this module is the orchestration.

use anyhow::Result;
use dek_bundle_sync::keys::{
    parse_signatures, RotationDelta, TrustedKey, TrustedKeySet, VerifyOutcome,
};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `<data_dir>/state/trusted_keys.json`
pub fn keys_path() -> PathBuf {
    dek_config::paths::get_data_dir()
        .join("state")
        .join("trusted_keys.json")
}

/// Load the persisted set, else bootstrap from the single pinned key delivered
/// at enrollment.
pub fn load_or_bootstrap(pinned_b64: &str) -> TrustedKeySet {
    let path = keys_path();
    if let Ok(bytes) = std::fs::read(&path) {
        if let Ok(set) = serde_json::from_slice::<TrustedKeySet>(&bytes) {
            if !set.keys.is_empty() {
                tracing::info!(
                    "DEBUG KEYMGR: Loaded TrustedKeySet from {}: {:?}",
                    path.display(),
                    set.keys.first().map(|k| &k.public_b64)
                );
                return set;
            }
        }
    }
    tracing::info!(
        "DEBUG KEYMGR: Bootstrapping from single pinned key: {}",
        pinned_b64
    );
    TrustedKeySet::from_single_pinned(pinned_b64)
}

pub fn persist(set: &TrustedKeySet) -> Result<()> {
    let path = keys_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(set)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Fetch `/v1/keys` and merge after chain-of-trust verification.
///
/// The endpoint returns `{ "signed": { "keys": [TrustedKey...] }, "signatures": [...] }`.
/// The `signed` payload MUST verify against a key ALREADY in `current` — this is
/// what prevents an attacker (or a compromised transport) from injecting new
/// trusted keys. Returns the merged set + what changed.
pub async fn fetch_and_merge(
    client: &reqwest::Client,
    keys_url: &str,
    api_token: Option<&str>,
    current: &TrustedKeySet,
) -> std::result::Result<(TrustedKeySet, RotationDelta), crate::SyncError> {
    let mut req = client.get(keys_url);
    if let Some(t) = api_token {
        req = req.header("Authorization", format!("Bearer {}", t));
    }
    let res = req
        .send()
        .await
        .map_err(|e| crate::SyncError::Fetch(format!("GET /v1/keys failed: {}", e)))?;
    if !res.status().is_success() {
        return Err(crate::SyncError::Fetch(format!(
            "keys fetch failed: HTTP {}",
            res.status()
        )));
    }
    let body: serde_json::Value = res
        .json()
        .await
        .map_err(|e| crate::SyncError::Fetch(format!("parse /v1/keys: {}", e)))?;

    // Chain of trust: verify `signed` with a CURRENTLY trusted key.
    let signed = body
        .get("signed")
        .ok_or_else(|| crate::SyncError::Verify("missing 'signed'".into()))?;
    let signed_bytes = serde_json::to_vec(signed)
        .map_err(|e| crate::SyncError::Verify(format!("serialize signed: {}", e)))?;
    let sigs = parse_signatures(body.get("signatures").unwrap_or(&serde_json::Value::Null));
    match current.verify(now_unix(), &signed_bytes, &sigs) {
        VerifyOutcome::Valid { key_id } => {
            info!(
                "[KeyMgr] /v1/keys payload verified by trusted key '{}'",
                key_id
            );
        }
        other => {
            // SECURITY: refuse to merge keys that aren't vouched for by a key we
            // already trust. This is the rogue-key-injection guard.
            return Err(crate::SyncError::Verify(format!(
                "payload not signed by a trusted key ({:?})",
                other
            )));
        }
    }

    let incoming: Vec<TrustedKey> = serde_json::from_value(
        signed
            .get("keys")
            .cloned()
            .ok_or_else(|| crate::SyncError::Verify("missing signed.keys".into()))?,
    )
    .map_err(|e| crate::SyncError::Verify(format!("parse keys list: {}", e)))?;

    let mut merged = current.clone();
    let delta = merged.merge_rotation(incoming);
    if !delta.is_empty() {
        warn!(
            "[KeyMgr] key rotation: added={:?} promoted={:?} revoked={:?}",
            delta.added, delta.promoted, delta.revoked
        );
        persist(&merged)
            .map_err(|e| crate::SyncError::Activation(format!("failed to persist keys: {}", e)))?;
    }
    Ok((merged, delta))
}
