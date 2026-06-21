// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! dek-spire-node — SPIRE node attestation.
//!
//! Two paths:
//!  1. `attest_and_fetch_svid` (existing) — used during bundle-sync once mTLS
//!     already works; returns the SPIFFE ID string. Unchanged.
//!  2. `attest_with_join_token` (NEW) — the FIRST-RUN path. The DEK has no
//!     identity yet, so it generates a keypair + CSR, presents the one-time
//!     join token from enrollment, and receives a signed **X.509-SVID** (a
//!     standard X.509 cert with the SPIFFE ID in the URI SAN). That key+cert
//!     becomes the DEK's mTLS identity.
//!
//! Transport note: real SPIRE uses the gRPC Node API. This HTTP shape matches
//! Pollen's `mock-cloud` `/node/attest` endpoint and is trivial to retarget to
//! gRPC later; the issued artifact (X.509-SVID) is identical either way.

use anyhow::{Context, Result};
use dek_config::MtlsConfig;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

pub mod jwt_svid;
pub mod trust_bundle;

pub use trust_bundle::{
    fetch_trust_bundle, install_root, spawn_trust_bundle_poller, TrustBundleResponse,
};

// ----------------------------- existing path -------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpireAttestRequest {
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpireAttestResponse {
    pub spiffe_id: String,
}

pub struct SpireNodeAgent {
    endpoint: String,
    mtls_client: reqwest::Client,
}

impl SpireNodeAgent {
    pub fn new(endpoint: &str, mtls: &MtlsConfig) -> Result<Self> {
        let mtls_client = mtls
            .build_client(None)
            .context("Failed to build mTLS client for SPIRE Node Agent")?;
        Ok(Self {
            endpoint: endpoint.to_string(),
            mtls_client,
        })
    }

    pub async fn attest_and_fetch_svid(&self, device_id: &str) -> Result<String> {
        let url = format!("{}/node/attest", self.endpoint);
        info!("Attesting node to SPIRE Server at {}", url);
        let req_body = SpireAttestRequest {
            device_id: device_id.to_string(),
        };
        let res = self.mtls_client.post(&url).json(&req_body).send().await?;
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            warn!("Failed to attest node. Status: {}, Body: {}", status, text);
            anyhow::bail!("SPIRE node attestation failed: {}", status);
        }
        let resp: SpireAttestResponse = res.json().await?;
        info!("Successfully attested. Received SVID: {}", resp.spiffe_id);
        Ok(resp.spiffe_id)
    }
}

// --------------------------- NEW: join-token path ---------------------------

/// A freshly issued X.509-SVID and the private key that backs it (both PEM).
#[derive(Debug, Clone)]
pub struct IssuedSvid {
    pub key_pem: String,
    pub cert_pem: String,
    pub spiffe_id: String,
    /// Trust bundle (root CA) echoed back, PEM.
    pub trust_bundle_pem: String,
}

#[derive(Debug, Serialize)]
struct JoinAttestRequest<'a> {
    join_token: &'a str,
    device_id: &'a str,
    /// PKCS#10 CSR carrying our public key. The server stamps the SPIFFE ID
    /// into the URI SAN when signing — client-proposed SANs are ignored.
    csr_pem: &'a str,
}

#[derive(Debug, Deserialize)]
struct JoinAttestResponse {
    /// Signed X.509-SVID (leaf cert), PEM.
    svid_cert_pem: String,
    spiffe_id: String,
    #[serde(default)]
    trust_bundle_pem: Option<String>,
}

/// Perform first-run node attestation with a one-time join token.
///
/// `trust_bundle_pem` is the root CA delivered during enrollment; we pin TLS to
/// it so even this bootstrap exchange resists MITM. Returns the issued SVID +
/// the private key we generated locally (the key never leaves the device).
pub async fn attest_with_join_token(
    spire_endpoint: &str,
    join_token: &str,
    device_id: &str,
    trust_bundle_pem: &str,
) -> Result<IssuedSvid> {
    // 1) Generate a fresh keypair + CSR locally. The private key stays here.
    let (key_pem, csr_pem) = generate_keypair_and_csr().context("generate keypair/CSR")?;

    // 2) TLS pinned to the enrollment-delivered trust bundle (anti-MITM on the
    //    bootstrap leg, before we have our own client identity).
    let root = reqwest::Certificate::from_pem(trust_bundle_pem.as_bytes())
        .context("parse trust bundle PEM")?;
    let client = reqwest::Client::builder()
        .add_root_certificate(root)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .context("build attest client")?;

    let url = format!("{}/node/attest", spire_endpoint.trim_end_matches('/'));
    info!("Performing join-token node attestation at {}", url);

    let res = client
        .post(&url)
        .json(&JoinAttestRequest {
            join_token,
            device_id,
            csr_pem: &csr_pem,
        })
        .send()
        .await
        .context("send join attestation")?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("join-token attestation failed: HTTP {status} — {body}");
    }

    let resp: JoinAttestResponse = res.json().await.context("parse attestation response")?;
    info!("Issued X.509-SVID for {}", resp.spiffe_id);

    Ok(IssuedSvid {
        key_pem,
        cert_pem: resp.svid_cert_pem,
        spiffe_id: resp.spiffe_id,
        trust_bundle_pem: resp
            .trust_bundle_pem
            .unwrap_or_else(|| trust_bundle_pem.to_string()),
    })
}

/// Renew the SVID using the CURRENT mTLS identity (the existing SVID
/// authenticates the request — join tokens are one-time and not reused).
/// Generates a fresh keypair + CSR and posts it to the renew endpoint over the
/// provided (already-authenticated) mTLS client.
pub async fn renew_svid(
    renew_url: &str,
    mtls_client: &reqwest::Client,
    device_id: &str,
) -> Result<IssuedSvid> {
    let (key_pem, csr_pem) = generate_keypair_and_csr().context("generate keypair/CSR")?;
    info!("Renewing SVID via {}", renew_url);

    let res = mtls_client
        .post(renew_url)
        .json(&JoinAttestRequest {
            join_token: "",
            device_id,
            csr_pem: &csr_pem,
        })
        .send()
        .await
        .context("send renew request")?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("SVID renewal failed: HTTP {status} — {body}");
    }

    let resp: JoinAttestResponse = res.json().await.context("parse renew response")?;
    Ok(IssuedSvid {
        key_pem,
        cert_pem: resp.svid_cert_pem,
        spiffe_id: resp.spiffe_id,
        trust_bundle_pem: resp.trust_bundle_pem.unwrap_or_default(),
    })
}

/// Generate a keypair and a PKCS#10 CSR (empty SANs — the SPIRE server assigns
/// the SPIFFE URI SAN at signing time).
///
/// Written for **rcgen 0.11** to match the repo (cert-gen uses 0.11). Add
/// `rcgen = "0.11"` to dek-spire-node/Cargo.toml. If you bump rcgen to 0.13,
/// use: `KeyPair::generate()` + `params.serialize_request(&kp)?.pem()`.
fn generate_keypair_and_csr() -> Result<(String, String)> {
    use rcgen::{CertificateParams, KeyPair};
    // Empty subject/SANs: the server stamps the SPIFFE URI SAN when signing.
    let params = CertificateParams::new(Vec::<String>::new()).context("build CSR params")?;
    let key_pair = KeyPair::generate().context("generate key pair")?;
    let csr_pem = params
        .serialize_request(&key_pair)
        .context("serialize CSR")?
        .pem()?;
    let key_pem = key_pair.serialize_pem();
    Ok((key_pem, csr_pem))
}
