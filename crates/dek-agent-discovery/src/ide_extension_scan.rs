use crate::model::*;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub fn scan_ide_extensions() -> Result<Vec<DiscoveryEvidenceV2>> {
    let mut evidence = Vec::new();

    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    if home.is_empty() {
        return Ok(evidence);
    }

    let mut vscode_ext_dir = PathBuf::from(&home);
    vscode_ext_dir.push(".vscode");
    vscode_ext_dir.push("extensions");

    if vscode_ext_dir.exists() {
        if let Ok(entries) = fs::read_dir(&vscode_ext_dir) {
            for entry in entries.filter_map(Result::ok) {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let name = entry
                            .file_name()
                            .to_string_lossy()
                            .to_string()
                            .to_lowercase();
                        if name.contains("github.copilot")
                            || name.contains("cline")
                            || name.contains("claude-dev")
                            || name.contains("continue")
                            || name.contains("sourcegraph.cody")
                        {
                            evidence.push(DiscoveryEvidenceV2 {
                                evidence_id: uuid::Uuid::new_v4().to_string(),
                                source: EvidenceSource::IdeExtension,
                                confidence: 0.90,
                                observed_at: chrono::Utc::now().to_rfc3339(),
                                privacy_class: PrivacyClass::InternalMetadata,
                                redacted: true,
                                data: serde_json::json!({
                                    "extension_folder": name
                                }),
                                merge_key: Some(format!(
                                    "vscode:{}",
                                    name.split('-').next().unwrap_or(&name)
                                )),
                                source_path_hash: Some(crate::redaction::sha256_string(
                                    &entry.path().to_string_lossy(),
                                )),
                                source_path_redacted: Some(name),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(evidence)
}
