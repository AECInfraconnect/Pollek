// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! keys.rs — trusted bundle-signing keys with rotation support.
//!
//! Phase 2: replaces the single pinned key with a SET of trusted keys so the
//! cloud can rotate signing keys without re-bootstrapping the device. A bundle
//! signature is accepted if it verifies against ANY currently-usable key
//! (active or next), enabling overlap windows during rotation.
//!
//! This type lives in dek-bundle-sync (the verifier) to avoid a circular dep
//! with dek-policy-syncer (which orchestrates distribution/rotation on top).

use anyhow::{Context, Result};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyStatus {
    /// Primary signing key.
    Active,
    /// Newly-introduced key, trusted for verify during the overlap window.
    Next,
    /// No longer trusted.
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedKey {
    pub key_id: String,
    /// base64 ed25519 public key (32 bytes).
    pub public_b64: String,
    pub status: KeyStatus,
    #[serde(default)]
    pub not_before_unix: i64,
    /// 0 = no upper bound.
    #[serde(default)]
    pub not_after_unix: i64,
}

impl TrustedKey {
    pub fn verifying_key(&self) -> Result<VerifyingKey> {
        use base64::Engine;
        let bytes = base64::prelude::BASE64_STANDARD
            .decode(&self.public_b64)
            .context("pubkey not base64")?;
        let arr: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| anyhow::anyhow!("pubkey must be 32 bytes, got {}", bytes.len()))?;
        VerifyingKey::from_bytes(&arr).context("invalid ed25519 pubkey")
    }
    /// Usable for VERIFY (active or next, within validity window).
    pub fn usable(&self, now_unix: i64) -> bool {
        self.status != KeyStatus::Revoked
            && now_unix >= self.not_before_unix
            && (self.not_after_unix == 0 || now_unix <= self.not_after_unix)
    }
}

/// One entry of a TUF-style `signatures` array.
#[derive(Debug, Clone)]
pub struct SignatureEntry {
    pub key_id: Option<String>,
    pub sig_b64: String,
}

/// Result of verifying a signed payload against the set.
#[derive(Debug, Clone, PartialEq)]
pub enum VerifyOutcome {
    /// Verified by this key.
    Valid { key_id: String },
    /// No signature verified against any usable key (possible unsigned/forged push).
    NoValidSignature,
    /// The set has no usable keys at all (misconfig / all revoked).
    NoUsableKeys,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TrustedKeySet {
    pub keys: Vec<TrustedKey>,
}

impl TrustedKeySet {
    /// Bootstrap from a single pinned key (back-compat with enrollment).
    pub fn from_single_pinned(public_b64: &str) -> Self {
        Self {
            keys: vec![TrustedKey {
                key_id: "bootstrap".to_string(),
                public_b64: public_b64.to_string(),
                status: KeyStatus::Active,
                not_before_unix: 0,
                not_after_unix: 0,
            }],
        }
    }

    pub fn usable_keys(&self, now_unix: i64) -> impl Iterator<Item = &TrustedKey> {
        self.keys.iter().filter(move |k| k.usable(now_unix))
    }

    /// Verify `signed` against the set. If a signature carries a `key_id`, that
    /// key is tried first; otherwise every usable key is tried. Length-checked,
    /// fail-closed: malformed sig is skipped, never panics.
    pub fn verify(
        &self,
        now_unix: i64,
        signed: &[u8],
        signatures: &[SignatureEntry],
    ) -> VerifyOutcome {
        use base64::Engine;
        if self.usable_keys(now_unix).next().is_none() {
            return VerifyOutcome::NoUsableKeys;
        }
        for entry in signatures {
            let Ok(sig_bytes) = base64::prelude::BASE64_STANDARD.decode(&entry.sig_b64) else {
                continue;
            };
            let Ok(sig_arr): std::result::Result<[u8; 64], _> = sig_bytes.as_slice().try_into()
            else {
                continue; // wrong length -> skip (fail-closed)
            };
            let signature = Signature::from_bytes(&sig_arr);

            // Candidate keys: keyid match first, else all usable.
            let candidates: Vec<&TrustedKey> = match &entry.key_id {
                Some(kid) => self
                    .usable_keys(now_unix)
                    .filter(|k| &k.key_id == kid)
                    .collect(),
                None => self.usable_keys(now_unix).collect(),
            };
            for key in candidates {
                let Ok(vk) = key.verifying_key() else {
                    tracing::error!("Failed to parse VerifyingKey for kid {:?}", key.key_id);
                    continue;
                };
                tracing::info!("Verifying with kid {:?}, strict verify...", key.key_id);
                if let Err(e) = vk.verify_strict(signed, &signature) {
                    tracing::error!(
                        "verify_strict failed: {:?}. Key_b64: {}, sig_b64: {}",
                        e,
                        key.public_b64,
                        entry.sig_b64
                    );
                } else {
                    return VerifyOutcome::Valid {
                        key_id: key.key_id.clone(),
                    };
                }
            }
        }
        VerifyOutcome::NoValidSignature
    }

    /// Merge an incoming key list (from a verified /v1/keys payload). New keys
    /// are added; existing keys updated by status; returns what changed (for audit).
    pub fn merge_rotation(&mut self, incoming: Vec<TrustedKey>) -> RotationDelta {
        let mut delta = RotationDelta::default();
        for inc in incoming {
            match self.keys.iter_mut().find(|k| k.key_id == inc.key_id) {
                Some(existing) => {
                    if existing.status != inc.status {
                        if inc.status == KeyStatus::Revoked {
                            delta.revoked.push(inc.key_id.clone());
                        } else if inc.status == KeyStatus::Active
                            && existing.status == KeyStatus::Next
                        {
                            delta.promoted.push(inc.key_id.clone());
                        }
                        existing.status = inc.status.clone();
                    }
                    existing.not_before_unix = inc.not_before_unix;
                    existing.not_after_unix = inc.not_after_unix;
                    existing.public_b64 = inc.public_b64;
                }
                None => {
                    delta.added.push(inc.key_id.clone());
                    self.keys.push(inc);
                }
            }
        }
        delta
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RotationDelta {
    pub added: Vec<String>,
    pub promoted: Vec<String>,
    pub revoked: Vec<String>,
}
impl RotationDelta {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.promoted.is_empty() && self.revoked.is_empty()
    }
}

/// Parse a TUF-style `signatures` array (Value) into SignatureEntry list.
pub fn parse_signatures(signatures: &serde_json::Value) -> Vec<SignatureEntry> {
    signatures
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    let sig_b64 = s.get("sig").and_then(|v| v.as_str())?.to_string();
                    let key_id = s
                        .get("keyid")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    Some(SignatureEntry { key_id, sig_b64 })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};

    fn keypair(seed: u8) -> (SigningKey, String) {
        let sk = SigningKey::from_bytes(&[seed; 32]);
        let pk_b64 = base64::prelude::BASE64_STANDARD.encode(sk.verifying_key().to_bytes());
        (sk, pk_b64)
    }
    fn sign_entry(sk: &SigningKey, kid: &str, msg: &[u8]) -> SignatureEntry {
        let sig = sk.sign(msg);
        SignatureEntry {
            key_id: Some(kid.into()),
            sig_b64: base64::prelude::BASE64_STANDARD.encode(sig.to_bytes()),
        }
    }

    #[test]
    fn verifies_with_active_key() {
        let (sk, pk) = keypair(1);
        let set = TrustedKeySet {
            keys: vec![TrustedKey {
                key_id: "k1".into(),
                public_b64: pk,
                status: KeyStatus::Active,
                not_before_unix: 0,
                not_after_unix: 0,
            }],
        };
        let msg = b"signed-bytes";
        let out = set.verify(100, msg, &[sign_entry(&sk, "k1", msg)]);
        assert_eq!(
            out,
            VerifyOutcome::Valid {
                key_id: "k1".into()
            }
        );
    }

    #[test]
    fn overlap_window_old_and_next_both_verify() {
        let (sk_old, pk_old) = keypair(1);
        let (sk_new, pk_new) = keypair(2);
        let set = TrustedKeySet {
            keys: vec![
                TrustedKey {
                    key_id: "old".into(),
                    public_b64: pk_old,
                    status: KeyStatus::Active,
                    not_before_unix: 0,
                    not_after_unix: 0,
                },
                TrustedKey {
                    key_id: "new".into(),
                    public_b64: pk_new,
                    status: KeyStatus::Next,
                    not_before_unix: 0,
                    not_after_unix: 0,
                },
            ],
        };
        let msg = b"bundle";
        assert!(matches!(
            set.verify(1, msg, &[sign_entry(&sk_old, "old", msg)]),
            VerifyOutcome::Valid { .. }
        ));
        assert!(matches!(
            set.verify(1, msg, &[sign_entry(&sk_new, "new", msg)]),
            VerifyOutcome::Valid { .. }
        ));
    }

    #[test]
    fn revoked_key_rejected_after_rotation() {
        let (sk_old, pk_old) = keypair(1);
        let mut set = TrustedKeySet {
            keys: vec![TrustedKey {
                key_id: "old".into(),
                public_b64: pk_old,
                status: KeyStatus::Active,
                not_before_unix: 0,
                not_after_unix: 0,
            }],
        };
        let msg = b"bundle";
        // revoke "old"
        let delta = set.merge_rotation(vec![TrustedKey {
            key_id: "old".into(),
            public_b64: set.keys[0].public_b64.clone(),
            status: KeyStatus::Revoked,
            not_before_unix: 0,
            not_after_unix: 0,
        }]);
        assert_eq!(delta.revoked, vec!["old".to_string()]);
        assert_eq!(
            set.verify(1, msg, &[sign_entry(&sk_old, "old", msg)]),
            VerifyOutcome::NoUsableKeys
        );
    }

    #[test]
    fn forged_signature_rejected() {
        let (_sk_real, pk_real) = keypair(1);
        let (sk_forged, _pk) = keypair(9);
        let set = TrustedKeySet {
            keys: vec![TrustedKey {
                key_id: "k1".into(),
                public_b64: pk_real,
                status: KeyStatus::Active,
                not_before_unix: 0,
                not_after_unix: 0,
            }],
        };
        let msg = b"bundle";
        // forged sig (signed by a key not in the set), claims keyid k1
        let out = set.verify(1, msg, &[sign_entry(&sk_forged, "k1", msg)]);
        assert_eq!(out, VerifyOutcome::NoValidSignature);
    }
}
