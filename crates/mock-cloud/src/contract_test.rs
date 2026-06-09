use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64::Engine;
use ed25519_dalek::{SigningKey, Verifier};
use serde_json::{json, Value};
use tower::util::ServiceExt;

#[tokio::test]
async fn trusted_keys_contract_path_serves_signed_envelope() {
    let state = crate::state::AppState {
        revision: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1)),
        rsa_public_key_pem: "".to_string(),
        pending: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        devices: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        telemetry_events: std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new())),
        rollout: std::sync::Arc::new(std::sync::Mutex::new(crate::state::RolloutConfig {
            latest_bundle: crate::state::PolicyBundle {
                version: "1.0".to_string(),
                cedar_src: "".to_string(),
                openfga_store: "".to_string(),
            },
            canary_bundle: None,
            canary_percentage: 0,
        })),
        audit_logs: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        pending_policies: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        trusted_keys: std::sync::Arc::new(std::sync::Mutex::new(vec![json!({
            "key_id": "bootstrap",
            "public_b64": crate::bundle_pubkey_b64(),
            "status": "active"
        })])),
        active_seed: std::sync::Arc::new(std::sync::Mutex::new(crate::BUNDLE_SEED.to_vec())),
        revocation_list: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        registry: std::sync::Arc::new(std::sync::Mutex::new(crate::state::RegistryState::default())),
        network_rules: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        chaos_config: std::sync::Arc::new(std::sync::Mutex::new(crate::state::ChaosConfig {
            outage_enabled: false,
            global_latency_ms: 0,
        })),
        approvals: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
    };

    let app = axum::Router::new()
        .merge(crate::keys::router())
        .with_state(state.clone());

    let req = Request::builder()
        .uri("/v1/tenants/t1/devices/d1/trusted-keys")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: Value = serde_json::from_slice(&body_bytes).unwrap();

    let signatures = body.get("signatures").unwrap().as_array().unwrap();
    assert_eq!(signatures.len(), 1);
    let sig_obj = &signatures[0];
    assert_eq!(sig_obj.get("keyid").unwrap().as_str().unwrap(), "bootstrap");

    let sig_b64 = sig_obj.get("sig").unwrap().as_str().unwrap();
    let sig_bytes = base64::prelude::BASE64_STANDARD.decode(sig_b64).unwrap();
    let sig = ed25519_dalek::Signature::from_slice(&sig_bytes).unwrap();

    let signed_obj = body.get("signed").unwrap();
    let signed_bytes = serde_json::to_vec(signed_obj).unwrap();

    let sk = SigningKey::from_bytes(&crate::BUNDLE_SEED);
    let vk = sk.verifying_key();
    assert!(vk.verify(&signed_bytes, &sig).is_ok(), "Signature must be valid");
}

#[tokio::test]
async fn telemetry_decision_logs_endpoint_accepts_and_redacts() {
    let state = crate::state::AppState {
        revision: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1)),
        rsa_public_key_pem: "".to_string(),
        pending: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        devices: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        telemetry_events: std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new())),
        rollout: std::sync::Arc::new(std::sync::Mutex::new(crate::state::RolloutConfig {
            latest_bundle: crate::state::PolicyBundle {
                version: "1.0".to_string(),
                cedar_src: "".to_string(),
                openfga_store: "".to_string(),
            },
            canary_bundle: None,
            canary_percentage: 0,
        })),
        audit_logs: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        pending_policies: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        trusted_keys: std::sync::Arc::new(std::sync::Mutex::new(vec![])),
        active_seed: std::sync::Arc::new(std::sync::Mutex::new(crate::BUNDLE_SEED.to_vec())),
        revocation_list: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        registry: std::sync::Arc::new(std::sync::Mutex::new(crate::state::RegistryState::default())),
        network_rules: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        chaos_config: std::sync::Arc::new(std::sync::Mutex::new(crate::state::ChaosConfig {
            outage_enabled: false,
            global_latency_ms: 0,
        })),
        approvals: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
    };

    let app = axum::Router::new()
        .merge(crate::telemetry::router())
        .with_state(state.clone());

    let payload_clean = json!({
        "events": [{
            "event_type": "decision",
            "schema_version": "1.0",
            "event_id": "evt1",
            "device_id": "device-1",
            "tenant_id": "t1",
            "timestamp": "2026-06-09T00:00:00Z",
            "trace_id": "tr1",
            "span_id": "span1",
            "spiffe_id": "spiffe://local/dev",
            "pep_type": "mcp-proxy",
            "agent_id": "agent1",
            "principal_id": "user1",
            "mcp_server_id": "mcp1",
            "tool_id": "tool1",
            "tool_name": "tool_name",
            "action": "read",
            "resource_id": "res1",
            "resource_uri": "uri1",
            "decision": "allow",
            "reason": "OK",
            "policy_ids": [],
            "bundle_id": "b1",
            "bundle_version": "v1",
            "latency_ms": 10,
            "cached": false
        }]
    });

    let req1 = Request::builder()
        .method("POST")
        .uri("/v1/telemetry/decision-logs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload_clean).unwrap()))
        .unwrap();

    let res1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(res1.status(), StatusCode::OK);

    let payload_dirty = json!({
        "events": [{
            "event_type": "decision",
            "schema_version": "1.0",
            "event_id": "evt2",
            "device_id": "device-1",
            "tenant_id": "t1",
            "timestamp": "2026-06-09T00:00:00Z",
            "trace_id": "tr1",
            "span_id": "span1",
            "spiffe_id": "spiffe://local/dev",
            "pep_type": "mcp-proxy",
            "agent_id": "agent1",
            "principal_id": "user1",
            "mcp_server_id": "mcp1",
            "tool_id": "tool1",
            "tool_name": "tool_name",
            "action": "read",
            "resource_id": "res1",
            "resource_uri": "uri1",
            "decision": "deny",
            "reason": "Invalid bearer token",
            "policy_ids": [],
            "bundle_id": "b1",
            "bundle_version": "v1",
            "latency_ms": 10,
            "cached": false
        }]
    });

    let req2 = Request::builder()
        .method("POST")
        .uri("/v1/telemetry/decision-logs")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload_dirty).unwrap()))
        .unwrap();

    let res2 = app.clone().oneshot(req2).await.unwrap();
    assert_eq!(res2.status(), StatusCode::BAD_REQUEST);
}
