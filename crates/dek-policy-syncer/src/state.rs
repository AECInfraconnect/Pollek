//! state.rs — fail-safe EnforcementState + the PURE freshness decision.
//!
//! The heart of Phase 1. `evaluate_state` is a pure function (fully unit
//! testable, no I/O) that maps (now, bundle expiry, last successful sync, cfg)
//! to an EnforcementState. The PEP consults this state and FAILS CLOSED
//! (strict deny) when the policy bundle is stale or absent.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Runtime enforcement posture derived from policy freshness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum EnforcementState {
    /// Bundle is current — enforce normally.
    Active { expires_at_unix: i64 },
    /// Bundle expired but within grace — keep enforcing LKG (cloud blip).
    GracePeriod { deadline_unix: i64, reason: String },
    /// Stale/absent policy — BLOCK all policy-gated traffic (default deny).
    StrictDeny { since_unix: i64, reason: String },
}

impl EnforcementState {
    pub fn is_strict_deny(&self) -> bool {
        matches!(self, EnforcementState::StrictDeny { .. })
    }
    pub fn label(&self) -> &'static str {
        match self {
            EnforcementState::Active { .. } => "active",
            EnforcementState::GracePeriod { .. } => "grace",
            EnforcementState::StrictDeny { .. } => "strict_deny",
        }
    }
    /// Numeric code for the `dek_enforcement_state` gauge (0/1/2).
    pub fn gauge(&self) -> f64 {
        match self {
            EnforcementState::Active { .. } => 0.0,
            EnforcementState::GracePeriod { .. } => 1.0,
            EnforcementState::StrictDeny { .. } => 2.0,
        }
    }
    pub fn reason(&self) -> String {
        match self {
            EnforcementState::Active { .. } => "active".into(),
            EnforcementState::GracePeriod { reason, .. } => reason.clone(),
            EnforcementState::StrictDeny { reason, .. } => reason.clone(),
        }
    }
}

/// Freshness thresholds (sourced from signed config; sane defaults here).
#[derive(Debug, Clone)]
pub struct FreshnessConfig {
    /// Hard cap on time since last SUCCESSFUL sync. Beyond this -> strict deny,
    /// independent of the bundle's own expiry. Guards against stale enforcement
    /// during a long network partition (e.g. 24h).
    pub max_bundle_age_secs: i64,
    /// After a bundle's own `expires_at`, keep enforcing for this long (LKG)
    /// before flipping to strict deny.
    pub grace_secs: i64,
}

impl Default for FreshnessConfig {
    fn default() -> Self {
        Self { max_bundle_age_secs: 86_400, grace_secs: 600 }
    }
}

/// PURE decision. No I/O. This is the contract that Phase 1 tests pin down.
///
/// - `bundle_expires_at_unix = None`  => no active bundle (cold start) -> deny
/// - `last_sync_unix = None`          => never synced               -> treated as age 0 from epoch
///
/// Precedence: cold-start deny > max_bundle_age deny > bundle-expiry(grace) > active.
pub fn evaluate_state(
    now_unix: i64,
    bundle_expires_at_unix: Option<i64>,
    last_sync_unix: Option<i64>,
    cfg: &FreshnessConfig,
) -> EnforcementState {
    // 1) Cold start: no bundle at all -> fail closed.
    let Some(expires) = bundle_expires_at_unix else {
        return EnforcementState::StrictDeny {
            since_unix: now_unix,
            reason: "no_bundle_cold_start".into(),
        };
    };

    // 2) Config-enforced staleness (network partition guard).
    let last_sync = last_sync_unix.unwrap_or(0);
    let sync_age = now_unix.saturating_sub(last_sync);
    if sync_age > cfg.max_bundle_age_secs {
        return EnforcementState::StrictDeny {
            since_unix: now_unix,
            reason: format!(
                "max_bundle_age_exceeded({}s>{}s)",
                sync_age, cfg.max_bundle_age_secs
            ),
        };
    }

    // 3) Bundle's own expiry.
    if now_unix <= expires {
        return EnforcementState::Active { expires_at_unix: expires };
    }
    // 4) Expired but within grace -> still enforce LKG.
    if now_unix <= expires + cfg.grace_secs {
        return EnforcementState::GracePeriod {
            deadline_unix: expires + cfg.grace_secs,
            reason: "bundle_expired_grace".into(),
        };
    }
    // 5) Beyond grace -> deny.
    EnforcementState::StrictDeny {
        since_unix: now_unix,
        reason: "bundle_expired_beyond_grace".into(),
    }
}

// ---------------------------------------------------------------------------
// Cross-process status file. dek-core's syncer WRITES it; the PEP (separate
// process) READS it to gate requests. Atomic write, world-readable JSON.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnforcementStatus {
    pub state: EnforcementState,
    pub updated_unix: i64,
    pub bundle_version: Option<String>,
}

/// `<data_dir>/state/enforcement_state.json`
pub fn status_path() -> PathBuf {
    dek_config::paths::get_data_dir()
        .join("state")
        .join("enforcement_state.json")
}

pub fn write_status_atomic(status: &EnforcementStatus) -> anyhow::Result<()> {
    let path = status_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(status)?;
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Returns None if the file is absent/unreadable (caller MUST treat None as
/// fail-closed — never as "allow").
pub fn read_status() -> Option<EnforcementStatus> {
    let bytes = std::fs::read(status_path()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> FreshnessConfig {
        FreshnessConfig { max_bundle_age_secs: 100, grace_secs: 10 }
    }

    #[test]
    fn cold_start_no_bundle_denies() {
        let s = evaluate_state(1_000, None, None, &cfg());
        assert!(s.is_strict_deny());
        assert_eq!(s.reason(), "no_bundle_cold_start");
    }

    #[test]
    fn fresh_bundle_active() {
        // now=1000, expires=2000, synced at 990 -> active
        let s = evaluate_state(1_000, Some(2_000), Some(990), &cfg());
        assert_eq!(s, EnforcementState::Active { expires_at_unix: 2_000 });
    }

    #[test]
    fn expired_within_grace_is_grace() {
        // now=2005, expires=2000, grace=10, synced recently -> grace until 2010
        let s = evaluate_state(2_005, Some(2_000), Some(2_000), &cfg());
        assert!(matches!(s, EnforcementState::GracePeriod { .. }));
    }

    #[test]
    fn expired_beyond_grace_denies() {
        let s = evaluate_state(2_011, Some(2_000), Some(2_000), &cfg());
        assert!(s.is_strict_deny());
        assert_eq!(s.reason(), "bundle_expired_beyond_grace");
    }

    #[test]
    fn max_bundle_age_denies_even_if_bundle_not_expired() {
        // bundle wouldn't expire until 10_000, but last sync was 1000s ago
        // (> max_bundle_age=100) -> strict deny (network partition guard)
        let s = evaluate_state(2_000, Some(10_000), Some(900), &cfg());
        assert!(s.is_strict_deny());
        assert!(s.reason().starts_with("max_bundle_age_exceeded"));
    }
}
