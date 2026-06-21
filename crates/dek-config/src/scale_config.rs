// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! scale_config.rs — freshness/sync + SaaS-scale tunables.
//!
//! These live in `DekConfig` and arrive in the SIGNED bundle/config from the
//! cloud (not hand-edited locally). All fields use serde defaults so existing
//! bundles that omit them keep working (non-breaking).

use serde::{Deserialize, Serialize};

/// Phase 1/2 — policy freshness + sync cadence (consumed by dek-policy-syncer).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncerConfig {
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    /// Hard cap on time since last SUCCESSFUL sync. Beyond this => strict deny,
    /// independent of the bundle's own expiry (network-partition guard).
    #[serde(default = "default_max_bundle_age_secs")]
    pub max_bundle_age_secs: i64,
    /// After a bundle's own `expires_at`, keep enforcing (LKG) for this long
    /// before flipping to strict deny.
    #[serde(default = "default_grace_secs")]
    pub grace_secs: i64,
    /// `/v1/keys` suffix for rotation. Empty => derive from cloud_url.
    #[serde(default)]
    pub keys_path_suffix: String,
}

fn default_poll_interval_secs() -> u64 {
    60
}
fn default_max_bundle_age_secs() -> i64 {
    86_400
} // 24h
fn default_grace_secs() -> i64 {
    600
}

impl Default for SyncerConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: default_poll_interval_secs(),
            max_bundle_age_secs: default_max_bundle_age_secs(),
            grace_secs: default_grace_secs(),
            keys_path_suffix: String::new(),
        }
    }
}

/// Phase 4 — backpressure + circuit breaker tunables (consumed by PEP via dek-resilience).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScaleConfig {
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    #[serde(default = "default_max_concurrent_per_tenant")]
    pub max_concurrent_per_tenant: usize,
    /// Per-evaluator (PDP) call timeout (ms). Timeout => fail-closed (deny).
    #[serde(default = "default_pdp_timeout_ms")]
    pub pdp_timeout_ms: u64,
    #[serde(default = "default_breaker_failure_threshold")]
    pub breaker_failure_threshold: u32,
    #[serde(
        alias = "auto_recovery_delay",
        default = "default_breaker_cooldown_secs"
    )]
    pub breaker_cooldown_secs: u64,
}

fn default_max_concurrent() -> usize {
    512
}
fn default_max_concurrent_per_tenant() -> usize {
    64
}
fn default_pdp_timeout_ms() -> u64 {
    200
}
fn default_breaker_failure_threshold() -> u32 {
    5
}
fn default_breaker_cooldown_secs() -> u64 {
    10
}

impl Default for ScaleConfig {
    fn default() -> Self {
        Self {
            max_concurrent: default_max_concurrent(),
            max_concurrent_per_tenant: default_max_concurrent_per_tenant(),
            pdp_timeout_ms: default_pdp_timeout_ms(),
            breaker_failure_threshold: default_breaker_failure_threshold(),
            breaker_cooldown_secs: default_breaker_cooldown_secs(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn defaults_are_safe() {
        assert_eq!(SyncerConfig::default().max_bundle_age_secs, 86_400);
        assert_eq!(ScaleConfig::default().pdp_timeout_ms, 200);
    }

    #[test]
    fn deserializes_with_missing_fields_using_defaults() {
        let s: SyncerConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(s, SyncerConfig::default());
        let sc: ScaleConfig = serde_json::from_str(r#"{"max_concurrent": 1024}"#).unwrap();
        assert_eq!(sc.max_concurrent, 1024);
        assert_eq!(sc.pdp_timeout_ms, 200); // default preserved
    }
}
