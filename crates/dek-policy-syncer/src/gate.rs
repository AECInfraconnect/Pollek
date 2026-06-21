//! gate.rs — PEP-side fail-safe gate (cross-process).
//!
//! The PEP (dek-mcp-proxy / dek-ext-authz) runs as a SEPARATE process from the
//! syncer (dek-core). It reads the enforcement status file written by the syncer
//! and FAILS CLOSED: stale/absent/unreadable status => deny.
//!
//! The file read is cached (~1s) so request throughput is unaffected regardless
//! of RPS. If the status is missing entirely (cold start / syncer not up yet),
//! the gate denies — never allows.

use crate::state::{read_status, EnforcementState};
use std::sync::Mutex;
use std::time::{Duration, Instant};

struct Cache {
    fetched_at: Instant,
    deny_reason: Option<String>,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
const REFRESH: Duration = Duration::from_secs(1);

/// Returns `Some(reason)` if the PEP must short-circuit DENY, else `None`
/// (policy evaluation may proceed). Fail-closed on every uncertainty.
pub fn strict_deny_reason() -> Option<String> {
    let mut guard = CACHE.lock().ok()?;
    let fresh = guard
        .as_ref()
        .map(|c| c.fetched_at.elapsed() < REFRESH)
        .unwrap_or(false);

    if !fresh {
        let deny_reason = match read_status() {
            Some(status) => match status.state {
                EnforcementState::StrictDeny { reason, .. } => Some(reason),
                EnforcementState::Active { .. } | EnforcementState::GracePeriod { .. } => None,
            },
            // No status file => syncer hasn't proven freshness => fail closed.
            None => Some("enforcement_status_unavailable".into()),
        };
        *guard = Some(Cache { fetched_at: Instant::now(), deny_reason });
    }
    guard.as_ref().and_then(|c| c.deny_reason.clone())
}

/// Force a cache refresh on next call (e.g., after a known state change).
pub fn invalidate_cache() {
    if let Ok(mut g) = CACHE.lock() {
        *g = None;
    }
}
