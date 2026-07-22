use crate::model::*;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn scan_cli_agents() -> Result<Vec<DiscoveryEvidenceV2>> {
    let mut evidence = Vec::new();

    // Real filesystem probe: each known CLI agent leaves a well-known config
    // file under the user's home directory; presence of that file is the
    // installed-agent evidence we report (path is hashed + redacted).
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
        push_cli_evidence(
            &mut evidence,
            "claude_code",
            "Claude Code CLI",
            "Anthropic",
            "Claude Code",
            &claude_code_config,
            ".claude.json",
        );
    }

    let mut codex_config = PathBuf::from(&home);
    codex_config.push(".codex/config.toml");
    if codex_config.exists() {
        push_cli_evidence(
            &mut evidence,
            "codex_cli",
            "OpenAI Codex (CLI)",
            "OpenAI",
            "Codex",
            &codex_config,
            ".codex/config.toml",
        );
    }

    let mut gemini_config = PathBuf::from(&home);
    gemini_config.push(".gemini/settings.json");
    if gemini_config.exists() {
        push_cli_evidence(
            &mut evidence,
            "gemini_cli",
            "Google Gemini (CLI)",
            "Google",
            "Gemini CLI",
            &gemini_config,
            ".gemini/settings.json",
        );
    }

    // Aider
    let mut aider_config = std::path::PathBuf::from(&home);
    aider_config.push(".aider.conf.yml");
    if aider_config.exists() {
        push_cli_evidence(
            &mut evidence,
            "aider",
            "Aider",
            "Aider",
            "Aider",
            &aider_config,
            ".aider.conf.yml",
        );
    }

    // Goose
    let mut goose_config = std::path::PathBuf::from(&home);
    goose_config.push(".config/goose/config.toml");
    if goose_config.exists() {
        push_cli_evidence(
            &mut evidence,
            "goose",
            "Goose",
            "Block",
            "Goose",
            &goose_config,
            ".config/goose/config.toml",
        );
    }

    // Open Interpreter
    let mut oi_config = std::path::PathBuf::from(&home);
    oi_config.push(".config/open-interpreter/config.yaml");
    if oi_config.exists() {
        push_cli_evidence(
            &mut evidence,
            "open_interpreter",
            "Open Interpreter",
            "Open Interpreter",
            "Open Interpreter",
            &oi_config,
            ".config/open-interpreter/config.yaml",
        );
    }

    // Cline / Roo Code (VSCode extensions typically store global state, but CLI variants exist)
    let mut cline_config = std::path::PathBuf::from(&home);
    cline_config.push(".cline");
    if cline_config.exists() {
        push_cli_evidence(
            &mut evidence,
            "cline",
            "Cline",
            "Cline",
            "Cline",
            &cline_config,
            ".cline",
        );
    }

    let mut continue_config = PathBuf::from(&home);
    continue_config.push(".continue/config.json");
    if continue_config.exists() {
        push_cli_evidence(
            &mut evidence,
            "continue",
            "Continue",
            "Continue",
            "Continue",
            &continue_config,
            ".continue/config.json",
        );
    }

    Ok(evidence)
}

fn push_cli_evidence(
    evidence: &mut Vec<DiscoveryEvidenceV2>,
    key: &str,
    name: &str,
    vendor: &str,
    product: &str,
    path: &Path,
    redacted_path: &str,
) {
    evidence.push(DiscoveryEvidenceV2 {
        evidence_id: uuid::Uuid::new_v4().to_string(),
        source: EvidenceSource::CliAgent,
        confidence: 0.85,
        observed_at: chrono::Utc::now().to_rfc3339(),
        privacy_class: PrivacyClass::InternalMetadata,
        redacted: true,
        data: serde_json::json!({
            "cli_agent": key,
            "name": name,
            "vendor": vendor,
            "product": product,
            "capability_tags": ["code.agentic", "tool.use", "cli.agent"]
        }),
        merge_key: Some(format!("cli:{key}")),
        source_path_hash: Some(crate::redaction::sha256_string(&path.to_string_lossy())),
        source_path_redacted: Some(redacted_path.into()),
    });
}
