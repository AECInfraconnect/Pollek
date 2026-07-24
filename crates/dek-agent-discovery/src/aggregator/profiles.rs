//! Per-agent-type semantic defaults and observation coverage: authority/
//! role/duplicate-policy parsers, default capability + risk semantics, and
//! the observation profile / coverage matrix keyed by inferred agent type.

use super::*;

pub(super) struct CandidateSemanticDefaults {
    pub(super) authority_boundary: AuthorityBoundary,
    pub(super) entity_role: EntityRole,
    pub(super) observe_scope: &'static str,
    pub(super) enforce_scope: &'static str,
}

pub(super) fn parse_authority_boundary(value: &str) -> AuthorityBoundary {
    match value {
        "local_device" => AuthorityBoundary::LocalDevice,
        "local_browser_profile" => AuthorityBoundary::LocalBrowserProfile,
        "local_container" => AuthorityBoundary::LocalContainer,
        "local_network" => AuthorityBoundary::LocalNetwork,
        "remote_cloud_sandbox" => AuthorityBoundary::RemoteCloudSandbox,
        "remote_workspace" => AuthorityBoundary::RemoteWorkspace,
        "remote_model_api" => AuthorityBoundary::RemoteModelApi,
        "mcp_remote_server" => AuthorityBoundary::McpRemoteServer,
        _ => AuthorityBoundary::Unknown,
    }
}

pub(super) fn parse_entity_role(value: &str) -> EntityRole {
    match value {
        "local_agent_host" => EntityRole::LocalAgentHost,
        "web_ai_surface" => EntityRole::WebAiSurface,
        "cloud_agent_runtime" => EntityRole::CloudAgentRuntime,
        "remote_workspace" => EntityRole::RemoteWorkspace,
        "model_api_endpoint" => EntityRole::ModelApiEndpoint,
        "mcp_tool_surface" => EntityRole::McpToolSurface,
        "browser_profile" => EntityRole::BrowserProfile,
        "generated_app_preview" => EntityRole::GeneratedAppPreview,
        "integration_endpoint" => EntityRole::IntegrationEndpoint,
        _ => EntityRole::Unknown,
    }
}

pub(super) fn parse_duplicate_policy(value: &str) -> DuplicatePolicy {
    match value {
        "child_surface" => DuplicatePolicy::ChildSurface,
        "related_endpoint" => DuplicatePolicy::RelatedEndpoint,
        "provider_endpoint" => DuplicatePolicy::ProviderEndpoint,
        "merged_duplicate" => DuplicatePolicy::MergedDuplicate,
        "needs_human_confirmation" => DuplicatePolicy::NeedsHumanConfirmation,
        _ => DuplicatePolicy::Standalone,
    }
}

pub(super) fn duplicate_policy_for_collapse(collapse_as: &str) -> DuplicatePolicy {
    match collapse_as {
        "child_surface" => DuplicatePolicy::ChildSurface,
        "remote_tool_surface" | "related_endpoint" => DuplicatePolicy::RelatedEndpoint,
        "provider_endpoint" => DuplicatePolicy::ProviderEndpoint,
        "merged_duplicate" => DuplicatePolicy::MergedDuplicate,
        _ => DuplicatePolicy::ChildSurface,
    }
}

pub(super) fn semantic_defaults_for_agent_type(
    agent_type: &InferredAgentType,
) -> CandidateSemanticDefaults {
    match agent_type {
        InferredAgentType::DesktopAgent
        | InferredAgentType::IdeAgent
        | InferredAgentType::CliAgent
        | InferredAgentType::AutomationAgent
        | InferredAgentType::CustomScriptAgent => CandidateSemanticDefaults {
            authority_boundary: AuthorityBoundary::LocalDevice,
            entity_role: EntityRole::LocalAgentHost,
            observe_scope: "local_process_file_network_tool_metadata",
            enforce_scope: "local_policy_pep_when_installed",
        },
        InferredAgentType::BrowserAgent | InferredAgentType::WebAIApp => {
            CandidateSemanticDefaults {
                authority_boundary: AuthorityBoundary::LocalBrowserProfile,
                entity_role: EntityRole::WebAiSurface,
                observe_scope: "browser_metadata_network_and_prompt_guard_extension",
                enforce_scope: "browser_extension_or_agent_settings",
            }
        }
        InferredAgentType::McpServer => CandidateSemanticDefaults {
            authority_boundary: AuthorityBoundary::LocalDevice,
            entity_role: EntityRole::McpToolSurface,
            observe_scope: "mcp_config_and_tool_call_metadata",
            enforce_scope: "mcp_wrapper_or_proxy_when_installed",
        },
        InferredAgentType::McpClient => CandidateSemanticDefaults {
            authority_boundary: AuthorityBoundary::LocalDevice,
            entity_role: EntityRole::LocalAgentHost,
            observe_scope: "local_process_and_mcp_client_metadata",
            enforce_scope: "mcp_client_config_or_agent_settings",
        },
        InferredAgentType::LocalModelServer => CandidateSemanticDefaults {
            authority_boundary: AuthorityBoundary::LocalNetwork,
            entity_role: EntityRole::ModelApiEndpoint,
            observe_scope: "local_model_endpoint_metadata",
            enforce_scope: "local_network_proxy_or_model_server_settings",
        },
        InferredAgentType::IdeExtension => CandidateSemanticDefaults {
            authority_boundary: AuthorityBoundary::LocalDevice,
            entity_role: EntityRole::IntegrationEndpoint,
            observe_scope: "ide_extension_metadata_and_declared_capabilities",
            enforce_scope: "ide_extension_settings_or_local_policy_pep",
        },
        InferredAgentType::UnknownAiProcess => CandidateSemanticDefaults {
            authority_boundary: AuthorityBoundary::Unknown,
            entity_role: EntityRole::Unknown,
            observe_scope: "metadata_only_until_confirmed",
            enforce_scope: "not_enforceable_until_confirmed",
        },
    }
}

/// Derives the observation profile (which signals to collect) tailored to the
/// agent type, so every discovered type is observed for what is actually
/// observable for it — rather than a single generic profile for all types.
///
/// `wants_token_usage` reflects whether the candidate's capability tags imply
/// LLM/model usage; it is only honored for types where token accounting is
/// meaningful (an MCP *server*, for instance, exposes tools but does not emit
/// tokens itself).
pub(super) fn observation_profile_for_agent_type(
    agent_type: &InferredAgentType,
    wants_token_usage: bool,
) -> ObservationProfile {
    let base = ObservationProfile {
        mode: ObservationMode::ObserveOnly,
        collect_process_metadata: true,
        collect_network_metadata: true,
        collect_mcp_tool_metadata: false,
        collect_token_usage: wants_token_usage,
        collect_file_metadata: false,
        collect_raw_prompt: false,
        collect_raw_response: false,
        retention_days: 30,
    };
    match agent_type {
        // Local host agents run on the device: they drive MCP tools and touch
        // local files, in addition to process/network/token signals.
        InferredAgentType::DesktopAgent
        | InferredAgentType::IdeAgent
        | InferredAgentType::CliAgent
        | InferredAgentType::AutomationAgent
        | InferredAgentType::CustomScriptAgent => ObservationProfile {
            collect_mcp_tool_metadata: true,
            collect_file_metadata: true,
            ..base
        },
        // An MCP server is the tool surface itself: tool-call metadata and the
        // resources it exposes matter; the server does not emit model tokens.
        InferredAgentType::McpServer => ObservationProfile {
            collect_mcp_tool_metadata: true,
            collect_file_metadata: true,
            collect_token_usage: false,
            ..base
        },
        // An MCP client both calls tools and (usually) drives a model.
        InferredAgentType::McpClient => ObservationProfile {
            collect_mcp_tool_metadata: true,
            collect_file_metadata: true,
            ..base
        },
        // A local model server is a network endpoint; token usage comes from
        // its responses, so keep the token flag and emphasize network metadata.
        InferredAgentType::LocalModelServer => ObservationProfile {
            collect_network_metadata: true,
            ..base
        },
        // Browser / web AI surfaces are observed through the browser and the
        // network (SNI) — there is no local process/file footprint to collect.
        InferredAgentType::BrowserAgent | InferredAgentType::WebAIApp => ObservationProfile {
            collect_process_metadata: false,
            collect_file_metadata: false,
            collect_network_metadata: true,
            ..base
        },
        // IDE extensions act inside the editor: tool and file metadata.
        InferredAgentType::IdeExtension => ObservationProfile {
            collect_mcp_tool_metadata: true,
            collect_file_metadata: true,
            ..base
        },
        // Unknown AI processes stay metadata-only (no token accounting) until a
        // user confirms what they are.
        InferredAgentType::UnknownAiProcess => ObservationProfile {
            collect_token_usage: false,
            ..base
        },
    }
}

/// Names the concrete method Pollek uses to retrieve token/cost accounting for
/// a given agent type. Different types report usage in different places, so the
/// method is type-specific — this is what LCP/Cloud surface as "how observed".
pub(super) fn token_usage_method_for(agent_type: &InferredAgentType) -> &'static str {
    match agent_type {
        // CLI / desktop / IDE coding agents persist usage in local session logs
        // (e.g. ~/.codex/sessions, ~/.claude) which the LCP bridge parses.
        InferredAgentType::CliAgent
        | InferredAgentType::DesktopAgent
        | InferredAgentType::IdeAgent
        | InferredAgentType::IdeExtension
        | InferredAgentType::AutomationAgent
        | InferredAgentType::CustomScriptAgent => "local_session_log + egress_llm_usage_parser",
        // Local model servers (Ollama, LM Studio, …) return usage inline in the
        // response body (prompt_eval_count/eval_count, usage.*).
        InferredAgentType::LocalModelServer => "egress_llm_usage_parser (response body)",
        // MCP clients drive a model; usage arrives via the provider-response
        // endpoint or the egress parser.
        InferredAgentType::McpClient => "provider_response_endpoint + egress_llm_usage_parser",
        // Browser / web AI surfaces have no local usage log; tokens are
        // estimated from observed request/response sizes.
        InferredAgentType::BrowserAgent | InferredAgentType::WebAIApp => "browser_network_estimate",
        // MCP servers and unconfirmed processes do not emit model tokens.
        InferredAgentType::McpServer | InferredAgentType::UnknownAiProcess => "not_applicable",
    }
}

/// Derives the per-signal observation coverage for a discovered agent from its
/// type and its suggested observation profile. A signal is `Active` when the
/// profile collects it, `Available` when Pollek could collect it but leaves it
/// off by default for this type, and `NotApplicable` when the signal is not
/// meaningful for the type. This is the displayable, type-aware answer to
/// "what can Pollek observe for this agent, and how".
pub(super) fn observation_coverage_for(
    agent_type: &InferredAgentType,
    profile: &ObservationProfile,
) -> Vec<ObservationSignalCoverage> {
    let is_web = matches!(
        agent_type,
        InferredAgentType::BrowserAgent | InferredAgentType::WebAIApp
    );
    let is_mcp_server = matches!(agent_type, InferredAgentType::McpServer);
    let is_unknown = matches!(agent_type, InferredAgentType::UnknownAiProcess);

    // Helper: pick Active if collected, else Available/NotApplicable.
    let status = |collected: bool, applicable: bool| {
        if collected {
            ObservationSignalStatus::Active
        } else if applicable {
            ObservationSignalStatus::Available
        } else {
            ObservationSignalStatus::NotApplicable
        }
    };

    let mut coverage = Vec::with_capacity(5);

    // Process metadata — meaningful for anything with a local process; web
    // surfaces have no local process footprint.
    coverage.push(ObservationSignalCoverage {
        signal: "process_metadata".into(),
        label: "Process activity".into(),
        status: status(profile.collect_process_metadata, !is_web),
        method: if is_web {
            "not_applicable".into()
        } else {
            "process_scan + ebpf_exec".into()
        },
    });

    // Network metadata — always meaningful (egress/SNI).
    coverage.push(ObservationSignalCoverage {
        signal: "network_metadata".into(),
        label: "Network egress".into(),
        status: status(profile.collect_network_metadata, true),
        method: "ebpf_egress + sni_inspection".into(),
    });

    // MCP tool metadata — meaningful for agents that speak MCP; web surfaces do
    // not expose local MCP tool calls.
    let mcp_applicable = !is_web && !is_unknown;
    coverage.push(ObservationSignalCoverage {
        signal: "mcp_tool_metadata".into(),
        label: "MCP tool calls".into(),
        status: status(profile.collect_mcp_tool_metadata, mcp_applicable),
        method: if mcp_applicable {
            "mcp_tool_call_metadata".into()
        } else {
            "not_applicable".into()
        },
    });

    // Token / cost usage — the retrieval method is type-specific.
    let token_applicable = !is_mcp_server && !is_unknown;
    coverage.push(ObservationSignalCoverage {
        signal: "token_usage".into(),
        label: "Token & cost usage".into(),
        status: status(profile.collect_token_usage, token_applicable),
        method: token_usage_method_for(agent_type).into(),
    });

    // File metadata — meaningful for local host agents; web surfaces have no
    // local file footprint.
    coverage.push(ObservationSignalCoverage {
        signal: "file_metadata".into(),
        label: "File access".into(),
        status: status(profile.collect_file_metadata, !is_web),
        method: if is_web {
            "not_applicable".into()
        } else {
            "ebpf_file_access".into()
        },
    });

    coverage
}
