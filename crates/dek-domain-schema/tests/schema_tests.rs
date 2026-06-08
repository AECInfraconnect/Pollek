use dek_domain_schema::tenant::Tenant;

#[test]
fn test_valid_tenant() {
    let json_str = r#"{
        "schema_version": "pollen.tenant.v1",
        "tenant_id": "ten_01HX",
        "tenant_type": "enterprise",
        "display_name": "ACME Bank",
        "trust_domain_strategy": "shared",
        "data_region": "ap-southeast-1",
        "policy_mode": "enforce",
        "created_at": "2026-06-08T00:00:00Z"
    }"#;
    let tenant: Result<Tenant, _> = serde_json::from_str(json_str);
    assert!(tenant.is_ok());
}

#[test]
fn test_invalid_tenant() {
    let json_str = r#"{
        "schema_version": "pollen.tenant.v1",
        "tenant_id": "ten_01HX"
        // missing required fields
    }"#;
    let tenant: Result<Tenant, _> = serde_json::from_str(json_str);
    assert!(tenant.is_err());
}
