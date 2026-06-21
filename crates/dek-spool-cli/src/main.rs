use dek_secure_spool::{
    key_manager::SpoolKeyManager,
    os::DefaultOsKeyStore,
    segment::{SegmentWriter, TelemetryEvent},
};
use std::path::PathBuf;
use uuid::Uuid;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Initialize OS-specific KeyStore...");

    #[cfg(windows)]
    let store = DefaultOsKeyStore::new(PathBuf::from("master.key.wrapped"));

    #[cfg(target_os = "linux")]
    let store = DefaultOsKeyStore::new(PathBuf::from("master.key.fallback"));

    #[cfg(target_os = "macos")]
    let store = DefaultOsKeyStore::new();

    let key_manager = SpoolKeyManager::new(store);
    let active_key = key_manager.active_aead_key()?;

    println!("Active AEAD Key ID: {}", active_key.key_id());

    let event = TelemetryEvent {
        schema_version: "pollen.telemetry.v1".to_string(),
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

    let spool_file = std::path::Path::new("seg-demo.pds");
    let mut writer = SegmentWriter::create(spool_file, "tnt_demo", "dev_demo", "seg_demo_001")?;

    println!("Appending encrypted event to {}...", spool_file.display());
    writer.append_event(&active_key, &event)?;

    println!("Success! Spool file created and record encrypted securely.");

    Ok(())
}
