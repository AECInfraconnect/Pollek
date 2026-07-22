use crate::model::FingerprintDefinition;
use anyhow::{bail, Result};
use sha2::Digest;

/// Canonical catalog hash: SHA-256 (hex) over the definition serialized with
/// its `catalog_hash` field cleared, so the hash covers the full catalog
/// content without being self-referential. This is the single source of truth
/// for how `catalog_hash` is produced and checked.
pub fn compute_catalog_hash(def: &FingerprintDefinition) -> Result<String> {
    let mut canonical = def.clone();
    canonical.catalog_hash = String::new();
    let bytes = serde_json::to_vec(&canonical)?;
    Ok(hex::encode(sha2::Sha256::digest(&bytes)))
}

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

    // Integrity: when the definition declares a catalog_hash, the content must
    // hash to exactly that value (fail-closed on mismatch). An empty hash is
    // allowed only for locally-authored deltas that never left this machine.
    if !def.catalog_hash.is_empty() {
        let computed = compute_catalog_hash(def)?;
        if computed != def.catalog_hash {
            bail!(
                "catalog_hash mismatch: definition claims {}, content hashes to {computed} — refusing (fail-closed)",
                def.catalog_hash
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_baseline_catalog_hash_is_valid() -> Result<()> {
        let baseline = crate::embedded_baseline();
        assert!(
            !baseline.catalog_hash.is_empty(),
            "embedded baseline must declare a catalog_hash"
        );
        let computed = compute_catalog_hash(&baseline)?;
        assert_eq!(
            computed, baseline.catalog_hash,
            "baseline.v4.json catalog_hash is stale — recompute it after editing the catalog"
        );
        Ok(())
    }

    #[test]
    fn tampered_catalog_changes_the_hash() -> Result<()> {
        let mut def = crate::embedded_baseline();
        let original = compute_catalog_hash(&def)?;
        def.installed_app_signatures.pop();
        let tampered = compute_catalog_hash(&def)?;
        assert_ne!(original, tampered);
        Ok(())
    }
}
