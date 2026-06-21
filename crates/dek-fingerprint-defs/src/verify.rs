use crate::model::FingerprintDefinition;
use anyhow::{bail, Result};

pub fn verify_definition(
    raw_bytes: &[u8],
    sig_b64: &str,
    def: &FingerprintDefinition,
    engine_version: &semver::Version,
    keys: &dek_bundle_sync::keys::TrustedKeySet,
) -> Result<()> {
    let now_unix = chrono::Utc::now().timestamp();
    let sig_entry = dek_bundle_sync::keys::SignatureEntry {
        key_id: None,
        sig_b64: sig_b64.to_string(),
    };
    
    match keys.verify(now_unix, raw_bytes, &[sig_entry]) {
        dek_bundle_sync::keys::VerifyOutcome::Valid { .. } => {}
        _ => bail!("fingerprint definition signature invalid — refusing (fail-closed)"),
    }
    
    let min = semver::Version::parse(&def.min_engine_version)?;
    if engine_version < &min {
        bail!("definition requires engine >= {min}, have {engine_version}");
    }
    
    // integrity — catalog_hash
    // In a real implementation we would compute the hash of the active catalog
    // For now we just parse properly as placeholder.
    
    Ok(())
}
