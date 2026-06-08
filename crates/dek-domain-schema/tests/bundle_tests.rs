use chrono::{Duration, Utc};
use dek_domain_schema::bundle::{ActivationMode, BundleArtifact, BundleManifest, LkgState};
use sha2::{Digest, Sha256};

fn create_test_manifest(
    generation: u64,
    expiry_offset: Duration,
    artifact_name: &str,
    artifact_content: &[u8],
) -> BundleManifest {
    let mut hasher = Sha256::new();
    hasher.update(artifact_content);
    let hash_bytes = hasher.finalize();
    let hash = hash_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();

    BundleManifest {
        schema_version: "pollen.bundle.v1".to_string(),
        bundle_id: format!("bnd-{}", generation),
        bundle_version: format!("1.0.{}", generation),
        bundle_generation: generation,
        tenant_id: "t-1".to_string(),
        created_at: Utc::now().to_rfc3339(),
        expires_at: (Utc::now() + expiry_offset).to_rfc3339(),
        activation_mode: ActivationMode::Full,
        artifacts: vec![BundleArtifact {
            name: artifact_name.to_string(),
            artifact_type: "cedar".to_string(),
            sha256: hash,
            url: None,
        }],
    }
}

#[test]
fn test_bundle_expiry() {
    let unexpired = create_test_manifest(1, Duration::days(1), "policy.cedar", b"permit();");
    assert!(!unexpired.is_expired(Utc::now()).unwrap());

    let expired = create_test_manifest(2, Duration::days(-1), "policy.cedar", b"permit();");
    assert!(expired.is_expired(Utc::now()).unwrap());
}

#[test]
fn test_bundle_anti_rollback() {
    let manifest_gen_10 = create_test_manifest(10, Duration::days(1), "policy.cedar", b"permit();");

    // Ok if current gen is 9
    assert!(manifest_gen_10.validate_anti_rollback(9).is_ok());

    // Ok if current gen is 10
    assert!(manifest_gen_10.validate_anti_rollback(10).is_ok());

    // Error if current gen is 11
    assert!(manifest_gen_10.validate_anti_rollback(11).is_err());
}

#[test]
fn test_bundle_artifact_hash() {
    let content = b"permit(principal == User::\"alice\");";
    let manifest = create_test_manifest(1, Duration::days(1), "policy.cedar", content);

    // Valid hash
    assert!(manifest.validate_artifact_hash("policy.cedar", content).is_ok());

    // Invalid hash
    assert!(manifest.validate_artifact_hash("policy.cedar", b"deny();").is_err());

    // Artifact not found
    assert!(manifest.validate_artifact_hash("missing.cedar", content).is_err());
}

#[test]
fn test_lkg_fallback() {
    let mut state = LkgState::new();
    assert!(state.rollback().is_err());

    let v1 = create_test_manifest(1, Duration::days(1), "1", b"1");
    let v2 = create_test_manifest(2, Duration::days(1), "2", b"2");

    state.apply_new_manifest(v1.clone());
    assert!(state.fallback_manifest.is_none());

    state.apply_new_manifest(v2.clone());
    assert_eq!(state.current_manifest.as_ref().unwrap().bundle_generation, 2);
    assert_eq!(state.fallback_manifest.as_ref().unwrap().bundle_generation, 1);

    // Simulated failure requires rollback
    let rolled_back = state.rollback().unwrap();
    assert_eq!(rolled_back.bundle_generation, 1);
    assert_eq!(state.current_manifest.as_ref().unwrap().bundle_generation, 1);
    assert!(state.fallback_manifest.is_none());
}
