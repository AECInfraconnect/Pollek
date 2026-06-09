//! keys.rs — trusted signing keys (rotation) + contract path (R3).
//!
//! DEK fetches keys at the contract path:
//!   GET /v1/tenants/{tenant_id}/devices/{device_id}/trusted-keys
//! The legacy `/v1/keys` is kept as an alias. Both return the SAME signed
//! envelope (signed by the current active key = chain of trust).

use axum::{extract::{Path, State}, routing::{get, post}, Json, Router};
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        // contract path (R2.2 expects this)
        .route("/v1/tenants/:tenant_id/devices/:device_id/trusted-keys", get(get_trusted_keys))
        // legacy alias
        .route("/v1/keys", get(get_keys))
        .route("/admin/rotate-key", post(rotate_key))
}

/// Build the signed trusted-keys envelope from current state. Signed by the
/// CURRENT active key so the DEK can verify chain-of-trust before merging.
fn signed_keys_envelope(st: &AppState) -> Value {
    let keys = st.trusted_keys.lock().unwrap().clone();
    let signed = json!({ "keys": keys, "version": 1 });
    let signed_bytes = serde_json::to_vec(&signed).unwrap_or_default();

    // sign with the current active seed (admin rotate may switch this).
    // active_seed is stored as Vec<u8>; coerce to [u8;32] (fallback to BUNDLE_SEED on bad len).
    let seed_vec = st.active_seed.lock().unwrap().clone();
    let seed: [u8; 32] = seed_vec.as_slice().try_into().unwrap_or(crate::BUNDLE_SEED);
    let sk = SigningKey::from_bytes(&seed);
    let sig = sk.sign(&signed_bytes);

    let active_kid = st
        .trusted_keys
        .lock()
        .unwrap()
        .iter()
        .find(|k| k.get("status").and_then(|s| s.as_str()) == Some("active"))
        .and_then(|k| k.get("key_id").and_then(|s| s.as_str()).map(String::from))
        .unwrap_or_else(|| "bootstrap".to_string());

    json!({
        "signed": signed,
        "signatures": [{
            "keyid": active_kid,
            "sig": base64::prelude::BASE64_STANDARD.encode(sig.to_bytes())
        }]
    })
}

async fn get_trusted_keys(
    Path((_tenant_id, _device_id)): Path<(String, String)>,
    State(st): State<AppState>,
) -> Json<Value> {
    Json(signed_keys_envelope(&st))
}

async fn get_keys(State(st): State<AppState>) -> Json<Value> {
    Json(signed_keys_envelope(&st))
}

async fn rotate_key(State(st): State<AppState>) -> Json<Value> {
    const NEXT_SEED: [u8; 32] = [9u8; 32];
    let next_kid = "key-prod-2";
    let next_pub = base64::prelude::BASE64_STANDARD
        .encode(SigningKey::from_bytes(&NEXT_SEED).verifying_key().to_bytes());
    let mut keys = st.trusted_keys.lock().unwrap();
    if !keys.iter().any(|k| k.get("key_id").and_then(|s| s.as_str()) == Some(next_kid)) {
        keys.push(json!({
            "key_id": next_kid, "public_b64": next_pub,
            "status": "next", "not_before_unix": 0, "not_after_unix": 0
        }));
    }
    drop(keys);
    st.audit_push("admin", "rotate-key", "introduced next key (overlap)");
    Json(json!({ "rotated": true, "next_key_id": next_kid }))
}
