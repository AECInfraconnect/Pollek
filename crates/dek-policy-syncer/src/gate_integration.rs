//! Integration: prove the cross-process gate fails closed and recovers.
//! Run: cargo test -p dek-policy-syncer --test gate_integration

use dek_policy_syncer::gate::{invalidate_cache, strict_deny_reason};
use dek_policy_syncer::state::{
    write_status_atomic, EnforcementState, EnforcementStatus,
};

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

// NOTE: these tests share the process-global status file path
// (data_dir/state/enforcement_state.json). They run serially via a mutex to
// avoid cross-test interference, and invalidate the gate cache between steps.
use std::sync::Mutex;
static SERIAL: Mutex<()> = Mutex::new(());

#[test]
fn gate_denies_then_allows_then_denies() {
    let _g = SERIAL.lock().unwrap();

    // 1) StrictDeny published -> gate denies with reason.
    write_status_atomic(&EnforcementStatus {
        state: EnforcementState::StrictDeny { since_unix: now(), reason: "bundle_expired_beyond_grace".into() },
        updated_unix: now(),
        bundle_version: Some("1_1_1".into()),
    })
    .unwrap();
    invalidate_cache();
    assert_eq!(strict_deny_reason().as_deref(), Some("bundle_expired_beyond_grace"));

    // 2) Active published -> gate allows (None).
    write_status_atomic(&EnforcementStatus {
        state: EnforcementState::Active { expires_at_unix: now() + 3600 },
        updated_unix: now(),
        bundle_version: Some("2_2_2".into()),
    })
    .unwrap();
    invalidate_cache();
    assert_eq!(strict_deny_reason(), None);

    // 3) GracePeriod -> still allows (LKG enforcing).
    write_status_atomic(&EnforcementStatus {
        state: EnforcementState::GracePeriod { deadline_unix: now() + 60, reason: "bundle_expired_grace".into() },
        updated_unix: now(),
        bundle_version: Some("2_2_2".into()),
    })
    .unwrap();
    invalidate_cache();
    assert_eq!(strict_deny_reason(), None, "grace period must keep enforcing (allow path)");

    // 4) Back to StrictDeny -> denies again.
    write_status_atomic(&EnforcementStatus {
        state: EnforcementState::StrictDeny { since_unix: now(), reason: "max_bundle_age_exceeded(100s>50s)".into() },
        updated_unix: now(),
        bundle_version: None,
    })
    .unwrap();
    invalidate_cache();
    assert!(strict_deny_reason().is_some());

    // cleanup
    let _ = std::fs::remove_file(dek_policy_syncer::state::status_path());
}

#[test]
fn gate_fails_closed_when_status_absent() {
    let _g = SERIAL.lock().unwrap();
    // Ensure no status file -> gate MUST deny (never fail-open).
    let _ = std::fs::remove_file(dek_policy_syncer::state::status_path());
    invalidate_cache();
    assert_eq!(strict_deny_reason().as_deref(), Some("enforcement_status_unavailable"));
}
