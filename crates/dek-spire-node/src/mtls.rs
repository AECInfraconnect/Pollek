// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! mtls.rs — build the DEK↔Cloud transport client that presents the device's
//! **X.509-SVID** as its client certificate over mutual TLS (roadmap Phase B).
//!
//! The SVID triple is written by enrollment / renewal into the identity dir:
//! `svid.pem` (leaf cert), `svid-key.pem` (private key), `trust-bundle.pem`
//! (the SPIFFE trust bundle we pin the server to). When all three are present,
//! the transport is mutual-TLS authenticated; Cloud can cryptographically tell
//! *which DEK* connected. Until an SVID is provisioned the caller stays on the
//! bearer transport (dev / pre-enrollment) — this module never fabricates one.

use anyhow::{Context, Result};
use dek_config::MtlsConfig;
use std::path::Path;

/// Leaf SVID certificate (PEM).
pub const SVID_CERT: &str = "svid.pem";
/// SVID private key (PEM).
pub const SVID_KEY: &str = "svid-key.pem";
/// SPIFFE trust bundle / root CA (PEM).
pub const TRUST_BUNDLE: &str = "trust-bundle.pem";

/// True only when the full SVID triple is present — i.e. mTLS is actually
/// possible. Used to decide transport mode without ever guessing.
pub fn identity_present(dir: &Path) -> bool {
    dir.join(SVID_CERT).exists() && dir.join(SVID_KEY).exists() && dir.join(TRUST_BUNDLE).exists()
}

/// Build an `MtlsConfig` pointing at the SVID triple in `dir`.
pub fn mtls_config(dir: &Path) -> MtlsConfig {
    MtlsConfig {
        client_cert_path: dir.join(SVID_CERT).to_string_lossy().into_owned(),
        client_key_path: dir.join(SVID_KEY).to_string_lossy().into_owned(),
        root_ca_path: dir.join(TRUST_BUNDLE).to_string_lossy().into_owned(),
    }
}

/// Build a reqwest client that presents the device SVID as its client
/// certificate over mutual TLS, pinned to the SPIFFE trust bundle. Fails closed
/// if any of the SVID triple is missing — the caller decides the fallback,
/// this never silently downgrades.
pub fn client_from_identity_dir(dir: &Path) -> Result<reqwest::Client> {
    if !identity_present(dir) {
        anyhow::bail!(
            "SVID material incomplete in {} — cannot build mTLS client",
            dir.display()
        );
    }
    mtls_config(dir)
        .build_client(None)
        .context("build mTLS client from SVID")
}
