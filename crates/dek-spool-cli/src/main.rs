use dek_secure_spool::{
    key_manager::SpoolKeyManager,
    os::DefaultOsKeyStore,
    segment::{SegmentWriter, TelemetryEvent},
};
use uuid::Uuid;

#[allow(clippy::print_stdout)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Keep every artifact this demo produces (wrapped key + spool segment) inside a
    // dedicated temp working directory, so running it never litters the current
    // directory or the repository root.
    let work_dir = std::env::temp_dir().join("dek-spool-demo");
    std::fs::create_dir_all(&work_dir)?;
    println!("Demo working directory: {}", work_dir.display());

    println!("Initialize OS-specific KeyStore...");
    #[cfg(windows)]
    let key_path = work_dir.join("master.key.wrapped");
    #[cfg(not(windows))]
    let key_path = work_dir.join("master.key.fallback");
    let store = DefaultOsKeyStore::new(key_path);

    let key_manager = SpoolKeyManager::new(store);
    let active_key = key_manager.active_aead_key()?;

    println!("Active AEAD Key ID: {}", active_key.key_id());

    let event = TelemetryEvent {
        schema_version: "pollek.telemetry.v1".to_string(),
        event_id: Uuid::new_v4(),
        tenant_id: "tnt_demo".to_string(),
        device_id: "dev_demo".to_string(),
        event_type: "policy.decision".to_string(),
        timestamp_unix_ms: 1760000000000,
        body: serde_json::json!({
            "decision": "deny",
            "policy_id": "pol_001",
            "reason_code": "network.egress.not_allowed",
            "subject": {
                "process_path_hash": "hmac-sha256:example_hash",
                "user_hash": "hmac-sha256:example_user"
            }
        }),
    };

    let spool_file = work_dir.join("seg-demo.pds");
    let mut writer = SegmentWriter::create(&spool_file, "tnt_demo", "dev_demo", "seg_demo_001")?;

    println!("Appending encrypted event to {}...", spool_file.display());
    writer.append_event(&active_key, &event)?;

    println!("Success! Spool file created and record encrypted securely.");

    Ok(())
}
