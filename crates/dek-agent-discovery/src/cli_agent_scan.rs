use crate::model::*;
use anyhow::Result;

pub fn scan_cli_agents() -> Result<Vec<DiscoveryEvidenceV2>> {
    let mut evidence = Vec::new();

    // Stub logic: If we had a catalog, we'd check ~/.codex/config.toml, ~/.config/claude-code, etc.
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    if home.is_empty() {
        return Ok(evidence);
    }

    // Claude Code Stub
    let mut claude_code_config = std::path::PathBuf::from(&home);
    claude_code_config.push(".claude.json");
    if claude_code_config.exists() {
        evidence.push(DiscoveryEvidenceV2 {
            evidence_id: uuid::Uuid::new_v4().to_string(),
            source: EvidenceSource::CliAgent,
            confidence: 0.85,
            observed_at: chrono::Utc::now().to_rfc3339(),
            privacy_class: PrivacyClass::InternalMetadata,
            redacted: true,
            data: serde_json::json!({
                "cli_agent": "claude-code"
            }),
            merge_key: Some("cli:claude-code".into()),
            source_path_hash: Some(crate::redaction::sha256_string(
                &claude_code_config.to_string_lossy(),
            )),
            source_path_redacted: Some(".claude.json".into()),
        });
    }

    Ok(evidence)
}
