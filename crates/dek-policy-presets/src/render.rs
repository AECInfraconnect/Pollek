// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::model::{DeployPresetRequest, PolicyPresetV2, RenderedArtifact};
use dek_control_plane_api::policy::{
    PolicyCompileOptions, PolicyDraft, PolicySource, PolicyTargets, PolicyType,
};
use dek_control_plane_api::registry::ObjectMeta;

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
                "default allow := true\n# TODO: PII redaction logic".into(),
            ));
            artifacts.push(RenderedArtifact::pep_config(
                "{\n  \"action\": \"redact\"\n}".into(),
            ));
        }
        "fs.folder_allowlist" => {
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.fs.folder_scope",
                "default allow := false\n# TODO: Folder allowlist logic".into(),
            ));
            artifacts.push(RenderedArtifact::pep_config(
                "{\n  \"action\": \"block_fs\"\n}".into(),
            ));
        }
        "budget.daily_token_cap" => {
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.budget.daily_token_cap",
                "default allow := true\n# TODO: Budget logic".into(),
            ));
        }
        "budget.monthly_cost_cap" => {
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.budget.monthly_cost_cap",
                "default allow := true\n# TODO: Monthly budget logic".into(),
            ));
        }
        "content.prompt_injection_guard" => {
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.content.prompt_injection_guard",
                "default allow := true\n# TODO: Prompt injection logic".into(),
            ));
        }
        "content.system_prompt_leak_guard" => {
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.content.system_prompt_leak_guard",
                "default allow := true\n# TODO: System prompt leak guard logic".into(),
            ));
        }
        "secrets.block_api_key_exposure" => {
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.secrets.block_api_key_exposure",
                "default allow := true\n# TODO: Secrets guard logic".into(),
            ));
        }
        "fs.secrets_file_guard" => {
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.fs.secrets_file_guard",
                "default allow := true\n# TODO: FS Secrets guard logic".into(),
            ));
        }
        "personal.email_send_approval" => {
            artifacts.push(RenderedArtifact::cedar(
                "personal.email_send_approval",
                "forbid (principal, action, resource); // TODO: Email approval logic".into(),
            ));
        }
        "personal.drive_external_share_guard" => {
            artifacts.push(RenderedArtifact::cedar(
                "personal.drive_external_share_guard",
                "forbid (principal, action, resource); // TODO: Drive approval logic".into(),
            ));
        }
        "network.shadow_ai_external_llm_block" => {
            artifacts.push(RenderedArtifact::rego(
                "pollek.presets.network.shadow_ai",
                "default allow := true\n# TODO: Network logic".into(),
            ));
        }
        "mcp.high_risk_tool_approval" => {
            artifacts.push(RenderedArtifact::cedar(
                "mcp.high_risk_tool_approval",
                "forbid (principal, action, resource); // TODO: High risk tool logic".into(),
            ));
        }
        "mcp.tool_allowlist" => {
            artifacts.push(RenderedArtifact::openfga(
                "mcp.tool_allowlist",
                "model\n  schema 1.1\n// TODO: OpenFGA logic".into(),
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
