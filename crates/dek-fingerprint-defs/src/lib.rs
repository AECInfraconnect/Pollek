pub mod loader;
pub mod merge;
pub mod model;
pub mod model_classifier;
pub mod store;
pub mod verify;

use model::*;

pub fn embedded_baseline() -> FingerprintDefinition {
    const BASELINE: &str = include_str!("../data/baseline.v4.json");
    serde_json::from_str(BASELINE).unwrap_or_else(|_| FingerprintDefinition {
        schema_version: "pollek.def.v4".into(),
        definition_version: 0,
        released_at: "1970-01-01T00:00:00Z".into(),
        min_engine_version: "0.0.0".into(),
        kind: DefinitionKind::Full,
        base_version: None,
        signatures: vec![],
        removed_ids: vec![],
        catalog_hash: String::new(),
        model_classifier: None,
        web_ai_signatures: vec![],
        installed_app_signatures: vec![],
        browser_processes: vec![],
        ai_process_hints: AiProcessHints::default(),
        cloud_resource_signatures: vec![],
        collapse_rules: vec![],
    })
}

fn get_data_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("DEK_STATE_DIR") {
        return std::path::PathBuf::from(dir);
    }
    if let Ok(dir) = std::env::var("DEK_DATA_DIR") {
        return std::path::PathBuf::from(dir);
    }
    #[cfg(target_os = "windows")]
    {
        let program_data =
            std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        std::path::PathBuf::from(program_data)
            .join("PollekDEK")
            .join("state")
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::path::PathBuf::from("/var/lib/pollek-dek")
    }
}

pub fn load_latest_baseline() -> FingerprintDefinition {
    let baseline = embedded_baseline();

    // Attempt to load dynamically from bundle
    let bundle_dir = get_data_dir().join("bundles").join("latest");

    let baseline_path = bundle_dir.join("baseline.v4.json");
    if baseline_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&baseline_path) {
            if let Ok(parsed) = serde_json::from_str::<FingerprintDefinition>(&content) {
                tracing::info!("Loaded dynamic baseline.v4.json from bundle");
                return parsed;
            } else {
                tracing::warn!(
                    "Failed to parse dynamic baseline.v4.json, falling back to embedded"
                );
            }
        }
    }

    baseline
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedded_baseline_has_browser_ai_definition_sections() -> anyhow::Result<()> {
        let baseline: crate::model::FingerprintDefinition =
            serde_json::from_str(include_str!("../data/baseline.v4.json"))?;

        assert_eq!(baseline.schema_version, "pollek.def.v4");
        assert!(!baseline.collapse_rules.is_empty());
        assert!(!baseline.browser_processes.is_empty());
        assert!(baseline.browser_processes.iter().any(|browser| browser
            .process_names
            .iter()
            .any(|name| name.eq_ignore_ascii_case("chrome.exe"))));
        assert!(baseline.ai_process_hints.require_match);
        assert!(baseline
            .web_ai_signatures
            .iter()
            .any(|sig| sig.id == "chatgpt_web" && sig.name == "ChatGPT (Web)"));
        assert!(baseline
            .web_ai_signatures
            .iter()
            .any(|sig| sig.id == "google_ai_studio_web"
                && sig.canonical_service_id == "google_ai_studio"
                && sig
                    .not_alias_domains
                    .iter()
                    .any(|d| d == "gemini.google.com")));
        Ok(())
    }

    #[test]
    fn embedded_baseline_covers_claw_family_engines_and_agentic_browsers() -> anyhow::Result<()> {
        let baseline: crate::model::FingerprintDefinition =
            serde_json::from_str(include_str!("../data/baseline.v4.json"))?;

        // Claw family: current + legacy footprints on one signature.
        let openclaw = baseline
            .signatures
            .iter()
            .find(|s| s.id == "openclaw")
            .ok_or_else(|| anyhow::anyhow!("openclaw signature missing"))?;
        assert!(openclaw.ports.contains(&18789), "gateway port footprint");
        assert!(openclaw.cli_binaries.iter().any(|b| b == "clawdbot"));
        assert!(openclaw.cli_binaries.iter().any(|b| b == "moltbot"));
        assert!(openclaw
            .install_markers
            .iter()
            .any(|m| m.path.contains(".clawdbot")));

        // Local model engines running under third-party runtimes.
        for id in [
            "vllm",
            "ollama",
            "lmstudio",
            "sglang",
            "tgi",
            "xinference",
            "llamafile",
            "mlx_lm",
            "anythingllm",
            "msty",
            "koboldcpp",
        ] {
            assert!(
                baseline.signatures.iter().any(|s| s.id == id),
                "engine signature {id} missing"
            );
        }

        // Black-box / agentic browser coverage.
        for id in [
            "comet_browser",
            "dia_browser",
            "chatgpt_atlas_browser",
            "headless_browser_automation",
            "browser_use_agent",
        ] {
            assert!(
                baseline.signatures.iter().any(|s| s.id == id),
                "browser agent signature {id} missing"
            );
        }
        for id in [
            "qwen_web",
            "kimi_web",
            "zai_web",
            "notebooklm_web",
            "genspark_web",
            "chatgpt_operator_web",
        ] {
            assert!(
                baseline.web_ai_signatures.iter().any(|s| s.id == id),
                "web ai signature {id} missing"
            );
        }
        assert!(baseline
            .browser_processes
            .iter()
            .any(|b| b.process_names.iter().any(|n| n == "comet")));
        Ok(())
    }
}
