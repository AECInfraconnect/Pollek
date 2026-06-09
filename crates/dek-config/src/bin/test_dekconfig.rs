use dek_config::DekConfig;

fn main() {
    let json = serde_json::json!({
        "device_id": "device-001",
        "tenant_id": "tenant-production-1",
        "mtls": { "client_cert_path": "certs/client.crt", "client_key_path": "certs/client.key", "root_ca_path": "certs/root_ca.crt" },
        "spire_server": { "endpoint": "https://127.0.0.1:43891/spire" },
        "jwt_config": { "public_key_pem": "test", "issuer_url": "https://127.0.0.1:43891", "audience": ["pollen-dek"] },
        "policy_config": {
            "version": "1.0",
            "policy": { "engine": "cedar" },
            "openfga": { "endpoint": "http://127.0.0.1:8080", "store_id": "test" },
            "cedar": { "policy_src": "permit" },
            "opa_wasm": { "policy_path": "test" },
            "routes": [
                { "id": "route_tools_call", "priority": 100,
                  "match_rule": { "method": "tools/call", "tool_category": null },
                  "pdp_required": ["cedar"],
                  "pdp_conditional": [] },
                { "id": "route_default", "priority": 10,
                  "match_rule": { "method": "*", "tool_category": null },
                  "pdp_required": ["cedar"], "pdp_conditional": [] }
            ]
        }
    });

    match serde_json::from_value::<DekConfig>(json) {
        Ok(config) => {
            let val = serde_json::to_value(&config.policy_config).unwrap();
            println!("Serialized policy_config: {}", val);
        }
        Err(e) => {
            println!("Error parsing DekConfig: {:?}", e);
        }
    }
}
