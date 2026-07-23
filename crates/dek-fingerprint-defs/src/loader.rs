// SPDX-License-Identifier: Apache-2.0

use arc_swap::ArcSwap;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::path::PathBuf;
use std::sync::Arc;

use crate::model::*;

pub struct DefinitionStore {
    current: ArcSwap<FingerprintDefinition>,
    on_disk_path: PathBuf,
}

impl DefinitionStore {
    pub fn load(on_disk_path: PathBuf, pubkey: Option<&VerifyingKey>) -> Self {
        let mut def = crate::load_latest_baseline();
        if let Ok(raw) = std::fs::read(&on_disk_path) {
            // First try parsing as SignedDefinition
            if let Ok(signed) = serde_json::from_slice::<SignedDefinition>(&raw) {
                let is_valid = if let Some(pk) = pubkey {
                    if let Ok(payload_bytes) = serde_json::to_vec(&signed.payload) {
                        use base64::{engine::general_purpose::STANDARD, Engine as _};
                        if let Ok(sig_bytes) = STANDARD.decode(&signed.signature) {
                            if let Ok(sig) = Signature::from_slice(&sig_bytes) {
                                pk.verify(&payload_bytes, &sig).is_ok()
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    true // If no pubkey provided, assume valid (for local/testing)
                };

                if is_valid {
                    let disk = signed.payload;
                    if disk.definition_version > def.definition_version
                        && disk.schema_version == def.schema_version
                    {
                        def = disk;
                        tracing::info!(
                            version = def.definition_version,
                            "loaded signed on-disk definition"
                        );
                    } else {
                        tracing::warn!("on-disk signed definition rejected (version/schema)");
                    }
                } else {
                    tracing::error!("on-disk definition signature verification failed!");
                }
            } else if let Ok(disk) = serde_json::from_slice::<FingerprintDefinition>(&raw) {
                // Fallback to raw FingerprintDefinition if allowed (e.g. pubkey is None)
                if pubkey.is_none() {
                    if disk.definition_version > def.definition_version
                        && disk.schema_version == def.schema_version
                    {
                        def = disk;
                        tracing::info!(
                            version = def.definition_version,
                            "loaded unsigned on-disk definition"
                        );
                    } else {
                        tracing::warn!("on-disk definition rejected (version/schema)");
                    }
                } else {
                    tracing::error!("unsigned on-disk definition found but pubkey is required");
                }
            }
        }
        Self {
            current: ArcSwap::from_pointee(def),
            on_disk_path,
        }
    }

    pub fn get(&self) -> Arc<FingerprintDefinition> {
        self.current.load_full()
    }

    pub fn apply_update(&self, incoming: FingerprintDefinition) -> anyhow::Result<u64> {
        let cur = self.current.load();
        anyhow::ensure!(
            incoming.schema_version == cur.schema_version,
            "schema mismatch"
        );
        anyhow::ensure!(
            incoming.definition_version > cur.definition_version,
            "stale version"
        );

        let merged = match incoming.kind {
            DefinitionKind::Full => incoming,
            DefinitionKind::Delta => merge_delta(&cur, &incoming)?,
        };

        // Persist to disk (atomic tmp+rename) BEFORE swapping the live value, so
        // a persistence failure never leaves the in-memory definition ahead of
        // what would be reloaded on restart.
        if let Ok(json) = serde_json::to_string_pretty(&merged) {
            if let Some(parent) = self.on_disk_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let tmp_path = self.on_disk_path.with_extension("tmp");
            std::fs::write(&tmp_path, json)?;
            std::fs::rename(tmp_path, &self.on_disk_path)?;
        }

        self.current.store(Arc::new(merged.clone()));

        tracing::info!(
            version = merged.definition_version,
            "definition updated (hot)"
        );
        Ok(merged.definition_version)
    }
}

fn merge_delta(
    base: &FingerprintDefinition,
    delta: &FingerprintDefinition,
) -> anyhow::Result<FingerprintDefinition> {
    let mut out = base.clone();
    out.definition_version = delta.definition_version;

    for s in &delta.signatures {
        match out.signatures.iter_mut().find(|x| x.id == s.id) {
            Some(existing) => *existing = s.clone(),
            None => out.signatures.push(s.clone()),
        }
    }
    for w in &delta.web_ai_signatures {
        match out
            .web_ai_signatures
            .iter_mut()
            .find(|x| x.stable_id() == w.stable_id() || x.domain == w.domain)
        {
            Some(e) => *e = w.clone(),
            None => out.web_ai_signatures.push(w.clone()),
        }
    }

    for browser in &delta.browser_processes {
        match out.browser_processes.iter_mut().find(|existing| {
            existing.engine == browser.engine
                && existing.process_names.iter().any(|existing_name| {
                    browser
                        .process_names
                        .iter()
                        .any(|incoming_name| existing_name.eq_ignore_ascii_case(incoming_name))
                })
        }) {
            Some(existing) => {
                for name in &browser.process_names {
                    if !existing
                        .process_names
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(name))
                    {
                        existing.process_names.push(name.clone());
                    }
                }
            }
            None => out.browser_processes.push(browser.clone()),
        }
    }

    if !delta.ai_process_hints.name_tokens.is_empty()
        || !delta.ai_process_hints.cmd_tokens.is_empty()
        || !delta.ai_process_hints.deny_tokens.is_empty()
        || delta.ai_process_hints.require_match
    {
        out.ai_process_hints = delta.ai_process_hints.clone();
    }

    // Process removed_ids if necessary
    out.signatures
        .retain(|s| !delta.removed_ids.contains(&s.id));
    out.web_ai_signatures
        .retain(|s| !delta.removed_ids.contains(&s.id) && !delta.removed_ids.contains(&s.domain));
    out.installed_app_signatures
        .retain(|s| !delta.removed_ids.contains(&s.id));
    out.browser_processes.retain(|b| {
        !b.process_names.iter().any(|name| {
            delta
                .removed_ids
                .iter()
                .any(|id| name.eq_ignore_ascii_case(id))
        })
    });

    // Recalculate catalog hash if needed
    // out.catalog_hash = compute_hash(&out);
    Ok(out)
}
