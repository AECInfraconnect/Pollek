use axum::{routing::{get, post}, Json, Router, extract::State};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};
use crate::{state::AppState, BUNDLE_SEED, bundle_pubkey_b64};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/keys", get(get_keys))
        .route("/admin/rotate-key", post(rotate_key))
}

async fn get_keys(State(st): State<AppState>) -> Json<Value> {
    // signed payload = รายการ trusted keys ปัจจุบัน (ดึงจาก state ที่ scenario ตั้งไว้)
    let keys = st.trusted_keys.lock().unwrap().clone(); // Vec<serde_json::Value> ของ TrustedKey
    let signed = json!({ "keys": keys, "version": 1 });
    let signed_bytes = serde_json::to_vec(&signed).unwrap();

    // sign ด้วย CURRENT active key (key เดิมที่ DEK trust อยู่แล้ว) = chain of trust
    let sk = SigningKey::from_bytes(&BUNDLE_SEED);
    let sig = sk.sign(&signed_bytes);
    use base64::Engine;
    Json(json!({
        "signed": signed,
        "signatures": [{ "keyid": "bootstrap", "sig": base64::prelude::BASE64_STANDARD.encode(sig.to_bytes()) }]
    }))
}

async fn rotate_key(State(st): State<AppState>) -> Json<Value> {
    // สร้าง next key, sign /v1/keys ด้วย bootstrap (current), ประกาศ next + revoke เก่าใน step ถัดไป
    let next_seed = [9u8; 32];
    let next_sk = SigningKey::from_bytes(&next_seed);
    use base64::Engine;
    let next_pub = base64::prelude::BASE64_STANDARD.encode(next_sk.verifying_key().to_bytes());
    let mut keys = st.trusted_keys.lock().unwrap();
    keys.push(json!({ "key_id":"key-prod-2", "public_b64": next_pub, "status":"next", "not_before_unix":0, "not_after_unix":0 }));
    // (step ต่อมาใน test: เปลี่ยน status bootstrap->revoked, key-prod-2->active, แล้ว sign bundle ด้วย next_seed)
    Json(json!({ "rotated": true }))
}
