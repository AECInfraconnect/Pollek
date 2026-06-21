use crate::config_paths::get_known_config_paths;
use crate::model::*;
use anyhow::Result;

pub fn scan_mcp_configs() -> Result<Vec<DiscoveryEvidenceV2>> {
    let mut evidence = Vec::new();
    let paths = get_known_config_paths();

    if let Ok(configs) = crate::mcp_config::discover_mcp_configs(&paths) {
        for cfg in configs {
            let data = serde_json::to_value(&cfg).unwrap_or_default();
            evidence.push(DiscoveryEvidenceV2 {
                evidence_id: uuid::Uuid::new_v4().to_string(),
                source: EvidenceSource::McpConfig,
                confidence: 0.95,
                observed_at: chrono::Utc::now().to_rfc3339(),
                privacy_class: PrivacyClass::InternalMetadata,
                redacted: true,
                data,
                merge_key: Some(format!("mcp_{}_{}", cfg.client_hint, cfg.server_name)),
                source_path_hash: Some(cfg.config_path_hash.clone()),
                source_path_redacted: Some(cfg.config_path_redacted.clone()),
            });
        }
    }

    Ok(evidence)
}
