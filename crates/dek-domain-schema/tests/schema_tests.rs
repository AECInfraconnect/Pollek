#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::duplicated_attributes
)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_domain_schema::tenant::Tenant;

#[test]
fn test_valid_tenant() {
    let json_str = r#"{
        "schema_version": "pollek.tenant.v1",
        "tenant_id": "ten_01HX",
        "tenant_type": "enterprise",
        "display_name": "ACME Bank",
        "trust_domain_strategy": "shared",
        "trust_domain": "spiffe://acme.internal",
        "data_region": "ap-southeast-1",
        "policy_mode": "enforce",
        "default_fail_mode": "fail_closed",
        "created_at": "2026-06-08T00:00:00Z"
    }"#;
    let tenant: Result<Tenant, _> = serde_json::from_str(json_str);
    assert!(tenant.is_ok());
}

#[test]
fn test_invalid_tenant() {
    let json_str = r#"{
        "schema_version": "pollek.tenant.v1",
        "tenant_id": "ten_01HX"
        // missing required fields
    }"#;
    let tenant: Result<Tenant, _> = serde_json::from_str(json_str);
    assert!(tenant.is_err());
}
