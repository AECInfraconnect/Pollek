// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::model::*;

pub fn builtin_presets() -> Vec<PolicyPresetV2> {
    vec![
        pii_redact_before_external_llm(),
        content_prompt_injection_guard(),
        content_system_prompt_leak_guard(),
        secrets_block_api_key_exposure(),
        fs_folder_allowlist(),
        fs_secrets_file_guard(),
        personal_email_send_approval(),
        personal_drive_external_share_guard(),
        budget_daily_token_cap(),
        budget_monthly_cost_cap(),
        network_shadow_ai_external_llm_block(),
        mcp_high_risk_tool_approval(),
        mcp_tool_allowlist(),
        personal_drive_folder_scope(),
    ]
}

pub fn get_builtin_preset(id: &str) -> Option<PolicyPresetV2> {
    builtin_presets().into_iter().find(|p| p.id == id)
}

pub fn pii_redact_before_external_llm() -> PolicyPresetV2 {
    PolicyPresetV2 {
        id: "pii.redact_before_external_llm".into(),
        version: "1.0.0".into(),
        title: "Redact PII Before External LLM".into(),
        short_description: "Redact personal data before prompts or tool outputs leave the device.".into(),
        long_description: "Detects PII/secrets in prompt, context, and tool output, then redacts or blocks before external provider egress.".into(),
        category: PresetCategory::PiiAndSecrets,
        risk_tags: vec![
            RiskTag::SensitiveInfoDisclosure,
            RiskTag::DataExfiltration,
            RiskTag::SecretLeakage,
        ],
        supported_pep_types: vec![
            PepType::McpProxy,
            PepType::StdioWrapper,
            PepType::HttpGateway,
            PepType::BrowserExtension,
        ],
        recommended_pep_types: vec![PepType::McpProxy, PepType::HttpGateway],
        supported_control_modes: vec![
            ControlMode::Observe,
            ControlMode::Warn,
            ControlMode::Approval,
            ControlMode::Enforce,
            ControlMode::StrictDeny,
        ],
        default_control_mode: ControlMode::Warn,
        supported_policy_outputs: vec![
            PolicyOutputKind::Rego,
            PolicyOutputKind::PepConfig,
            PolicyOutputKind::RedactionPipeline,
            PolicyOutputKind::TelemetryRule,
        ],
        parameters: vec![
            PresetParameter {
                key: "detectors".into(),
                label: "Sensitive Data Types".into(),
                description: "PII and secret detectors to enable.".into(),
                value_type: PresetValueType::StringList,
                required: true,
                default_value: serde_json::json!(["email", "phone", "api_key", "jwt", "ssh_private_key"]),
                examples: vec![serde_json::json!(["email", "national_id", "credit_card"])],
            },
            PresetParameter {
                key: "external_provider_action".into(),
                label: "External Provider Action".into(),
                description: "Action when sensitive data would leave the device.".into(),
                value_type: PresetValueType::String,
                required: true,
                default_value: serde_json::json!("redact"),
                examples: vec![serde_json::json!("block"), serde_json::json!("require_approval")],
            },
        ],
        generated_artifacts: vec![
            ArtifactKind::PolicyDraft,
            ArtifactKind::SignedBundle,
            ArtifactKind::PepBinding,
            ArtifactKind::TelemetrySubscription,
            ArtifactKind::RollbackSnapshot,
        ],
        telemetry_requirements: vec![
            TelemetryRequirement {
                event_type: "content_scan".into(),
                required_fields: vec!["findings".into(), "redaction.applied".into()],
                pii_handling: PiiHandling::Redact,
            },
            TelemetryRequirement {
                event_type: "policy_decision".into(),
                required_fields: vec!["allow".into(), "reason".into(), "pep_type".into()],
                pii_handling: PiiHandling::Hash,
            },
        ],
        default_simulation_window: SimulationWindow::Last24Hours,
        safety_notes: vec![
            "Do not upload raw payloads to cloud telemetry unless explicitly enabled.".into(),
            "Redaction should run before provider egress and before telemetry export.".into(),
        ],
    }
}

pub fn content_prompt_injection_guard() -> PolicyPresetV2 {
    PolicyPresetV2 {
        id: "content.prompt_injection_guard".into(),
        version: "1.0.0".into(),
        title: "Prompt Injection Guard".into(),
        short_description: "Detect/block prompt injection and jailbreak-style instructions.".into(),
        long_description: "Scans input text for known prompt injection signatures and blocks execution if detected.".into(),
        category: PresetCategory::ContentGuard,
        risk_tags: vec![RiskTag::PromptInjection],
        supported_pep_types: vec![
            PepType::McpProxy,
            PepType::StdioWrapper,
            PepType::HttpGateway,
            PepType::LocalModelProxy,
        ],
        recommended_pep_types: vec![PepType::McpProxy, PepType::LocalModelProxy],
        supported_control_modes: vec![ControlMode::Observe, ControlMode::Warn, ControlMode::Enforce, ControlMode::StrictDeny],
        default_control_mode: ControlMode::Enforce,
        supported_policy_outputs: vec![PolicyOutputKind::Rego, PolicyOutputKind::PepConfig],
        parameters: vec![],
        generated_artifacts: vec![ArtifactKind::PolicyDraft, ArtifactKind::PepBinding],
        telemetry_requirements: vec![],
        default_simulation_window: SimulationWindow::Last24Hours,
        safety_notes: vec![],
    }
}

pub fn content_system_prompt_leak_guard() -> PolicyPresetV2 {
    PolicyPresetV2 {
        id: "content.system_prompt_leak_guard".into(),
        version: "1.0.0".into(),
        title: "System Prompt Leak Guard".into(),
        short_description: "Prevent model output from exposing system prompts.".into(),
        long_description: "Scans output text for internal system instructions or developer prompts and blocks them.".into(),
        category: PresetCategory::ContentGuard,
        risk_tags: vec![RiskTag::SensitiveInfoDisclosure],
        supported_pep_types: vec![PepType::McpProxy, PepType::HttpGateway, PepType::LocalModelProxy],
        recommended_pep_types: vec![PepType::HttpGateway],
        supported_control_modes: vec![ControlMode::Observe, ControlMode::Warn, ControlMode::Enforce, ControlMode::StrictDeny],
        default_control_mode: ControlMode::Enforce,
        supported_policy_outputs: vec![PolicyOutputKind::Rego],
        parameters: vec![],
        generated_artifacts: vec![ArtifactKind::PolicyDraft, ArtifactKind::PepBinding],
        telemetry_requirements: vec![],
        default_simulation_window: SimulationWindow::Last24Hours,
        safety_notes: vec![],
    }
}

pub fn secrets_block_api_key_exposure() -> PolicyPresetV2 {
    PolicyPresetV2 {
        id: "secrets.block_api_key_exposure".into(),
        version: "1.0.0".into(),
        title: "Block API Key Exposure".into(),
        short_description: "Detect and block API keys, tokens, SSH keys, JWTs.".into(),
        long_description:
            "Prevents high-value secrets from being exposed via prompt inputs or file system reads."
                .into(),
        category: PresetCategory::PiiAndSecrets,
        risk_tags: vec![RiskTag::SecretLeakage, RiskTag::DataExfiltration],
        supported_pep_types: vec![
            PepType::McpProxy,
            PepType::HttpGateway,
            PepType::FileSystemPep,
        ],
        recommended_pep_types: vec![PepType::McpProxy, PepType::FileSystemPep],
        supported_control_modes: vec![
            ControlMode::Observe,
            ControlMode::Warn,
            ControlMode::Enforce,
            ControlMode::StrictDeny,
        ],
        default_control_mode: ControlMode::Enforce,
        supported_policy_outputs: vec![PolicyOutputKind::Rego, PolicyOutputKind::PepConfig],
        parameters: vec![],
        generated_artifacts: vec![ArtifactKind::PolicyDraft, ArtifactKind::PepBinding],
        telemetry_requirements: vec![],
        default_simulation_window: SimulationWindow::Last24Hours,
        safety_notes: vec![],
    }
}

pub fn fs_folder_allowlist() -> PolicyPresetV2 {
    PolicyPresetV2 {
        id: "fs.folder_allowlist".into(),
        version: "1.0.0".into(),
        title: "Folder Allowlist".into(),
        short_description: "Permit access only to selected folders and globs.".into(),
        long_description: "Restricts the agent's file system access to a strict set of allowed roots and include/exclude patterns.".into(),
        category: PresetCategory::FileSystem,
        risk_tags: vec![RiskTag::UnsafeFileAccess],
        supported_pep_types: vec![PepType::McpProxy, PepType::StdioWrapper, PepType::FileSystemPep],
        recommended_pep_types: vec![PepType::FileSystemPep, PepType::StdioWrapper],
        supported_control_modes: vec![ControlMode::Observe, ControlMode::Approval, ControlMode::Enforce],
        default_control_mode: ControlMode::Enforce,
        supported_policy_outputs: vec![PolicyOutputKind::Rego, PolicyOutputKind::PepConfig],
        parameters: vec![],
        generated_artifacts: vec![ArtifactKind::PolicyDraft, ArtifactKind::PepBinding],
        telemetry_requirements: vec![],
        default_simulation_window: SimulationWindow::Last24Hours,
        safety_notes: vec!["Ensure absolute path normalization is performed at the PEP level.".into()],
    }
}

pub fn fs_secrets_file_guard() -> PolicyPresetV2 {
    PolicyPresetV2 {
        id: "fs.secrets_file_guard".into(),
        version: "1.0.0".into(),
        title: "Secrets File Guard".into(),
        short_description: "Block access to SSH keys, .env, cloud credentials.".into(),
        long_description: "Prevents reading files commonly known to contain credentials (e.g. ~/.ssh, .env, ~/.aws/credentials).".into(),
        category: PresetCategory::FileSystem,
        risk_tags: vec![RiskTag::UnsafeFileAccess, RiskTag::SecretLeakage],
        supported_pep_types: vec![PepType::FileSystemPep, PepType::StdioWrapper, PepType::LinuxEbpf],
        recommended_pep_types: vec![PepType::FileSystemPep],
        supported_control_modes: vec![ControlMode::Observe, ControlMode::Warn, ControlMode::Enforce, ControlMode::StrictDeny],
        default_control_mode: ControlMode::Enforce,
        supported_policy_outputs: vec![PolicyOutputKind::Rego],
        parameters: vec![],
        generated_artifacts: vec![ArtifactKind::PolicyDraft, ArtifactKind::PepBinding],
        telemetry_requirements: vec![],
        default_simulation_window: SimulationWindow::Last24Hours,
        safety_notes: vec![],
    }
}

pub fn personal_email_send_approval() -> PolicyPresetV2 {
    PolicyPresetV2 {
        id: "personal.email_send_approval".into(),
        version: "1.0.0".into(),
        title: "Email Send Approval".into(),
        short_description: "Require approval before sending email.".into(),
        long_description: "Intercepts email sending API calls and requires a human to explicitly approve the draft before sending.".into(),
        category: PresetCategory::PersonalResources,
        risk_tags: vec![RiskTag::ExcessiveAgency],
        supported_pep_types: vec![PepType::McpProxy, PepType::BrowserExtension],
        recommended_pep_types: vec![PepType::McpProxy],
        supported_control_modes: vec![ControlMode::Approval, ControlMode::Enforce, ControlMode::StrictDeny],
        default_control_mode: ControlMode::Approval,
        supported_policy_outputs: vec![PolicyOutputKind::Cedar, PolicyOutputKind::ApprovalWorkflow],
        parameters: vec![],
        generated_artifacts: vec![ArtifactKind::PolicyDraft, ArtifactKind::PepBinding, ArtifactKind::ApprovalRule],
        telemetry_requirements: vec![],
        default_simulation_window: SimulationWindow::Last7Days,
        safety_notes: vec![],
    }
}

pub fn personal_drive_external_share_guard() -> PolicyPresetV2 {
    PolicyPresetV2 {
        id: "personal.drive_external_share_guard".into(),
        version: "1.0.0".into(),
        title: "Drive External Share Guard".into(),
        short_description: "Require approval before creating public/external share links.".into(),
        long_description: "Intercepts file sharing API calls and blocks them or requires approval if the recipient domain is external.".into(),
        category: PresetCategory::PersonalResources,
        risk_tags: vec![RiskTag::DataExfiltration, RiskTag::ExcessiveAgency],
        supported_pep_types: vec![PepType::McpProxy, PepType::BrowserExtension, PepType::CloudConnectorProxy],
        recommended_pep_types: vec![PepType::McpProxy],
        supported_control_modes: vec![ControlMode::Approval, ControlMode::Enforce],
        default_control_mode: ControlMode::Approval,
        supported_policy_outputs: vec![PolicyOutputKind::Cedar, PolicyOutputKind::Rego],
        parameters: vec![],
        generated_artifacts: vec![ArtifactKind::PolicyDraft, ArtifactKind::PepBinding],
        telemetry_requirements: vec![],
        default_simulation_window: SimulationWindow::Last7Days,
        safety_notes: vec![],
    }
}

pub fn budget_daily_token_cap() -> PolicyPresetV2 {
    PolicyPresetV2 {
        id: "budget.daily_token_cap".into(),
        version: "1.0.0".into(),
        title: "Daily Token Cap".into(),
        short_description: "Cap daily input/output/total tokens.".into(),
        long_description: "Tracks agent token consumption across all providers and restricts usage once a daily threshold is reached.".into(),
        category: PresetCategory::CostAndTokens,
        risk_tags: vec![RiskTag::ModelDosCostSpike, RiskTag::FinancialRisk],
        supported_pep_types: vec![PepType::McpProxy, PepType::HttpGateway, PepType::LocalModelProxy],
        recommended_pep_types: vec![PepType::HttpGateway],
        supported_control_modes: vec![ControlMode::Observe, ControlMode::Warn, ControlMode::Enforce],
        default_control_mode: ControlMode::Enforce,
        supported_policy_outputs: vec![PolicyOutputKind::Rego, PolicyOutputKind::RouterRule],
        parameters: vec![
            PresetParameter {
                key: "max_daily_total_tokens".into(),
                label: "Max Daily Total Tokens".into(),
                description: "Maximum combined input/output tokens per day.".into(),
                value_type: PresetValueType::Integer,
                required: true,
                default_value: serde_json::json!(1000000),
                examples: vec![serde_json::json!(500000)],
            }
        ],
        generated_artifacts: vec![ArtifactKind::PolicyDraft, ArtifactKind::PepBinding],
        telemetry_requirements: vec![],
        default_simulation_window: SimulationWindow::Last30Days,
        safety_notes: vec![],
    }
}

pub fn budget_monthly_cost_cap() -> PolicyPresetV2 {
    PolicyPresetV2 {
        id: "budget.monthly_cost_cap".into(),
        version: "1.0.0".into(),
        title: "Monthly Cost Cap".into(),
        short_description: "Cap monthly spend by agent/provider/model.".into(),
        long_description: "Limits execution based on monetary constraints over a calendar month."
            .into(),
        category: PresetCategory::CostAndTokens,
        risk_tags: vec![RiskTag::FinancialRisk],
        supported_pep_types: vec![PepType::McpProxy, PepType::HttpGateway],
        recommended_pep_types: vec![PepType::McpProxy],
        supported_control_modes: vec![
            ControlMode::Observe,
            ControlMode::Warn,
            ControlMode::Enforce,
        ],
        default_control_mode: ControlMode::Enforce,
        supported_policy_outputs: vec![PolicyOutputKind::Rego, PolicyOutputKind::TelemetryRule],
        parameters: vec![PresetParameter {
            key: "max_monthly_usd".into(),
            label: "Max Monthly Limit (USD)".into(),
            description: "Maximum USD spend per month.".into(),
            value_type: PresetValueType::Float,
            required: true,
            default_value: serde_json::json!(10.0),
            examples: vec![serde_json::json!(50.0)],
        }],
        generated_artifacts: vec![
            ArtifactKind::PolicyDraft,
            ArtifactKind::PepBinding,
            ArtifactKind::TelemetrySubscription,
        ],
        telemetry_requirements: vec![],
        default_simulation_window: SimulationWindow::Last30Days,
        safety_notes: vec![],
    }
}

pub fn network_shadow_ai_external_llm_block() -> PolicyPresetV2 {
    PolicyPresetV2 {
        id: "network.shadow_ai_external_llm_block".into(),
        version: "1.0.0".into(),
        title: "Block Shadow AI External LLM".into(),
        short_description:
            "Block unregistered/shadow AI agents from reaching public LLM providers.".into(),
        long_description:
            "Stops unknown processes on the device from making API calls to external LLM providers."
                .into(),
        category: PresetCategory::NetworkAndProviders,
        risk_tags: vec![RiskTag::ShadowAi, RiskTag::DataExfiltration],
        supported_pep_types: vec![PepType::LinuxEbpf, PepType::HttpGateway],
        recommended_pep_types: vec![PepType::LinuxEbpf, PepType::HttpGateway],
        supported_control_modes: vec![
            ControlMode::Observe,
            ControlMode::Warn,
            ControlMode::Enforce,
            ControlMode::StrictDeny,
        ],
        default_control_mode: ControlMode::Observe,
        supported_policy_outputs: vec![PolicyOutputKind::Rego],
        parameters: vec![],
        generated_artifacts: vec![ArtifactKind::PolicyDraft, ArtifactKind::PepBinding],
        telemetry_requirements: vec![],
        default_simulation_window: SimulationWindow::Last7Days,
        safety_notes: vec![],
    }
}

pub fn mcp_high_risk_tool_approval() -> PolicyPresetV2 {
    PolicyPresetV2 {
        id: "mcp.high_risk_tool_approval".into(),
        version: "1.0.0".into(),
        title: "Require Approval for High-Risk Tools".into(),
        short_description: "Require approval for tools with side effects.".into(),
        long_description: "Pause action until a human user or admin explicitly approves the tool execution if the tool is designated as high or critical risk.".into(),
        category: PresetCategory::McpTools,
        risk_tags: vec![RiskTag::ExcessiveAgency],
        supported_pep_types: vec![PepType::McpProxy, PepType::StdioWrapper],
        recommended_pep_types: vec![PepType::McpProxy],
        supported_control_modes: vec![ControlMode::Approval, ControlMode::Enforce, ControlMode::StrictDeny],
        default_control_mode: ControlMode::Approval,
        supported_policy_outputs: vec![PolicyOutputKind::Cedar],
        parameters: vec![],
        generated_artifacts: vec![ArtifactKind::PolicyDraft, ArtifactKind::PepBinding],
        telemetry_requirements: vec![],
        default_simulation_window: SimulationWindow::Last7Days,
        safety_notes: vec![],
    }
}

pub fn mcp_tool_allowlist() -> PolicyPresetV2 {
    PolicyPresetV2 {
        id: "mcp.tool_allowlist".into(),
        version: "1.0.0".into(),
        title: "MCP Tool Allowlist".into(),
        short_description: "Allow only selected tools per agent/workspace.".into(),
        long_description: "Use OpenFGA relational policy to explicitly define which tools an agent or user is allowed to invoke.".into(),
        category: PresetCategory::McpTools,
        risk_tags: vec![RiskTag::UnauthorizedAccess],
        supported_pep_types: vec![PepType::McpProxy],
        recommended_pep_types: vec![PepType::McpProxy],
        supported_control_modes: vec![ControlMode::Observe, ControlMode::Warn, ControlMode::Enforce, ControlMode::StrictDeny],
        default_control_mode: ControlMode::Observe,
        supported_policy_outputs: vec![PolicyOutputKind::OpenFgaModel],
        parameters: vec![],
        generated_artifacts: vec![ArtifactKind::PolicyDraft, ArtifactKind::PepBinding],
        telemetry_requirements: vec![],
        default_simulation_window: SimulationWindow::Last7Days,
        safety_notes: vec![],
    }
}

pub fn personal_drive_folder_scope() -> PolicyPresetV2 {
    PolicyPresetV2 {
        id: "personal.drive_folder_scope".into(),
        version: "1.0.0".into(),
        title: "Drive Folder Scope Delegation".into(),
        short_description: "Restrict agents to specific Google Drive or local folders.".into(),
        long_description: "Delegates resource access based on folder relationships in OpenFGA."
            .into(),
        category: PresetCategory::FileSystem,
        risk_tags: vec![RiskTag::UnauthorizedAccess, RiskTag::DataExfiltration],
        supported_pep_types: vec![PepType::McpProxy],
        recommended_pep_types: vec![PepType::McpProxy],
        supported_control_modes: vec![
            ControlMode::Observe,
            ControlMode::Warn,
            ControlMode::Enforce,
            ControlMode::StrictDeny,
        ],
        default_control_mode: ControlMode::Observe,
        supported_policy_outputs: vec![PolicyOutputKind::OpenFgaModel],
        parameters: vec![],
        generated_artifacts: vec![ArtifactKind::PolicyDraft, ArtifactKind::PepBinding],
        telemetry_requirements: vec![],
        default_simulation_window: SimulationWindow::Last7Days,
        safety_notes: vec![],
    }
}
