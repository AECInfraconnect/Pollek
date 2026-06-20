#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::duplicated_attributes
)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! Integration test: full enrollment flow against an in-process mock cloud.
//!
//! Place at: crates/dek-cli/tests/enrollment_e2e.rs
//! Runs:  cargo test -p dek-cli --test enrollment_e2e
//!
//! It spins up a minimal axum server implementing the 4 enrollment endpoints
//! (device_authorization / token / enroll / node-attest with REAL CSR signing
//! against a test CA), runs the real `dek_enroll::EnrollClient` device flow and
//! `dek_spire_node::attest_with_join_token`, then asserts a valid X.509-SVID
//! with the SPIFFE ID in the URI SAN is issued.
//!
//! dek-cli/Cargo.toml [dev-dependencies]:
//!   tokio = { workspace = true, features = ["full"] }
//!   axum = "0.7"
//!   rcgen = "0.11"
//!   serde_json = { workspace = true }
//!   x509-parser = "0.16"

#![allow(clippy::unwrap_used, clippy::expect_used)]
use axum::{
    extract::{Form, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Mock {
    addr: String,
    ca_cert_pem: String,
    ca_key_pem: String,
    pending: Arc<Mutex<HashMap<String, u32>>>,
}

#[tokio::test]
async fn enroll_then_attest_yields_x509_svid() {
    // --- test CA (rcgen 0.11) ---
    let (ca_cert_pem, ca_key_pem) = make_test_ca();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{addr}");

    let mock = Mock {
        addr: addr.to_string(),
        ca_cert_pem: ca_cert_pem.clone(),
        ca_key_pem,
        pending: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/oauth/device_authorization", post(device_auth))
        .route("/oauth/token", post(token))
        .route("/enroll", post(enroll))
        .route("/spire/node/attest", post(attest_csr))
        .route("/health", get(|| async { "ok" }))
        .with_state(mock);

    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .await
            .unwrap();
    });

    // --- 1) device flow ---
    let client = dek_enroll::EnrollClient::new(&base, "pollen-dek", "dek.enroll", None);
    let enrollment = client
        .run(|p| {
            // sanity: server returned a user code + verification uri
            assert!(!p.user_code.is_empty());
            assert!(p.verification_uri.starts_with("http"));
        })
        .await
        .expect("enrollment should succeed");

    assert!(!enrollment.join_token.is_empty(), "join_token present");
    assert_eq!(enrollment.tenant_id, "tenant-test");
    assert!(enrollment.spire_endpoint.contains("/spire"));
    assert!(!enrollment.pinned_bundle_public_key.is_empty());

    // --- 2) join-token attestation -> real X.509-SVID ---
    let svid = dek_spire_node::attest_with_join_token(
        &enrollment.spire_endpoint,
        &enrollment.join_token,
        &enrollment.device_id,
        &enrollment.trust_bundle_pem,
    )
    .await
    .expect("attestation should issue an SVID");

    // key + cert look like PEM
    assert!(svid.key_pem.contains("PRIVATE KEY"), "got a private key");
    assert!(svid.cert_pem.contains("BEGIN CERTIFICATE"), "got a cert");

    // cert parses as X.509 and carries the SPIFFE ID in a URI SAN
    let (_, pem) = x509_parser::pem::parse_x509_pem(svid.cert_pem.as_bytes()).unwrap();
    let cert = pem.parse_x509().unwrap();
    let spiffe_in_san = cert
        .subject_alternative_name()
        .ok()
        .flatten()
        .map(|san| {
            san.value
                .general_names
                .iter()
                .any(|gn| matches!(gn, x509_parser::extensions::GeneralName::URI(u) if u.starts_with("spiffe://")))
        })
        .unwrap_or(false);
    assert!(spiffe_in_san, "SVID must carry a spiffe:// URI SAN");
    assert!(
        svid.spiffe_id.starts_with("spiffe://"),
        "spiffe id returned"
    );
    assert!(
        svid.spiffe_id.contains(&enrollment.device_id),
        "svid bound to device"
    );
}

// ----------------------------- mock handlers -----------------------------

#[derive(serde::Deserialize)]
struct AnyForm {
    #[allow(dead_code)]
    grant_type: Option<String>,
    device_code: Option<String>,
}

async fn device_auth(State(m): State<Mock>, Form(_f): Form<AnyForm>) -> Json<Value> {
    let dc = "test-device-code".to_string();
    m.pending.lock().unwrap().insert(dc.clone(), 0);
    Json(json!({
        "device_code": dc,
        "user_code": "BCDF-GHJK",
        "verification_uri": format!("http://{}/device", m.addr),
        "expires_in": 60,
        "interval": 0            // poll fast in tests
    }))
}

async fn token(State(m): State<Mock>, Form(f): Form<AnyForm>) -> (StatusCode, Json<Value>) {
    let dc = f.device_code.unwrap_or_default();
    let mut map = m.pending.lock().unwrap();
    let c = map.entry(dc).or_insert(0);
    *c += 1;
    if *c < 2 {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "authorization_pending" })),
        )
    } else {
        (
            StatusCode::OK,
            Json(json!({ "access_token": "test-token", "token_type": "Bearer" })),
        )
    }
}

async fn enroll(State(m): State<Mock>, headers: HeaderMap) -> (StatusCode, Json<Value>) {
    assert!(
        headers.get("authorization").is_some(),
        "enroll must be authenticated"
    );
    (
        StatusCode::OK,
        Json(json!({
            "join_token": "test-join-token",
            "spire_endpoint": format!("http://{}/spire", m.addr),
            "trust_bundle_pem": m.ca_cert_pem,
            "pinned_bundle_public_key": "dGVzdC1wdWJrZXk=",
            "tenant_id": "tenant-test",
            "device_id": "device-test-1",
            "spiffe_id": "spiffe://pollen.test/tenant-test/device/device-test-1",
            "cloud_url": format!("http://{}", m.addr)
        })),
    )
}

#[derive(serde::Deserialize)]
struct Attest {
    device_id: String,
    csr_pem: String,
}

async fn attest_csr(State(m): State<Mock>, Json(req): Json<Attest>) -> (StatusCode, Json<Value>) {
    let spiffe_id = format!("spiffe://pollen.test/tenant-test/device/{}", req.device_id);
    let cert = sign_csr(&m.ca_cert_pem, &m.ca_key_pem, &req.csr_pem, &spiffe_id);
    (
        StatusCode::OK,
        Json(
            json!({ "svid_cert_pem": cert, "spiffe_id": spiffe_id, "trust_bundle_pem": m.ca_cert_pem }),
        ),
    )
}

// ------------------------------- test crypto -------------------------------

fn make_test_ca() -> (String, String) {
    use rcgen::{CertificateParams, IsCa, KeyPair, KeyUsagePurpose};
    let mut params = CertificateParams::new(vec!["Pollen Test Root CA".to_string()]).unwrap();
    params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    let key_pair = KeyPair::generate().unwrap();
    let ca = params.self_signed(&key_pair).unwrap();
    (ca.pem(), key_pair.serialize_pem())
}

fn sign_csr(ca_cert_pem: &str, ca_key_pem: &str, csr_pem: &str, spiffe_id: &str) -> String {
    use rcgen::{CertificateParams, CertificateSigningRequestParams, KeyPair, SanType};
    let ca_key = KeyPair::from_pem(ca_key_pem).unwrap();
    let ca_params = CertificateParams::from_ca_cert_pem(ca_cert_pem).unwrap();
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let mut csr = CertificateSigningRequestParams::from_pem(csr_pem).unwrap();
    csr.params
        .subject_alt_names
        .push(SanType::URI(spiffe_id.try_into().unwrap()));
    csr.params
        .signed_by(&csr.public_key, &ca, &ca_key)
        .unwrap()
        .pem()
}
