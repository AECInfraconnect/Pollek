use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ActivationMode {
    Full,
    Canary,
    DryRun,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct BundleArtifact {
    pub name: String,
    pub artifact_type: String, // e.g. "cedar", "openfga", "wasm", "routes"
    pub sha256: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct BundleManifest {
    pub schema_version: String,
    pub bundle_id: String,
    pub bundle_version: String,
    pub bundle_generation: u64, // Used for anti-rollback
    pub tenant_id: String,
    pub created_at: String,
    pub expires_at: String,
    pub activation_mode: ActivationMode,
    pub artifacts: Vec<BundleArtifact>,
}

impl BundleManifest {
    /// Check if the bundle is expired
    pub fn is_expired(&self, now: DateTime<Utc>) -> Result<bool, &'static str> {
        let expiry = DateTime::parse_from_rfc3339(&self.expires_at)
            .map_err(|_| "Invalid expiry date format")?;
        Ok(now > expiry.with_timezone(&Utc))
    }

    /// Check if the bundle generation is older than the current generation (anti-rollback)
    pub fn validate_anti_rollback(&self, current_generation: u64) -> Result<(), &'static str> {
        if self.bundle_generation < current_generation {
            return Err("Anti-rollback protection triggered: bundle generation is older than current");
        }
        Ok(())
    }

    /// Check if the artifact matches its expected SHA256 hash
    pub fn validate_artifact_hash(&self, name: &str, content: &[u8]) -> Result<(), &'static str> {
        let artifact = self
            .artifacts
            .iter()
            .find(|a| a.name == name)
            .ok_or("Artifact not found in manifest")?;

        let mut hasher = Sha256::new();
        hasher.update(content);
        let hash_bytes = hasher.finalize();
        let hash = hash_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();

        if hash != artifact.sha256 {
            return Err("Hash mismatch");
        }
        Ok(())
    }
}

/// Helper struct for Last Known Good (LKG) fallback state
#[derive(Debug, Clone)]
pub struct LkgState {
    pub current_manifest: Option<BundleManifest>,
    pub fallback_manifest: Option<BundleManifest>,
}

impl LkgState {
    pub fn new() -> Self {
        Self {
            current_manifest: None,
            fallback_manifest: None,
        }
    }

    /// Applies a new manifest. If successful, current becomes fallback.
    pub fn apply_new_manifest(&mut self, new_manifest: BundleManifest) {
        self.fallback_manifest = self.current_manifest.take();
        self.current_manifest = Some(new_manifest);
    }

    /// Rollback to the fallback manifest.
    pub fn rollback(&mut self) -> Result<&BundleManifest, &'static str> {
        if let Some(fallback) = self.fallback_manifest.take() {
            self.current_manifest = Some(fallback);
            Ok(self.current_manifest.as_ref().unwrap())
        } else {
            Err("No LKG manifest available to rollback to")
        }
    }
}
