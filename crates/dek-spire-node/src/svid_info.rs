//! X.509-SVID introspection — read the workload identity out of an issued
//! SVID certificate: its SPIFFE ID (the `spiffe://` URI SAN), validity window,
//! and how long until it expires. Used to surface the DEK's device/workload
//! identity in the dashboard and to decide when to renew.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use x509_parser::prelude::*;

/// A parsed view of an X.509-SVID.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SvidInfo {
    /// The `spiffe://…` id from the certificate's URI SAN, when present.
    pub spiffe_id: Option<String>,
    /// Certificate serial number (hex).
    pub serial: String,
    /// Subject distinguished name.
    pub subject: String,
    /// Issuer distinguished name.
    pub issuer: String,
    /// `notBefore` / `notAfter` as unix seconds.
    pub not_before_unix: i64,
    pub not_after_unix: i64,
    /// Seconds until expiry relative to `now_unix` (negative if already expired).
    pub seconds_until_expiry: i64,
    pub expired: bool,
}

/// Parse a PEM-encoded X.509-SVID and describe it relative to `now_unix`.
pub fn describe_svid(cert_pem: &str, now_unix: i64) -> Result<SvidInfo> {
    let (_, pem) =
        x509_parser::pem::parse_x509_pem(cert_pem.as_bytes()).context("SVID is not valid PEM")?;
    let cert = pem
        .parse_x509()
        .context("SVID PEM does not contain a valid X.509 certificate")?;

    let mut spiffe_id = None;
    if let Ok(Some(san)) = cert.subject_alternative_name() {
        for name in &san.value.general_names {
            if let GeneralName::URI(uri) = name {
                if uri.starts_with("spiffe://") {
                    spiffe_id = Some(uri.to_string());
                    break;
                }
            }
        }
    }

    let not_before_unix = cert.validity().not_before.timestamp();
    let not_after_unix = cert.validity().not_after.timestamp();

    Ok(SvidInfo {
        spiffe_id,
        serial: cert.raw_serial_as_string(),
        subject: cert.subject().to_string(),
        issuer: cert.issuer().to_string(),
        not_before_unix,
        not_after_unix,
        seconds_until_expiry: not_after_unix - now_unix,
        expired: now_unix >= not_after_unix,
    })
}
