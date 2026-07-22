// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::model::{ControlMode, DeployPresetRequest, PolicyPresetV2, RenderedArtifact};
use dek_control_plane_api::policy::{
    PolicyCompileOptions, PolicyDraft, PolicySource, PolicyTargets, PolicyType,
};
use dek_control_plane_api::registry::ObjectMeta;
use dek_guard_pipeline::config::{GuardConfig, GuardMode};

pub fn render(
    preset: &PolicyPresetV2,
    req: &DeployPresetRequest,
) -> anyhow::Result<Vec<RenderedArtifact>> {
    // Basic validation of unknown params
    for key in req.params.keys() {
        if !preset.parameters.iter().any(|p| p.key == *key) {
            return Err(anyhow::anyhow!("Unknown parameter: {}", key));
        }
    }

    // Basic validation of required
    for param in &preset.parameters {
        if !req.params.contains_key(&param.key) && param.required {
            return Err(anyhow::anyhow!("Missing required parameter: {}", param.key));
        }
    }

    let mut artifacts = Vec::new();

    match preset.id.as_str() {
        "pii.redact_before_external_llm" => {
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.pii.redact_before_external_llm",
                "default allow := true\n# GuardPipeline enforces PII redaction in the response filter data plane".into(),
            ));
            artifacts.push(render_guard_pipeline_config(preset, req)?);
        }
        "fs.folder_allowlist" => {
            // Deny-by-default folder scope. The allowlist comes from the deploy
            // request's resource targets; with no targets nothing is allowed,
            // which is the strict (and honest) default for an allowlist.
            let folders = req
                .targets
                .resource_ids
                .iter()
                .map(|f| format!("\"{}\"", f.replace('\\', "/").replace('"', "")))
                .collect::<Vec<_>>()
                .join(", ");
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.fs.folder_scope",
                format!(
                    "import future.keywords.if\nimport future.keywords.in\n\n\
                     default allow := false\n\n\
                     allowed_folders := [{folders}]\n\n\
                     allow if {{\n  some folder in allowed_folders\n  startswith(input.resource, folder)\n}}\n"
                ),
            ));
            artifacts.push(RenderedArtifact::pep_config(
                "{\n  \"action\": \"block_fs\"\n}".into(),
            ));
        }
        "budget.daily_token_cap" => {
            let cap = req
                .params
                .get("max_daily_total_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(1_000_000);
            // Advisory PDP policy mirroring the enforced limit, so rego-based
            // PEPs deny once reported daily usage crosses the cap.
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.budget.daily_token_cap",
                format!(
                    "import future.keywords.if\n\n\
                     default allow := true\n\n\
                     max_daily_total_tokens := {cap}\n\n\
                     allow := false if {{\n  input.usage.window == \"daily\"\n  input.usage.total_tokens > max_daily_total_tokens\n}}\n"
                ),
            ));
            // The binding that actually enforces: an ai-budget-limit consumed
            // by the real budget engine (dek-agent-observer::usage_budget).
            artifacts.push(budget_limit_artifact(
                preset,
                req,
                "daily",
                Some(cap),
                None,
            )?);
        }
        "budget.monthly_cost_cap" => {
            let cap_usd = req
                .params
                .get("max_monthly_usd")
                .and_then(|v| v.as_f64())
                .unwrap_or(100.0);
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.budget.monthly_cost_cap",
                format!(
                    "import future.keywords.if\n\n\
                     default allow := true\n\n\
                     max_monthly_cost_usd := {cap_usd}\n\n\
                     allow := false if {{\n  input.cost.window == \"monthly\"\n  input.cost.currency == \"USD\"\n  input.cost.total_cost > max_monthly_cost_usd\n}}\n"
                ),
            ));
            artifacts.push(budget_limit_artifact(
                preset,
                req,
                "monthly",
                None,
                Some(cap_usd),
            )?);
        }
        "content.prompt_injection_guard" => {
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.content.prompt_injection_guard",
                "default allow := true\n# GuardPipeline evaluates prompt injection before PDP routing".into(),
            ));
            artifacts.push(render_guard_pipeline_config(preset, req)?);
        }
        "content.system_prompt_leak_guard" => {
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.content.system_prompt_leak_guard",
                "default allow := true\n# GuardPipeline evaluates model output through /v1/filter/response".into(),
            ));
            artifacts.push(render_guard_pipeline_config(preset, req)?);
        }
        "secrets.block_api_key_exposure" => {
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.secrets.block_api_key_exposure",
                "default allow := true\n# GuardPipeline redacts or blocks secret echo before returning output".into(),
            ));
            artifacts.push(render_guard_pipeline_config(preset, req)?);
        }
        "fs.secrets_file_guard" => {
            // Blocks reads/writes on well-known credential material locations.
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.fs.secrets_file_guard",
                "import future.keywords.if\nimport future.keywords.in\n\n\
                 default allow := true\n\n\
                 secret_path_markers := [\".env\", \".pem\", \".key\", \"id_rsa\", \"id_ed25519\", \".aws/credentials\", \".ssh/\", \".kube/config\", \".netrc\", \".npmrc\", \"secrets.\", \"credentials.json\", \"service-account\", \".pgpass\"]\n\n\
                 allow := false if {\n  some marker in secret_path_markers\n  contains(lower(input.resource), marker)\n}\n"
                    .into(),
            ));
        }
        "personal.email_send_approval" => {
            // Deny email sends unless the approval flow has granted this
            // request (the PEP injects `approval_granted` into the Cedar
            // context after a human approves).
            artifacts.push(RenderedArtifact::cedar(
                "personal.email_send_approval",
                "forbid (principal, action, resource)\nwhen { !(context has approval_granted && context.approval_granted == true) };"
                    .into(),
            ));
        }
        "personal.drive_external_share_guard" => {
            // External shares are denied unless explicitly approved; internal
            // shares (context.share_scope == \"internal\") pass through.
            artifacts.push(RenderedArtifact::cedar(
                "personal.drive_external_share_guard",
                "forbid (principal, action, resource)\nwhen { context has share_scope && context.share_scope == \"external\" && !(context has approval_granted && context.approval_granted == true) };"
                    .into(),
            ));
        }
        "network.shadow_ai_external_llm_block" => {
            // Blocks direct egress to well-known external LLM provider APIs so
            // traffic must flow through the governed proxy instead.
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.network.shadow_ai",
                "import future.keywords.if\nimport future.keywords.in\n\n\
                 default allow := true\n\n\
                 external_llm_hosts := [\"api.openai.com\", \"api.anthropic.com\", \"generativelanguage.googleapis.com\", \"api.mistral.ai\", \"api.deepseek.com\", \"openrouter.ai\", \"api.groq.com\", \"api.together.xyz\", \"api.x.ai\", \"api.cohere.com\", \"api.perplexity.ai\"]\n\n\
                 allow := false if {\n  some host in external_llm_hosts\n  endswith(lower(input.resource), host)\n}\n"
                    .into(),
            ));
        }
        "mcp.high_risk_tool_approval" => {
            // High-risk tool calls require an explicit human approval; the MCP
            // PEP tags risk in the Cedar context from the tool's risk tags.
            artifacts.push(RenderedArtifact::cedar(
                "mcp.high_risk_tool_approval",
                "forbid (principal, action, resource)\nwhen { context has risk_level && context.risk_level == \"high\" && !(context has approval_granted && context.approval_granted == true) };"
                    .into(),
            ));
        }
        "mcp.tool_allowlist" => {
            // Authorization model: an agent may call a tool only when it holds
            // the caller relation on that tool.
            artifacts.push(RenderedArtifact::openfga(
                "mcp.tool_allowlist",
                "model\n  schema 1.1\ntype agent\ntype tool\n  relations\n    define caller: [agent]\n"
                    .into(),
            ));
        }
        "personal.drive_folder_scope" => {
            artifacts.push(RenderedArtifact::openfga(
                "personal.drive_folder_scope",
                "model\n  schema 1.1\ntype user\ntype folder\n  relations\n    define viewer: [user]\ntype document\n  relations\n    define parent: [folder]\n    define viewer: viewer from parent\n".into(),
            ));
        }
        _ => {
            return Err(anyhow::anyhow!("Unsupported preset: {}", preset.id));
        }
    }

    Ok(artifacts)
}

/// Render an `ai-budget-limit.v1` binding for the REAL budget engine
/// (`dek-agent-observer::usage_budget::evaluate_budget`). The deploy handler
/// upserts this into the budget store, so the cap is actually enforced instead
/// of only being described by an advisory policy.
fn budget_limit_artifact(
    preset: &PolicyPresetV2,
    req: &DeployPresetRequest,
    window: &str,
    hard_token_limit: Option<i64>,
    hard_cost_limit: Option<f64>,
) -> anyhow::Result<RenderedArtifact> {
    // Control mode decides what the engine does at the cap.
    let (soft_frac, action_on_soft, action_on_hard, hard_enabled) = match req.control_mode {
        ControlMode::Observe => (1.0, "warn", "warn", false),
        ControlMode::Warn => (0.8, "warn", "warn", false),
        ControlMode::Approval => (0.8, "warn", "require_approval", true),
        ControlMode::Enforce | ControlMode::StrictDeny => (0.8, "warn", "deny", true),
    };

    // Scope: a single targeted agent when given, otherwise the whole tenant.
    let (scope_type, scope_id) = match req.targets.agent_ids.as_slice() {
        [only] => ("agent", only.clone()),
        _ => ("tenant", String::new()),
    };

    let value = serde_json::json!({
        "schema_version": "ai-budget-limit.v1",
        "budget_id": format!("preset_{}", preset.id.replace('.', "_")),
        "tenant_id": "",
        "scope_type": scope_type,
        "scope_id": scope_id,
        "window": window,
        "currency": "USD",
        "soft_cost_limit": hard_cost_limit.map(|v| v * soft_frac),
        "hard_cost_limit": if hard_enabled { hard_cost_limit } else { None },
        "soft_token_limit": hard_token_limit.map(|v| (v as f64 * soft_frac) as i64),
        "hard_token_limit": if hard_enabled { hard_token_limit } else { None },
        "action_on_soft": action_on_soft,
        "action_on_hard": action_on_hard,
        "enabled": true,
        "created_at": "",
        "updated_at": "",
    });
    Ok(RenderedArtifact {
        language: "budget_limit".into(),
        content: serde_json::to_string_pretty(&value)?,
        warnings: vec![],
    })
}

fn guard_mode_for_control_mode(control_mode: &ControlMode) -> GuardMode {
    match control_mode {
        ControlMode::Observe => GuardMode::Observe,
        ControlMode::Warn | ControlMode::Approval => GuardMode::Warn,
        ControlMode::Enforce => GuardMode::Enforce,
        ControlMode::StrictDeny => GuardMode::StrictDeny,
    }
}

fn guard_config_for_preset(preset_id: &str, control_mode: &ControlMode) -> Option<GuardConfig> {
    let mut cfg = GuardConfig {
        mode: guard_mode_for_control_mode(control_mode),
        ..GuardConfig::default()
    };

    match preset_id {
        "content.prompt_injection_guard" => {
            cfg.request_guard_enabled = true;
            cfg.response_guard_enabled = false;
            Some(cfg)
        }
        "content.system_prompt_leak_guard" => {
            cfg.request_guard_enabled = false;
            cfg.response_guard_enabled = true;
            Some(cfg)
        }
        "pii.redact_before_external_llm" | "secrets.block_api_key_exposure" => {
            cfg.request_guard_enabled = true;
            cfg.response_guard_enabled = true;
            Some(cfg)
        }
        _ => None,
    }
}

fn render_guard_pipeline_config(
    preset: &PolicyPresetV2,
    req: &DeployPresetRequest,
) -> anyhow::Result<RenderedArtifact> {
    let Some(config) = guard_config_for_preset(&preset.id, &req.control_mode) else {
        return Err(anyhow::anyhow!(
            "Preset does not support GuardPipeline config: {}",
            preset.id
        ));
    };
    let value = serde_json::json!({
        "schema_version": "guard-pipeline-config.v1",
        "preset_id": &preset.id,
        "preset_version": &preset.version,
        "control_mode": &req.control_mode,
        "data_plane": "/v1/filter/response",
        "guard_pipeline": config
    });
    Ok(RenderedArtifact::pep_config(serde_json::to_string_pretty(
        &value,
    )?))
}

pub fn to_policy_draft(
    tenant_id: &str,
    preset: &PolicyPresetV2,
    req: &DeployPresetRequest,
) -> anyhow::Result<Option<PolicyDraft>> {
    let rendered = render(preset, req)?;

    // Find the first rego, cedar, or openfga artifact to turn into a draft
    let policy_artifact = rendered
        .into_iter()
        .find(|a| a.language == "rego" || a.language == "cedar" || a.language == "openfga");

    if let Some(artifact) = policy_artifact {
        let policy_type = match artifact.language.as_str() {
            "rego" => PolicyType::Rego,
            "cedar" => PolicyType::Cedar,
            "openfga" => PolicyType::OpenFga,
            _ => return Err(anyhow::anyhow!("Unknown language type")),
        };

        let targets = PolicyTargets {
            agent_ids: req.targets.agent_ids.clone(),
            tool_ids: req.targets.tool_ids.clone(),
            resource_ids: req.targets.resource_ids.clone(),
            entity_ids: vec![],
            route_ids: vec![],
        };

        let draft = PolicyDraft {
            meta: ObjectMeta {
                schema_version: "v1".to_string(),
                tenant_id: tenant_id.to_string(),
                workspace_id: "default".to_string(),
                environment_id: "default".to_string(),
                created_at: "".to_string(),
                updated_at: "".to_string(),
                created_by: "".to_string(),
                updated_by: "".to_string(),
                source: dek_control_plane_api::registry::RegistrationSource::Manual,
                status: dek_control_plane_api::registry::RegistryStatus::Draft,
                tags: vec![],
            },
            policy_id: format!(
                "pol_{}_{}",
                preset.id.replace('.', "_"),
                uuid::Uuid::new_v4().simple()
            ),
            name: preset.title.clone(),
            description: Some(preset.short_description.clone()),
            policy_type,
            targets,
            source: PolicySource::RawText {
                language: artifact.language,
                text: artifact.content,
            },
            compile_options: PolicyCompileOptions {
                optimization_level: None,
                fail_on_warnings: Some(true),
            },
        };
        Ok(Some(draft))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::get_builtin_preset;
    use crate::model::{ControlMode, PresetTargets};
    use serde::Deserialize;

    const GUARD_PRESET_CORPUS: &str = include_str!("../tests/corpus/guard_preset_config.jsonl");

    #[derive(Debug, Deserialize)]
    struct GuardPresetCorpusCase {
        id: String,
        preset_id: String,
        control_mode: String,
        expected_guard_mode: String,
        status: String,
    }

    fn control_mode_from_name(value: &str) -> anyhow::Result<ControlMode> {
        Ok(serde_json::from_value(serde_json::json!(value))?)
    }

    fn guard_config_json(artifacts: &[RenderedArtifact]) -> anyhow::Result<serde_json::Value> {
        for artifact in artifacts {
            if artifact.language == "json" && artifact.content.contains("guard_pipeline") {
                return Ok(serde_json::from_str(&artifact.content)?);
            }
        }
        Err(anyhow::anyhow!("missing guard pipeline config artifact"))
    }

    fn request(preset: &PolicyPresetV2, control_mode: ControlMode) -> DeployPresetRequest {
        let mut params: std::collections::BTreeMap<String, serde_json::Value> = Default::default();
        for param in &preset.parameters {
            if param.required {
                params.insert(param.key.clone(), param.default_value.clone());
            }
        }
        DeployPresetRequest {
            preset_id: preset.id.clone(),
            preset_version: None,
            control_mode,
            selected_pep_types: Vec::new(),
            targets: PresetTargets::default(),
            params,
            dry_run_first: true,
            pdp_route: None,
        }
    }

    #[test]
    fn guard_control_mode_mapping_matches_golden_corpus() -> anyhow::Result<()> {
        for line in GUARD_PRESET_CORPUS
            .lines()
            .filter(|line| !line.trim().is_empty())
        {
            let case: GuardPresetCorpusCase = serde_json::from_str(line)?;
            if case.status != "active" {
                continue;
            }
            let Some(preset) = get_builtin_preset(&case.preset_id) else {
                return Err(anyhow::anyhow!("unknown preset {}", case.preset_id));
            };
            let req = request(&preset, control_mode_from_name(&case.control_mode)?);
            let rendered = render(&preset, &req)?;
            let config = guard_config_json(&rendered)?;
            let mode = config
                .get("guard_pipeline")
                .and_then(|value| value.get("mode"))
                .and_then(|value| value.as_str())
                .ok_or_else(|| anyhow::anyhow!("missing guard mode"))?;

            assert!(case.id.starts_with("rt-pr9-"));
            assert_eq!(mode, case.expected_guard_mode);
            assert_eq!(config["data_plane"], "/v1/filter/response");
        }
        Ok(())
    }

    #[test]
    fn approval_control_mode_maps_to_warn_guard_mode() {
        assert_eq!(
            guard_mode_for_control_mode(&ControlMode::Approval),
            GuardMode::Warn
        );
    }

    #[test]
    fn no_preset_renders_placeholder_policy_bodies() -> anyhow::Result<()> {
        // Every builtin preset must render real policy logic — a TODO body is
        // a fake control that claims protection it does not provide.
        for preset in crate::catalog::builtin_presets() {
            let req = request(&preset, preset.default_control_mode.clone());
            let rendered = render(&preset, &req)?;
            assert!(!rendered.is_empty(), "{} rendered nothing", preset.id);
            for artifact in &rendered {
                assert!(
                    !artifact.content.contains("TODO"),
                    "{} renders a placeholder body: {}",
                    preset.id,
                    artifact.content
                );
            }
        }
        Ok(())
    }

    #[test]
    fn budget_presets_bind_to_the_real_budget_engine() -> anyhow::Result<()> {
        for (preset_id, expect_tokens, expect_cost) in [
            ("budget.daily_token_cap", true, false),
            ("budget.monthly_cost_cap", false, true),
        ] {
            let preset = get_builtin_preset(preset_id)
                .ok_or_else(|| anyhow::anyhow!("missing preset {preset_id}"))?;
            let req = request(&preset, ControlMode::Enforce);
            let rendered = render(&preset, &req)?;
            let budget = rendered
                .iter()
                .find(|a| a.language == "budget_limit")
                .ok_or_else(|| anyhow::anyhow!("{preset_id} must render a budget_limit"))?;
            let value: serde_json::Value = serde_json::from_str(&budget.content)?;
            assert_eq!(value["schema_version"], "ai-budget-limit.v1");
            assert_eq!(value["action_on_hard"], "deny");
            assert_eq!(
                value["hard_token_limit"].is_i64(),
                expect_tokens,
                "{preset_id} hard_token_limit"
            );
            assert_eq!(
                value["hard_cost_limit"].is_f64(),
                expect_cost,
                "{preset_id} hard_cost_limit"
            );
        }
        Ok(())
    }
}
