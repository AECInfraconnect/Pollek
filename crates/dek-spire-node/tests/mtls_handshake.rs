// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect
//
// A REAL mutual-TLS handshake, on one box: issue a CA + a server cert + a client
// X.509-SVID with rcgen, stand up a client-auth-REQUIRED rustls server, and prove
// that the reqwest client built by `dek_spire_node::client_from_identity_dir`
// (from the SVID triple) completes the handshake and is authenticated — while a
// client that presents NO certificate is rejected at the TLS layer.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rcgen::{BasicConstraints, CertificateParams, Ia5String, IsCa, KeyPair, SanType};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio_rustls::rustls::server::WebPkiClientVerifier;
use tokio_rustls::rustls::{RootCertStore, ServerConfig};
use tokio_rustls::TlsAcceptor;

struct Pki {
    trust_bundle_pem: String,
    server_cert_der: CertificateDer<'static>,
    server_key_der: PrivateKeyDer<'static>,
    ca_der: CertificateDer<'static>,
    svid_cert_pem: String,
    svid_key_pem: String,
    spiffe_id: String,
}

fn issue_pki() -> Pki {
    // Root CA.
    let mut ca_params = CertificateParams::new(vec!["Pollek Test Root CA".into()]).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_kp = KeyPair::generate().unwrap();
    let ca_cert = ca_params.self_signed(&ca_kp).unwrap();

    // Server leaf for 127.0.0.1 / localhost, signed by the CA.
    let server_params =
        CertificateParams::new(vec!["localhost".into(), "127.0.0.1".into()]).unwrap();
    let server_kp = KeyPair::generate().unwrap();
    let server_cert = server_params
        .signed_by(&server_kp, &ca_cert, &ca_kp)
        .unwrap();

    // Client X.509-SVID with a SPIFFE URI SAN, signed by the CA.
    let spiffe_id = "spiffe://pollek.io/tenant/local/device/demo-01".to_string();
    let mut client_params = CertificateParams::new(vec![]).unwrap();
    client_params.subject_alt_names.push(SanType::URI(
        Ia5String::try_from(spiffe_id.clone()).unwrap(),
    ));
    let client_kp = KeyPair::generate().unwrap();
    let client_cert = client_params
        .signed_by(&client_kp, &ca_cert, &ca_kp)
        .unwrap();

    Pki {
        trust_bundle_pem: ca_cert.pem(),
        server_cert_der: server_cert.der().clone(),
        server_key_der: PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(server_kp.serialize_der())),
        ca_der: ca_cert.der().clone(),
        svid_cert_pem: client_cert.pem(),
        svid_key_pem: client_kp.serialize_pem(),
        spiffe_id,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn svid_client_completes_mutual_tls_and_certless_is_rejected() {
    // reqwest's rustls-tls uses the ring provider; match it for the server config.
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();

    let pki = issue_pki();

    // Persist the SVID triple exactly as enrollment/renewal would.
    let dir = std::env::temp_dir().join(format!("dek-mtls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("svid.pem"), &pki.svid_cert_pem).unwrap();
    std::fs::write(dir.join("svid-key.pem"), &pki.svid_key_pem).unwrap();
    std::fs::write(dir.join("trust-bundle.pem"), &pki.trust_bundle_pem).unwrap();
    assert!(dek_spire_node::identity_present(&dir));

    // Client-auth-REQUIRED server config.
    let mut roots = RootCertStore::empty();
    roots.add(pki.ca_der.clone()).unwrap();
    let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
        .build()
        .unwrap();
    let server_cfg = ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(
            vec![pki.server_cert_der.clone()],
            pki.server_key_der.clone_key(),
        )
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(server_cfg));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Server accepts up to two connections: the mTLS client (should succeed and
    // present the SVID) and the certless client (handshake should error).
    let (tx, rx) = tokio::sync::oneshot::channel::<Option<String>>();
    tokio::spawn(async move {
        let mut sent = false;
        let mut tx = Some(tx);
        for _ in 0..2 {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            match acceptor.accept(stream).await {
                Ok(mut tls) => {
                    // Capture the authenticated peer identity (the SVID).
                    let peer_spiffe = tls
                        .get_ref()
                        .1
                        .peer_certificates()
                        .and_then(|c| c.first().cloned())
                        .map(|c| san_uri(c.as_ref()).unwrap_or_default());
                    let mut buf = [0u8; 1024];
                    let _ = tls.read(&mut buf).await;
                    let _ = tls
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                        )
                        .await;
                    let _ = tls.flush().await;
                    if !sent {
                        sent = true;
                        if let Some(tx) = tx.take() {
                            let _ = tx.send(peer_spiffe);
                        }
                    }
                }
                Err(_) => {
                    // Certless client rejected — expected; keep serving.
                }
            }
        }
    });

    let url = format!("https://127.0.0.1:{}/", addr.port());

    // 1) mTLS client built from the SVID triple → handshake succeeds, 200 ok.
    let mtls = dek_spire_node::client_from_identity_dir(&dir).unwrap();
    let resp = mtls
        .get(&url)
        .send()
        .await
        .expect("mTLS request should succeed");
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    assert_eq!(resp.text().await.unwrap(), "ok");

    // The server authenticated us by our SVID's SPIFFE ID.
    let peer = rx.await.unwrap();
    assert_eq!(peer.as_deref(), Some(pki.spiffe_id.as_str()));

    // 2) A client that trusts the CA but presents NO client cert → rejected.
    let root = reqwest::Certificate::from_pem(pki.trust_bundle_pem.as_bytes()).unwrap();
    let certless = reqwest::Client::builder()
        .add_root_certificate(root)
        .tls_built_in_root_certs(false)
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    let res = certless.get(&url).send().await;
    assert!(
        res.is_err(),
        "certless client must be rejected by the client-auth gate"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Extract the first URI SAN (the SPIFFE ID) from a DER cert.
fn san_uri(der: &[u8]) -> Option<String> {
    use x509_parser::prelude::*;
    let (_, cert) = X509Certificate::from_der(der).ok()?;
    for ext in cert.extensions() {
        if let ParsedExtension::SubjectAlternativeName(san) = ext.parsed_extension() {
            for name in &san.general_names {
                if let GeneralName::URI(uri) = name {
                    return Some(uri.to_string());
                }
            }
        }
    }
    None
}
