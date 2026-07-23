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
        "contract_version": crate::contract_api::CONTRACT_VERSION,
        "compatible_cloud_contracts": [">=2026.06.29 <2026.09.00"],
        "dek_version": dek_bundle_format::dek_version(),
        "minimum_dek_version": crate::contract_api::MIN_SUPPORTED_DEK_VERSION,
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
            "tool.usage": "tool-usage.v1.schema.json",
            "plugin.manifest": "pollek-plugin.v1.schema.json"
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
                "requires_oauth": true,
                "requires_mtls": true,
                "tenant_scoped": true,
                "paths": [
                    "/v1/tenants/{tenant_id}/bundles/hot-reload",
                    "/v1/tenants/{tenant_id}/policy-bundles/hot-reload",
                    "/v1/policy-bundles/{bundle_id}/hot-reload"
                ],
                "controls": [
                    "signed-control-envelope",
                    "nonce",
                    "expiry",
                    "payload_hash",
                    "allowlisted_paths",
                    "replay_record",
                    "audit_event"
                ]
            },
            "pollek.cloud.connection_update": {
                "schema": "pollek.cloud.connection-update.v1",
                "direction": "cloud_to_local",
                "hot_reload": true,
                "requires_spiffe": true,
                "requires_oauth": true,
                "requires_mtls": true,
                "tenant_scoped": true,
                "paths": [
                    "/v1/tenants/{tenant_id}/pdp/cloud",
                    "/v1/tenants/{tenant_id}/pdp/cloud/probe"
                ],
                "controls": [
                    "signed-control-envelope",
                    "payload_hash",
                    "allowlisted_paths",
                    "replay_record"
                ]
            },
            "pollek.cloud.secure_control_channel": {
                "direction": "bidirectional",
                "hot_reload": true,
                "requires_spiffe": true,
                "requires_oauth": true,
                "requires_mtls": true,
                "tenant_scoped": true,
                "paths": [
                    "/v1/tenants/{tenant_id}/pdp/cloud",
                    "/v1/tenants/{tenant_id}/bundles/hot-reload",
                    "/v1/tenants/{tenant_id}/policy-bundles/hot-reload",
                    "/v1/policy-bundles/{bundle_id}/hot-reload"
                ],
                "controls": [
                    "signed-control-envelope",
                    "nonce",
                    "expiry",
                    "payload_hash",
                    "allowlisted_paths",
                    "replay_record",
                    "audit_event",
                    "secret_redaction"
                ],
                "purpose": "Accept least-privilege Pollek Cloud control dispatches and apply local configuration or hot-reload updates without treating unsupported paths as success."
            },
            "local.discovery.grouped_surfaces": {
                "direction": "local_to_dashboard",
                "hot_reload": true,
                "requires_spiffe": false,
                "requires_oauth": false,
                "tenant_scoped": true,
                "paths": [
                    "/v1/tenants/{tenant_id}/discovery/candidates",
                    "/v1/tenants/{tenant_id}/discovery/entities"
                ],
                "controls": [
                    "canonical_service_id",
                    "surface_group_id",
                    "authority_boundary",
                    "entity_role",
                    "duplicate_policy",
                    "control_parent_id",
                    "related_surfaces"
                ],
                "purpose": "Expose typed discovery identity and grouping semantics so dashboard and cloud consumers do not infer identity from labels."
            },
            "local.discovery.interactive_enrichment": {
                "direction": "local_to_dashboard",
                "hot_reload": true,
                "requires_spiffe": false,
                "requires_oauth": false,
                "tenant_scoped": true,
                "paths": [
                    "/v1/tenants/{tenant_id}/discovery/candidates/{candidate_id}/enrichment/start",
                    "/v1/tenants/{tenant_id}/discovery/enrichment/{session_id}",
                    "/v1/tenants/{tenant_id}/discovery/enrichment/{session_id}/approve",
                    "/v1/tenants/{tenant_id}/discovery/enrichment/{session_id}/submit"
                ],
                "controls": [
                    "user_consent_required",
                    "safe_public_metadata_source_plan",
                    "no_package_install",
                    "no_code_execution",
                    "no_prompt_or_secret_capture",
                    "local_learned_profile"
                ],
                "purpose": "Let users improve unknown discovery definitions through a consent-first local enrichment workflow."
            }
        },
        "capabilities": [
            "contract.discovery.v1",
            "local.capability-snapshot.v2",
            "security.coverage.v1",
            "user-message.catalog.v1",
            "registered-agent.identity-binding.v1",
            "scan-session.v2",
            "discovery.grouped-surfaces.v1",
            "discovery.interactive-enrichment.v1",
            "bundle.signed-envelope.v1",
            "telemetry.batch.v1",
            "policy.opa-wasm.v1",
            "policy.wasm-host-call-policy.v1",
            "policy.opa-wasm-abi.v1",
            "policy.cedar.v1",
            "policy.openfga.v1",
            "policy.wasm-plugin.v1",
            "plugin.manifest.v1",
            "plugin.marketplace.v1",
            "pdp.routing.v1",
            "pdp.cloud-sync.v1",
            "pdp.system-managed-runtimes.v1",
            "pdp.cloud-config-dispatch.v1",
            "pdp.cloud-hot-reload-apply.v1",
            "secure-control.signed-envelope.v1"
        ]
    }))
}
