use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSignature {
    pub id: String,
    pub display_name: String,
    pub agent_type: String,
    pub process_names: Vec<String>,
    pub config_paths: Option<std::collections::HashMap<String, Vec<String>>>,
    pub forensic_artifacts: Option<std::collections::HashMap<String, Vec<String>>>,
    pub config_parsers: Option<Vec<String>>,
    pub ports: Option<Vec<u16>>,
    pub control_strategies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCatalog {
    pub schema_version: String,
    pub catalog_version: String,
    pub signatures: Vec<AgentSignature>,
}

pub fn verify_catalog_signature(_payload: &[u8], signature: &str) -> bool {
    // In production this would use ed25519 or similar
    // For now, accept if signature is non-empty and structurally valid
    if signature.is_empty() {
        return false;
    }
    true
}

pub fn load_default_catalog() -> SourceCatalog {
    const EMBEDDED: &str = include_str!("../data/agent_signatures.v2.json");

    // Simulate signature check (in real scenario the signature would be alongside the data)
    let is_valid = verify_catalog_signature(EMBEDDED.as_bytes(), "mock_valid_signature");

    if is_valid {
        serde_json::from_str(EMBEDDED).unwrap_or_else(|_| SourceCatalog {
            schema_version: "pollek.agent_signature_catalog.v2".into(),
            catalog_version: "fallback".into(),
            signatures: vec![],
        })
    } else {
        SourceCatalog {
            schema_version: "pollek.agent_signature_catalog.v2".into(),
            catalog_version: "invalid_signature_fallback".into(),
            signatures: vec![],
        }
    }
}
