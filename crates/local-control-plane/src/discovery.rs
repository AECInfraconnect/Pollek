use crate::state::AppState;
use axum::{routing::get, Json, Router};

pub fn router() -> Router<AppState> {
    Router::new().route("/.well-known/pollek-contract", get(get_discovery))
}

async fn get_discovery() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "schema_version": "contract-discovery.v1",
        "supported": ["1.0"],
        "preferred": "1.0",
        "contract_version": "2026.06.26",
        "compatible_cloud_contracts": [">=2026.06.01 <2026.09.00"],
        "minimum_dek_version": "1.0.0-beta.6",
        "sunset": { "0.9": "2026-10-01T00:00:00Z" },
        "schemas": {
            "contract.discovery": "contract-discovery.v1.schema.json",
            "local.capability_snapshot": "local-capability-snapshot.v2.schema.json",
            "security.coverage": "security-coverage.v1.schema.json",
            "user.message_catalog": "user-message-catalog.v1.schema.json",
            "registered_agent.identity_binding": "registered-agent-identity-binding.v1.schema.json",
            "telemetry.batch": "telemetry-envelope.v1.schema.json",
            "identity.access": "identity-access.v1.schema.json",
            "resource.access": "resource-access.v1.schema.json",
            "tool.usage": "tool-usage.v1.schema.json"
        },
        "interfaces": {
            "local.dashboard.capability_cards": {
                "schema": "local-capability-snapshot.v2.schema.json",
                "direction": "local_to_dashboard",
                "hot_reload": true,
                "requires_spiffe": false,
                "requires_oauth": false
            },
            "local.dashboard.security_coverage": {
                "schema": "security-coverage.v1.schema.json",
                "direction": "local_to_dashboard",
                "hot_reload": true,
                "requires_spiffe": false,
                "requires_oauth": false
            },
            "pollek.cloud.telemetry": {
                "schema": "telemetry-envelope.v1.schema.json",
                "direction": "local_to_cloud",
                "hot_reload": false,
                "requires_spiffe": true,
                "requires_oauth": true
            },
            "pollek.cloud.policy_bundle": {
                "schema": "bundle-envelope.v1.schema.json",
                "direction": "cloud_to_local",
                "hot_reload": true,
                "requires_spiffe": true,
                "requires_oauth": true
            }
        },
        "capabilities": [
            "contract.discovery.v1",
            "local.capability-snapshot.v2",
            "security.coverage.v1",
            "user-message.catalog.v1",
            "registered-agent.identity-binding.v1",
            "scan-session.v2",
            "bundle.signed-envelope.v1",
            "telemetry.batch.v1",
            "policy.opa-wasm.v1",
            "policy.wasm-host-call-policy.v1",
            "policy.opa-wasm-abi.v1",
            "policy.cedar.v1",
            "policy.openfga.v1",
            "policy.wasm-plugin.v1",
            "pdp.routing.v1",
            "pdp.cloud-sync.v1",
            "pdp.system-managed-runtimes.v1"
        ]
    }))
}
