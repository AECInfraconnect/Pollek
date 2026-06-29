//! Loading, validating, and integrity-checking detection packs.

use crate::spec::{DetectType, RuleSpec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum LoadError {
    Io(String),
    Parse { file: String, error: String },
    Validation { rule: String, error: String },
    Integrity(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "io error: {e}"),
            LoadError::Parse { file, error } => write!(f, "parse error in {file}: {error}"),
            LoadError::Validation { rule, error } => {
                write!(f, "validation error in {rule}: {error}")
            }
            LoadError::Integrity(e) => write!(f, "integrity error: {e}"),
        }
    }
}

impl std::error::Error for LoadError {}

/// Pack manifest for the EDR-style local detection definition pack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackManifest {
    pub schema_version: String,
    pub pack_id: String,
    pub version: String,
    pub created: String,
    /// ABI/engine compatibility. Engines should refuse packs newer than they
    /// can safely interpret.
    pub min_engine: String,
    pub rules: Vec<RuleEntry>,
    pub signature: SignatureRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEntry {
    pub id: String,
    pub file: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureRef {
    /// Examples: "sha256-manifest-ci" for local-dev packs, "sigstore-cosign"
    /// for release packs.
    pub method: String,
    /// Path/URI to a detached signature bundle, or an explicit placeholder for
    /// local-dev packs that are protected by manifest hash checks only.
    pub bundle: String,
}

/// Enforce the coverage gate: every rule must map to OWASP Agentic and at
/// least one ATLAS or ATT&CK control, and each rule must keep an observe-only
/// fallback.
pub fn validate_rule(rule: &RuleSpec) -> Result<(), LoadError> {
    let fail = |msg: &str| LoadError::Validation {
        rule: rule.id.clone(),
        error: msg.to_string(),
    };

    if rule.maps.owasp_agentic.is_empty() {
        return Err(fail(
            "maps.owasp_agentic is empty (coverage proof required)",
        ));
    }
    if rule.maps.atlas.is_empty() && rule.maps.attack.is_empty() {
        return Err(fail("maps must include at least one ATLAS or ATT&CK id"));
    }
    if !rule.response.observe_only_fallback {
        return Err(fail("response.observe_only_fallback must be true"));
    }
    if rule.detect.steps.is_empty() {
        return Err(fail("detect.steps must not be empty"));
    }
    match rule.detect.detect_type {
        DetectType::Sequence | DetectType::Anomaly if rule.detect.window.is_none() => {
            return Err(fail("sequence/anomaly rules require a window"));
        }
        _ => {}
    }
    Ok(())
}

/// Parse and validate every `*.yaml` rule in a directory without a manifest
/// signature check. Use [`verify_and_load_pack`] for integrity enforcement.
pub fn load_pack_dir<P: AsRef<Path>>(dir: P) -> Result<Vec<RuleSpec>, LoadError> {
    let dir = dir.as_ref();
    let mut rules = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| LoadError::Io(e.to_string()))?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .map(|x| x == "yaml" || x == "yml")
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    for path in files {
        let text = fs::read_to_string(&path).map_err(|e| LoadError::Io(e.to_string()))?;
        let rule: RuleSpec = serde_yaml::from_str(&text).map_err(|e| LoadError::Parse {
            file: path.display().to_string(),
            error: e.to_string(),
        })?;
        validate_rule(&rule)?;
        rules.push(rule);
    }
    Ok(rules)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let digest = h.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

pub fn sha256_text_lf(bytes: &[u8]) -> Result<String, LoadError> {
    let text = std::str::from_utf8(bytes).map_err(|e| LoadError::Io(e.to_string()))?;
    let canonical = text.replace("\r\n", "\n");
    Ok(sha256_hex(canonical.as_bytes()))
}

/// Verify pack integrity by running the caller-provided manifest verifier and
/// comparing every rule's content hash with the manifest entry.
///
/// Production can wire `verify_signature` to cosign/sigstore. Local-dev and CI
/// tests can keep this hermetic while still enforcing per-rule SHA-256 hashes.
pub fn verify_and_load_pack<P, F>(dir: P, verify_signature: F) -> Result<Vec<RuleSpec>, LoadError>
where
    P: AsRef<Path>,
    F: FnOnce(&PackManifest, &Path) -> Result<(), String>,
{
    let dir = dir.as_ref();
    let manifest_path = dir.join("manifest.json");
    let manifest_text =
        fs::read_to_string(&manifest_path).map_err(|e| LoadError::Io(e.to_string()))?;
    let manifest: PackManifest =
        serde_json::from_str(&manifest_text).map_err(|e| LoadError::Parse {
            file: manifest_path.display().to_string(),
            error: e.to_string(),
        })?;

    verify_signature(&manifest, dir).map_err(LoadError::Integrity)?;

    let mut rules = Vec::with_capacity(manifest.rules.len());
    for entry in &manifest.rules {
        let path = dir.join(&entry.file);
        let bytes = fs::read(&path).map_err(|e| LoadError::Io(e.to_string()))?;
        let actual = sha256_text_lf(&bytes)?;
        if actual != entry.sha256 {
            return Err(LoadError::Integrity(format!(
                "sha256 mismatch for {} (manifest {}, actual {})",
                entry.file, entry.sha256, actual
            )));
        }
        let rule_text = std::str::from_utf8(&bytes)
            .map_err(|e| LoadError::Io(e.to_string()))?
            .replace("\r\n", "\n");
        let rule: RuleSpec = serde_yaml::from_str(&rule_text).map_err(|e| LoadError::Parse {
            file: path.display().to_string(),
            error: e.to_string(),
        })?;
        validate_rule(&rule)?;
        rules.push(rule);
    }
    Ok(rules)
}
