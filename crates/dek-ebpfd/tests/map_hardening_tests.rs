#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::duplicated_attributes
)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_domain_schema::ebpf::{EbpfMapUpdate, UpdateSource};
use dek_ebpfd::map_updater::MapUpdater;
use serde_json::json;

#[test]
fn test_valid_bundle_update() {
    let mut updater = MapUpdater::new("tenant-abc".to_string(), "device-123".to_string(), 10);

    let update = EbpfMapUpdate {
        schema_version: "1.0".to_string(),
        map_name: "LPM_MAP".to_string(),
        operation: "insert".to_string(),
        source: UpdateSource::Bundle,
        tenant_id: "tenant-abc".to_string(),
        device_id: "device-123".to_string(),
        generation: 11,
        key: json!({"ip": "192.168.1.1", "prefix": 32}),
        value: json!({"allow": 1, "log_event": 1}),
        signature: None,
    };

    assert!(updater.apply_update(update).is_ok());
    assert_eq!(updater.current_generation, 11);
}

#[test]
fn test_unauthorized_tenant() {
    let mut updater = MapUpdater::new("tenant-abc".to_string(), "device-123".to_string(), 10);

    let update = EbpfMapUpdate {
        schema_version: "1.0".to_string(),
        map_name: "LPM_MAP".to_string(),
        operation: "insert".to_string(),
        source: UpdateSource::Bundle,
        tenant_id: "tenant-malicious".to_string(), // Mismatch
        device_id: "device-123".to_string(),
        generation: 11,
        key: json!({"ip": "192.168.1.1", "prefix": 32}),
        value: json!({"allow": 1, "log_event": 1}),
        signature: None,
    };

    let result = updater.apply_update(update);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "Unauthorized map update: tenant_id mismatch"
    );
}

#[test]
fn test_anti_rollback_generation() {
    let mut updater = MapUpdater::new("tenant-abc".to_string(), "device-123".to_string(), 20); // Current is 20

    let update = EbpfMapUpdate {
        schema_version: "1.0".to_string(),
        map_name: "LPM_MAP".to_string(),
        operation: "insert".to_string(),
        source: UpdateSource::Bundle,
        tenant_id: "tenant-abc".to_string(),
        device_id: "device-123".to_string(),
        generation: 19, // Older generation
        key: json!({"ip": "192.168.1.1", "prefix": 32}),
        value: json!({"allow": 1, "log_event": 1}),
        signature: None,
    };

    let result = updater.apply_update(update);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "Unauthorized map update: generation rollback attempt"
    );
}

#[test]
fn test_high_risk_map_requires_signature() {
    let mut updater = MapUpdater::new("tenant-abc".to_string(), "device-123".to_string(), 10);

    // VERDICT_MAP is high-risk, should require a signature even if from bundle
    let mut update = EbpfMapUpdate {
        schema_version: "1.0".to_string(),
        map_name: "VERDICT_MAP".to_string(),
        operation: "insert".to_string(),
        source: UpdateSource::Bundle,
        tenant_id: "tenant-abc".to_string(),
        device_id: "device-123".to_string(),
        generation: 11,
        key: json!({"ip": "192.168.1.1", "prefix": 32}),
        value: json!({"allow": 0, "log_event": 1}),
        signature: None, // Missing signature
    };

    let result = updater.apply_update(update.clone());
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "Unauthorized map update: signature strictly required for this source/map"
    );

    // Add signature, should pass
    update.signature = Some("dummy-sig-123".to_string());
    assert!(updater.apply_update(update).is_ok());
}

#[test]
fn test_out_of_band_requires_signature() {
    let mut updater = MapUpdater::new("tenant-abc".to_string(), "device-123".to_string(), 10);

    let mut update = EbpfMapUpdate {
        schema_version: "1.0".to_string(),
        map_name: "LPM_MAP".to_string(),
        operation: "insert".to_string(),
        source: UpdateSource::OutOfBand,
        tenant_id: "tenant-abc".to_string(),
        device_id: "device-123".to_string(),
        generation: 11,
        key: json!({"ip": "192.168.1.1", "prefix": 32}),
        value: json!({"allow": 1, "log_event": 1}),
        signature: None, // Missing signature
    };

    let result = updater.apply_update(update.clone());
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "Unauthorized map update: signature strictly required for this source/map"
    );

    // Add signature, should pass
    update.signature = Some("dummy-sig-123".to_string());
    assert!(updater.apply_update(update).is_ok());
}
