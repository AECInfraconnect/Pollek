#![allow(clippy::unwrap_used, clippy::expect_used)]

use dek_spire_node::describe_svid;
use rcgen::{CertificateParams, KeyPair, SanType};

fn issue_svid(spiffe: &str) -> String {
    let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
    params
        .subject_alt_names
        .push(SanType::URI(spiffe.try_into().unwrap()));
    let key = KeyPair::generate().unwrap();
    params.self_signed(&key).unwrap().pem()
}

#[test]
fn reads_spiffe_id_and_validity() {
    let pem = issue_svid("spiffe://pollek.local/device/abc123");
    let info = describe_svid(&pem, 0).unwrap();
    assert_eq!(
        info.spiffe_id.as_deref(),
        Some("spiffe://pollek.local/device/abc123")
    );
    assert!(info.not_after_unix > info.not_before_unix);
    assert!(!info.expired);
    assert!(info.seconds_until_expiry > 0);
}

#[test]
fn flags_expired_relative_to_now() {
    let pem = issue_svid("spiffe://pollek.local/device/xyz");
    let not_after = describe_svid(&pem, 0).unwrap().not_after_unix;
    let info = describe_svid(&pem, not_after + 10).unwrap();
    assert!(info.expired);
    assert!(info.seconds_until_expiry < 0);
}

#[test]
fn rejects_non_pem() {
    assert!(describe_svid("not a cert", 0).is_err());
}
