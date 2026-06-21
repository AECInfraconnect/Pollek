#![allow(clippy::unwrap_used)]
use dek_control_plane_api::bundle::{
    ActivationStrategy, BundleArtifactV2, PollenPolicyBundleManifestV2,
};
use serde_json::Value;

#[test]
fn test_serde_parity() {
    let manifest = PollenPolicyBundleManifestV2 {
        schema_version: "2.0".to_string(),
        bundle_version: "v1".to_string(),
        bundle_id: "bundle-local-1".to_string(),
        tenant_id: "local".to_string(),
        workspace_id: "default".to_string(),
        environment_id: "local".to_string(),
        build_number: 1,
        created_at: "2026-06-09T16:08:17.169165500+00:00".to_string(),
        expires_at: Some("2036-01-01T00:00:00Z".to_string()),
        created_by: "local-admin".to_string(),
        registry_snapshot_sha256:
            "4e7d2773e89b75eaf683b4604e5c510a08e8f8c423e18d1420fab0f483b06501".to_string(),
        router_config_sha256: "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
            .to_string(),
        artifacts: vec![BundleArtifactV2 {
            artifact_id: "e2e allow".to_string(),
            adapter_id: "cedar".to_string(),
            artifact_type: "cedar_text".to_string(),
            path: "artifacts/bacfa3fab8b34a87fbe68d2b949b8fb62bc093942a84fd9a08543f15a0b63ee2"
                .to_string(),
            sha256: "bacfa3fab8b34a87fbe68d2b949b8fb62bc093942a84fd9a08543f15a0b63ee2".to_string(),
            size_bytes: 36,
            entrypoint: None,
            data_path: None,
            schema_path: None,
            entities_path: None,
        }],
        signatures: vec![],
        min_dek_version: "0.1.0".to_string(),
        activation_strategy: ActivationStrategy::AtomicAllOrNothing,
        rollback_from: None,
    };

    let signed_bytes = serde_json::to_vec(&manifest).unwrap();
    println!("LCP signed bytes len: {}", signed_bytes.len());
    println!(
        "LCP signed string: {}",
        String::from_utf8_lossy(&signed_bytes)
    );

    // Convert to serde_json::Value
    let val = serde_json::to_value(&manifest).unwrap();

    // Serialize to string like the server does
    let json_str = serde_json::to_string(&val).unwrap();

    // Parse back
    let manifest_val: Value = serde_json::from_str(&json_str).unwrap();

    let mut manifest_sync: PollenPolicyBundleManifestV2 =
        serde_json::from_value(manifest_val).unwrap();
    manifest_sync.signatures.clear();
    let sync_bytes = serde_json::to_vec(&manifest_sync).unwrap();

    println!("SYNC signed bytes len: {}", sync_bytes.len());
    println!(
        "SYNC signed string: {}",
        String::from_utf8_lossy(&sync_bytes)
    );

    assert_eq!(signed_bytes, sync_bytes);
}
