use crate::model::*;
use std::collections::{BTreeMap, HashMap};

pub fn aggregate_evidence(
    tenant_id: &str,
    device_id: &str,
    evidence: Vec<DiscoveryEvidenceV2>,
) -> Vec<DiscoveredAgentCandidateV2> {
    let raw = aggregate_by_merge_key(tenant_id, device_id, evidence);
    apply_surface_grouping(coalesce_by_identity(tenant_id, raw))
}

struct CandidateSemanticDefaults {
    authority_boundary: AuthorityBoundary,
    entity_role: EntityRole,
    observe_scope: &'static str,
    enforce_scope: &'static str,
}

fn parse_authority_boundary(value: &str) -> AuthorityBoundary {
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

fn parse_entity_role(value: &str) -> EntityRole {
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

fn parse_duplicate_policy(value: &str) -> DuplicatePolicy {
    match value {
        "child_surface" => DuplicatePolicy::ChildSurface,
        "related_endpoint" => DuplicatePolicy::RelatedEndpoint,
        "provider_endpoint" => DuplicatePolicy::ProviderEndpoint,
        "merged_duplicate" => DuplicatePolicy::MergedDuplicate,
        "needs_human_confirmation" => DuplicatePolicy::NeedsHumanConfirmation,
        _ => DuplicatePolicy::Standalone,
    }
}

fn duplicate_policy_for_collapse(collapse_as: &str) -> DuplicatePolicy {
    match collapse_as {
        "child_surface" => DuplicatePolicy::ChildSurface,
        "remote_tool_surface" | "related_endpoint" => DuplicatePolicy::RelatedEndpoint,
        "provider_endpoint" => DuplicatePolicy::ProviderEndpoint,
        "merged_duplicate" => DuplicatePolicy::MergedDuplicate,
        _ => DuplicatePolicy::ChildSurface,
    }
}

fn semantic_defaults_for_agent_type(agent_type: &InferredAgentType) -> CandidateSemanticDefaults {
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
fn observation_profile_for_agent_type(
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
fn token_usage_method_for(agent_type: &InferredAgentType) -> &'static str {
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
fn observation_coverage_for(
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

fn service_slug(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if slug.is_empty() {
        "unknown".into()
    } else {
        slug
    }
}

fn default_canonical_service_id(
    agent_type: &InferredAgentType,
    matched_signature_id: Option<&str>,
    display_name: &str,
    process_hash: Option<&str>,
) -> String {
    if let Some(signature_id) = matched_signature_id {
        return signature_id.to_string();
    }
    let prefix = match agent_type {
        InferredAgentType::WebAIApp | InferredAgentType::BrowserAgent => "web_ai",
        InferredAgentType::McpServer | InferredAgentType::McpClient => "mcp",
        InferredAgentType::LocalModelServer => "local_model",
        InferredAgentType::IdeExtension => "ide_extension",
        InferredAgentType::UnknownAiProcess => "unknown_ai",
        _ => "local_agent",
    };
    let mut id = format!("{prefix}_{}", service_slug(display_name));
    if let Some(hash) = process_hash {
        id.push('_');
        id.push_str(&hash.chars().take(8).collect::<String>());
    }
    id
}

fn merge_candidate_semantics(
    existing: &mut DiscoveredAgentCandidateV2,
    incoming: &DiscoveredAgentCandidateV2,
) {
    if existing.authority_boundary == AuthorityBoundary::Unknown
        && incoming.authority_boundary != AuthorityBoundary::Unknown
    {
        existing.authority_boundary = incoming.authority_boundary.clone();
    }
    if existing.entity_role == EntityRole::Unknown && incoming.entity_role != EntityRole::Unknown {
        existing.entity_role = incoming.entity_role.clone();
    }
    if existing.duplicate_policy == DuplicatePolicy::Standalone
        && incoming.duplicate_policy != DuplicatePolicy::Standalone
    {
        existing.duplicate_policy = incoming.duplicate_policy.clone();
    }
    if existing.control_parent_id.is_none() {
        existing.control_parent_id = incoming.control_parent_id.clone();
    }
    if existing.grouping_reason.is_none() {
        existing.grouping_reason = incoming.grouping_reason.clone();
    }
    if existing.observe_scope == "metadata_only_until_confirmed" {
        existing.observe_scope = incoming.observe_scope.clone();
    }
    if existing.enforce_scope == "not_enforceable_until_confirmed" {
        existing.enforce_scope = incoming.enforce_scope.clone();
    }
    for surface in &incoming.related_surfaces {
        if !existing
            .related_surfaces
            .iter()
            .any(|existing_surface| existing_surface.service_id == surface.service_id)
        {
            existing.related_surfaces.push(surface.clone());
        }
    }
}

fn coalesce_by_identity(
    tenant: &str,
    raw: Vec<DiscoveredAgentCandidateV2>,
) -> Vec<DiscoveredAgentCandidateV2> {
    use std::collections::HashMap;
    let mut by_key: HashMap<String, DiscoveredAgentCandidateV2> = HashMap::new();

    for mut c in raw {
        let key = candidate_identity_bucket(&c);
        c.candidate_id = crate::identity_key::deterministic_candidate_id(tenant, &key);
        // Also update the target_candidate_id in suggested control bindings
        for cb in &mut c.suggested_control_bindings {
            cb.target_candidate_id = c.candidate_id.clone();
        }

        match by_key.get_mut(&key) {
            Some(existing) => {
                existing.evidence.extend(std::mem::take(&mut c.evidence));
                existing.confidence = existing.confidence.max(c.confidence);
                existing.risk_score = existing.risk_score.max(c.risk_score);
                existing.instance_count = existing.instance_count.saturating_add(1);
                merge_candidate_semantics(existing, &c);

                for _cap in c
                    .suggested_registration
                    .declared_tools
                    .iter()
                    .chain(c.labels.keys())
                {
                    // Not strictly capabilities but labels could be merged.
                    // We'll merge labels.
                    for (k, v) in c.labels.iter() {
                        if !existing.labels.contains_key(k) {
                            existing.labels.insert(k.clone(), v.clone());
                        }
                    }
                }

                if is_better_name(&c.display_name, &existing.display_name) {
                    existing.display_name = c.display_name;
                    existing.vendor = c.vendor.or(existing.vendor.take());
                    existing.product = c.product.or(existing.product.take());
                    existing.inferred_agent_type = c.inferred_agent_type;
                }

                existing.first_seen = std::cmp::min(existing.first_seen.clone(), c.first_seen);
                existing.last_seen = std::cmp::max(existing.last_seen.clone(), c.last_seen);
            }
            None => {
                c.instance_count = 1;
                by_key.insert(key, c);
            }
        }
    }
    by_key.into_values().collect()
}

/// Extracts the port from a probe endpoint URL like `http://127.0.0.1:11434`.
fn port_from_url(url: &str) -> Option<u16> {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host_port = rest.split('/').next().unwrap_or(rest);
    host_port.rsplit_once(':')?.1.parse::<u16>().ok()
}

/// The catalog carries a few near-duplicate signature ids for the same real
/// product (e.g. `openclaw` matched from the gateway process vs
/// `openclaw_agent` matched from an installed binary). Normalize them to one
/// canonical id for identity bucketing so a single agent never shows up as
/// several duplicate candidates.
fn canonical_signature_alias(id: &str) -> &str {
    match id {
        "openclaw_agent" => "openclaw",
        "hiclaw_agent" => "hiclaw",
        "cursor_desktop" | "cursor_app" => "cursor",
        "claude_desktop_app" => "claude_desktop",
        "opencode_cli" => "opencode",
        "aider_cli" => "aider",
        "goose_cli" => "goose",
        "open_interpreter_cli" => "open_interpreter",
        "antigravity_desktop" => "antigravity_cli",
        other => other,
    }
}

fn candidate_identity_bucket(candidate: &DiscoveredAgentCandidateV2) -> String {
    if matches!(
        candidate.authority_boundary,
        AuthorityBoundary::LocalBrowserProfile
    ) || matches!(candidate.inferred_agent_type, InferredAgentType::WebAIApp)
    {
        if let Some(merge_key) = candidate
            .evidence
            .iter()
            .find_map(|ev| ev.merge_key.as_deref())
        {
            return merge_key.to_string();
        }
    }

    crate::identity_key::identity_key(
        candidate
            .matched_signature_id
            .as_deref()
            .map(canonical_signature_alias),
        candidate.vendor.as_deref(),
        candidate.product.as_deref(),
        candidate
            .suggested_registration
            .process_path_hash
            .as_deref(),
        &candidate.display_name,
    )
}

fn apply_surface_grouping(
    mut candidates: Vec<DiscoveredAgentCandidateV2>,
) -> Vec<DiscoveredAgentCandidateV2> {
    let baseline = dek_fingerprint_defs::load_latest_baseline();
    let mut actions: Vec<(usize, usize, String, DuplicatePolicy, bool)> = Vec::new();

    for rule in &baseline.collapse_rules {
        let parent_indices = if let Some(parent_sig) = &rule.when_parent_signature_id {
            candidates
                .iter()
                .enumerate()
                .filter_map(|(idx, candidate)| {
                    (candidate.matched_signature_id.as_deref() == Some(parent_sig.as_str()))
                        .then_some(idx)
                })
                .collect::<Vec<_>>()
        } else {
            candidates
                .iter()
                .enumerate()
                .filter_map(|(idx, candidate)| {
                    let signature_match = candidate
                        .matched_signature_id
                        .as_deref()
                        .is_some_and(|sig| rule.parent_client_candidates.iter().any(|p| p == sig));
                    let canonical_match = rule
                        .parent_client_candidates
                        .iter()
                        .any(|p| p == &candidate.canonical_service_id);
                    (signature_match || canonical_match).then_some(idx)
                })
                .collect::<Vec<_>>()
        };

        if parent_indices.is_empty() {
            continue;
        }

        let child_indices = candidates
            .iter()
            .enumerate()
            .filter_map(|(idx, candidate)| {
                rule.child_service_ids
                    .iter()
                    .any(|child| child == &candidate.canonical_service_id)
                    .then_some(idx)
            })
            .collect::<Vec<_>>();

        for parent_idx in parent_indices {
            for child_idx in &child_indices {
                if parent_idx == *child_idx {
                    continue;
                }
                actions.push((
                    parent_idx,
                    *child_idx,
                    rule.id.clone(),
                    duplicate_policy_for_collapse(&rule.collapse_as),
                    rule.control_parent_only,
                ));
            }
        }
    }

    for (parent_idx, child_idx, rule_id, duplicate_policy, control_parent_only) in actions {
        if parent_idx >= candidates.len() || child_idx >= candidates.len() {
            continue;
        }

        let parent_id = candidates[parent_idx].candidate_id.clone();
        let parent_name = candidates[parent_idx].display_name.clone();
        let evidence_sources = {
            let mut sources = candidates[child_idx]
                .evidence
                .iter()
                .map(|ev| ev.source.clone())
                .collect::<Vec<_>>();
            sources.sort_by_key(|source| format!("{:?}", source));
            sources.dedup();
            sources
        };
        let surface = RelatedSurfaceRef {
            service_id: candidates[child_idx].canonical_service_id.clone(),
            display_name: candidates[child_idx].display_name.clone(),
            entity_role: candidates[child_idx].entity_role.clone(),
            authority_boundary: candidates[child_idx].authority_boundary.clone(),
            evidence_sources,
            confidence: candidates[child_idx].confidence,
            control_parent_id: Some(parent_id.clone()),
            grouping_reason: Some(rule_id.clone()),
        };

        {
            let child = &mut candidates[child_idx];
            child.duplicate_policy = duplicate_policy.clone();
            child.control_parent_id = Some(parent_id.clone());
            child.grouping_reason =
                Some(format!("{}; controlled through {}", rule_id, parent_name));
            if control_parent_only {
                child.suggested_control_bindings.clear();
            }
            child.labels.insert("control_parent_id".into(), parent_id);
            child
                .labels
                .insert("grouping_rule_id".into(), rule_id.clone());
            child.labels.insert(
                "duplicate_policy".into(),
                format!("{:?}", child.duplicate_policy),
            );
        }

        let parent = &mut candidates[parent_idx];
        if !parent
            .related_surfaces
            .iter()
            .any(|existing| existing.service_id == surface.service_id)
        {
            parent.related_surfaces.push(surface);
        }
    }

    candidates
}

fn is_better_name(new: &str, old: &str) -> bool {
    let bad = |s: &str| {
        s == "Unknown Agent"
            || s.starts_with("Possible AI Agent")
            || s.contains("unconfirmed")
            || basename_no_ext(s) == s && s.len() > 15
    };
    bad(old) && !bad(new)
}

fn aggregate_by_merge_key(
    tenant_id: &str,
    device_id: &str,
    mut evidence: Vec<DiscoveryEvidenceV2>,
) -> Vec<DiscoveredAgentCandidateV2> {
    // Group evidence by merge_key
    let mut groups: HashMap<String, Vec<DiscoveryEvidenceV2>> = HashMap::new();

    for ev in evidence.drain(..) {
        let key = ev
            .merge_key
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        groups.entry(key).or_default().push(ev);
    }

    let mut candidates = Vec::new();

    for (_key, group) in groups {
        let mut max_confidence = 0.0;
        let mut risk_score = 0;
        let mut agent_type = InferredAgentType::UnknownAiProcess;
        let mut name = "Unknown Agent".to_string();
        let mut vendor = None;
        let mut product = None;
        let mut matched_signature_id: Option<String> = None;
        let mut capability_tags = Vec::new();
        let mut matched_signals = Vec::new();
        let mut status = DiscoveryStatus::Discovered;
        let mut canonical_service_id: Option<String> = None;
        let mut surface_group_id: Option<String> = None;
        let mut authority_boundary: Option<AuthorityBoundary> = None;
        let mut entity_role: Option<EntityRole> = None;
        let mut duplicate_policy: Option<DuplicatePolicy> = None;
        let mut control_parent_id: Option<String> = None;
        let mut grouping_reason: Option<String> = None;
        let mut observe_scope: Option<String> = None;
        let mut enforce_scope: Option<String> = None;
        let related_surfaces: Vec<RelatedSurfaceRef> = Vec::new();
        let mut evidence_requires_human_confirmation = false;

        let has_confirmed = group.iter().any(|e| {
            e.data
                .get("confirmed")
                .and_then(|v| v.as_bool())
                .unwrap_or(true)
        });
        if !has_confirmed {
            status = DiscoveryStatus::Unconfirmed;
        }

        let mut process_hash = None;
        let mut mcp_servers = Vec::new();
        let mut endpoints = Vec::new();
        let mut redacted_env_keys = Vec::new();
        let mut local_model_provider: Option<String> = None;

        let mut ctx = crate::identity::ResolutionContext::default();
        let mut best_hint = crate::identity_hint::IdentityHint::default();

        for ev in &group {
            if let Some(hint) = crate::identity_hint::extract_identity_hint(ev) {
                if hint.confidence >= best_hint.confidence {
                    best_hint = hint;
                }
            }

            if ev.confidence > max_confidence {
                max_confidence = ev.confidence;
            }

            match ev.source {
                EvidenceSource::ProcessScan => {
                    if let Some(r_name) = ev.data.get("resolved_name").and_then(|v| v.as_str()) {
                        name = r_name.to_string();
                    }
                    if let Some(r_vendor) = ev.data.get("vendor").and_then(|v| v.as_str()) {
                        vendor = Some(r_vendor.to_string());
                    }
                    if let Some(sig_id) =
                        ev.data.get("matched_signature_id").and_then(|v| v.as_str())
                    {
                        matched_signature_id = Some(sig_id.to_string());
                    }

                    let process_data = ev.data.get("process").unwrap_or(&ev.data);
                    if let Ok(p) = serde_json::from_value::<crate::process_scan::ProcessEvidence>(
                        process_data.clone(),
                    ) {
                        ctx.process_name = p.process_name.clone();
                        ctx.cmd_redacted = p.cmd_template.join(" ");
                        ctx.exe_path_norm = p.exe_path_redacted.clone();
                        ctx.binary_hash = p.exe_path_hash.clone();
                        process_hash = p.exe_path_hash.clone();

                        if let Some(exe) = &p.exe_path_redacted {
                            ctx.cli_on_path.push(basename_no_ext(exe));
                        }
                        if let Some(pkg) = npm_pkg_from_argv(&p.cmd_template) {
                            ctx.packages.push(("npm".into(), pkg.clone()));
                            ctx.cli_on_path.push(pkg);
                        }
                    }
                }
                EvidenceSource::McpConfig => {
                    if let Some(path) = &ev.source_path_redacted {
                        ctx.present_paths.push(path.clone());
                    }
                    if let Some(transport) = ev.data.get("transport").and_then(|v| v.as_str()) {
                        let server_name = ev
                            .data
                            .get("server_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let command = ev
                            .data
                            .get("command_template")
                            .and_then(|v| v.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        mcp_servers.push(DiscoveredMcpServerRef {
                            server_name: server_name.clone(),
                            transport: transport.to_string(),
                            command,
                        });

                        if let Some(env_keys) =
                            ev.data.get("env_key_names").and_then(|v| v.as_array())
                        {
                            for key in env_keys {
                                if let Some(k) = key.as_str() {
                                    ctx.env_present.push(k.to_string());
                                    if !redacted_env_keys.contains(&k.to_string()) {
                                        redacted_env_keys.push(k.to_string());
                                    }
                                }
                            }
                        }
                    } else if let Some(data) = ev.data.get("servers") {
                        if let Some(obj) = data.as_object() {
                            for (k, v) in obj {
                                mcp_servers.push(DiscoveredMcpServerRef {
                                    server_name: k.to_string(),
                                    transport: "stdio".into(),
                                    command: v
                                        .get("command")
                                        .and_then(|c| c.as_str())
                                        .map(|s| s.to_string()),
                                });
                            }
                        }
                    }
                }
                EvidenceSource::LocalModelServer => {
                    if let Some(key_url) = &ev.merge_key {
                        endpoints.push(DiscoveredEndpointRef {
                            url: key_url.clone(),
                            protocol: "http".into(),
                        });
                    }
                    if let Some(provider) = ev.data.get("provider").and_then(|v| v.as_str()) {
                        local_model_provider = Some(provider.to_string());
                    }
                    // Real listening port: explicit field first, otherwise the
                    // probed endpoint URL (previously this fell back to 80 and
                    // broke port-based signature attribution → duplicates).
                    if let Some(port) = ev.data.get("port").and_then(|v| v.as_u64()) {
                        ctx.listening_ports.push(port as u16);
                    } else if let Some(port) = ev
                        .data
                        .get("endpoint")
                        .and_then(|v| v.as_str())
                        .or(ev.merge_key.as_deref())
                        .and_then(port_from_url)
                    {
                        ctx.listening_ports.push(port);
                    } else {
                        ctx.listening_ports.push(80);
                    }

                    if let Some(models_val) = ev.data.get("models") {
                        if let Some(arr) = models_val.as_array() {
                            if let Some(clf_def) =
                                &dek_fingerprint_defs::load_latest_baseline().model_classifier
                            {
                                let clf =
                                    dek_fingerprint_defs::model_classifier::ModelClassifier::new(
                                        clf_def,
                                    );
                                for v in arr {
                                    if let Some(m_name) = v.as_str() {
                                        let mc = clf.classify(m_name);
                                        for cap in mc.capability_tags {
                                            if !capability_tags.contains(&cap) {
                                                capability_tags.push(cap);
                                            }
                                        }
                                        let r = (mc.risk_score * 100.0) as u32;
                                        if r > risk_score {
                                            risk_score = r;
                                        }
                                        if mc.needs_human {
                                            status = DiscoveryStatus::PendingApproval;
                                        }
                                    }
                                }
                            }
                            if !capability_tags.contains(&"model.server".to_string()) {
                                capability_tags.push("model.server".to_string());
                            }
                        }
                    }
                }
                EvidenceSource::PortProbe => {
                    let endpoint_url = ev
                        .data
                        .get("endpoint")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                        .or_else(|| ev.source_path_redacted.clone());
                    if let Some(url) = endpoint_url {
                        let transport = ev
                            .data
                            .get("transport")
                            .and_then(|v| v.as_str())
                            .unwrap_or("sse")
                            .to_string();
                        endpoints.push(DiscoveredEndpointRef {
                            url,
                            protocol: transport.clone(),
                        });

                        let server_name = ev
                            .data
                            .get("mcp")
                            .and_then(|m| m.get("server_name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("sse_server")
                            .to_string();
                        mcp_servers.push(DiscoveredMcpServerRef {
                            server_name,
                            transport,
                            command: None,
                        });
                    }
                    if let Some(port) = ev.data.get("port").and_then(|v| v.as_u64()) {
                        ctx.listening_ports.push(port as u16);
                    } else if let Some(port) = ev
                        .data
                        .get("endpoint")
                        .and_then(|v| v.as_str())
                        .or(ev.source_path_redacted.as_deref())
                        .and_then(port_from_url)
                    {
                        ctx.listening_ports.push(port);
                    } else {
                        ctx.listening_ports.push(80);
                    }
                }
                EvidenceSource::IdeExtension => {
                    // Not fully utilizing this signal yet in identity.rs
                }
                EvidenceSource::InstalledAppScan => {
                    if let Some(evidence_name) = ev.data.get("name").and_then(|v| v.as_str()) {
                        name = evidence_name.to_string();
                        vendor = ev
                            .data
                            .get("vendor")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        product = ev
                            .data
                            .get("product")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        agent_type = ev
                            .data
                            .get("agent_type")
                            .and_then(|v| v.as_str())
                            .map(|agent_type| match agent_type {
                                "desktop_agent" => InferredAgentType::DesktopAgent,
                                "ide_agent" => InferredAgentType::IdeAgent,
                                "cli_agent" => InferredAgentType::CliAgent,
                                "local_model_server" | "local_model" => {
                                    InferredAgentType::LocalModelServer
                                }
                                _ => InferredAgentType::DesktopAgent,
                            })
                            .unwrap_or(InferredAgentType::DesktopAgent);
                        if let Some(caps) =
                            ev.data.get("capability_tags").and_then(|v| v.as_array())
                        {
                            for cap in caps.iter().filter_map(|v| v.as_str()) {
                                let cap = cap.to_string();
                                if !capability_tags.contains(&cap) {
                                    capability_tags.push(cap);
                                }
                            }
                        }
                    }
                    if let Some(path) = ev.data.get("path").and_then(|v| v.as_str()) {
                        let sigs =
                            &dek_fingerprint_defs::load_latest_baseline().installed_app_signatures;
                        if let Some(am) = crate::identity::resolve_installed_app(path, sigs) {
                            name = am.display_name.clone();
                            vendor = am.vendor.clone();
                            product = am.product.clone();
                            agent_type = match am.agent_type.as_str() {
                                "desktop_agent" => InferredAgentType::DesktopAgent,
                                "ide_agent" => InferredAgentType::IdeAgent,
                                _ => InferredAgentType::DesktopAgent,
                            };
                            for cap in &am.capability_tags {
                                if !capability_tags.contains(cap) {
                                    capability_tags.push(cap.clone());
                                }
                            }
                        } else {
                            name = path.to_string();
                            agent_type = InferredAgentType::DesktopAgent;
                        }
                    }
                }
                EvidenceSource::BrowserSession
                | EvidenceSource::BrowserWindow
                | EvidenceSource::BrowserHistory
                | EvidenceSource::NetworkSni => {
                    let evidence_name = ev.data.get("name").and_then(|v| v.as_str());
                    if let Some(evidence_name) = evidence_name {
                        name = evidence_name.to_string();
                        vendor = ev
                            .data
                            .get("vendor")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        agent_type = InferredAgentType::WebAIApp;
                        if let Some(caps) =
                            ev.data.get("capability_tags").and_then(|v| v.as_array())
                        {
                            for cap in caps.iter().filter_map(|v| v.as_str()) {
                                let cap = cap.to_string();
                                if !capability_tags.contains(&cap) {
                                    capability_tags.push(cap);
                                }
                            }
                        }
                        if let Some(sig_id) =
                            ev.data.get("matched_signature_id").and_then(|v| v.as_str())
                        {
                            matched_signature_id = Some(sig_id.to_string());
                        }
                        if let Some(value) =
                            ev.data.get("canonical_service_id").and_then(|v| v.as_str())
                        {
                            canonical_service_id = Some(value.to_string());
                        }
                        if let Some(value) =
                            ev.data.get("surface_group_id").and_then(|v| v.as_str())
                        {
                            surface_group_id = Some(value.to_string());
                        }
                        if let Some(value) =
                            ev.data.get("authority_boundary").and_then(|v| v.as_str())
                        {
                            authority_boundary = Some(parse_authority_boundary(value));
                        }
                        if let Some(value) = ev.data.get("entity_role").and_then(|v| v.as_str()) {
                            entity_role = Some(parse_entity_role(value));
                        }
                        if let Some(value) =
                            ev.data.get("duplicate_policy").and_then(|v| v.as_str())
                        {
                            duplicate_policy = Some(parse_duplicate_policy(value));
                        }
                        if let Some(value) =
                            ev.data.get("control_parent_id").and_then(|v| v.as_str())
                        {
                            control_parent_id = Some(value.to_string());
                        }
                        if let Some(value) = ev.data.get("grouping_reason").and_then(|v| v.as_str())
                        {
                            grouping_reason = Some(value.to_string());
                        }
                        if let Some(value) = ev.data.get("observe_scope").and_then(|v| v.as_str()) {
                            observe_scope = Some(value.to_string());
                        }
                        if let Some(value) = ev.data.get("enforce_scope").and_then(|v| v.as_str()) {
                            enforce_scope = Some(value.to_string());
                        }
                        if matches!(ev.source, EvidenceSource::NetworkSni)
                            || ev
                                .data
                                .get("evidence_strength")
                                .and_then(|v| v.as_str())
                                .is_some_and(|v| v == "network_sni_only")
                        {
                            evidence_requires_human_confirmation = true;
                        }
                        let detail = ev
                            .data
                            .get("matched_domain")
                            .and_then(|v| v.as_str())
                            .or_else(|| ev.data.get("domain").and_then(|v| v.as_str()))
                            .or_else(|| ev.data.get("origin").and_then(|v| v.as_str()))
                            .unwrap_or("web_ai");
                        matched_signals.push(MatchedSignal {
                            kind: format!("{:?}", ev.source),
                            detail: detail.to_string(),
                            weight: ev.confidence,
                        });
                    }
                    if let Some(url) = ev
                        .data
                        .get("url")
                        .and_then(|v| v.as_str())
                        .or_else(|| ev.data.get("sni").and_then(|v| v.as_str()))
                        .or_else(|| ev.data.get("origin").and_then(|v| v.as_str()))
                    {
                        let sigs = &dek_fingerprint_defs::load_latest_baseline().web_ai_signatures;
                        let mut found = false;
                        for w_sig in sigs {
                            if url.contains(&w_sig.domain) {
                                if evidence_name.is_none() {
                                    let browser_name = ev
                                        .data
                                        .get("browser_name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("Browser");
                                    name = crate::browser_window_scan::browser_scoped_ai_name(
                                        &w_sig.name,
                                        browser_name,
                                    );
                                }
                                if vendor.is_none() {
                                    vendor = Some(w_sig.vendor.clone());
                                }
                                for cap in &w_sig.capability_tags {
                                    if !capability_tags.contains(cap) {
                                        capability_tags.push(cap.clone());
                                    }
                                }
                                agent_type = InferredAgentType::WebAIApp;
                                matched_signals.push(MatchedSignal {
                                    kind: format!("{:?}", ev.source),
                                    detail: w_sig.domain.clone(),
                                    weight: ev.confidence,
                                });
                                if matched_signature_id.is_none() {
                                    matched_signature_id = Some(w_sig.id.clone());
                                }
                                canonical_service_id
                                    .get_or_insert_with(|| w_sig.canonical_service_id.clone());
                                surface_group_id
                                    .get_or_insert_with(|| w_sig.surface_group_id.clone());
                                authority_boundary.get_or_insert_with(|| {
                                    parse_authority_boundary(&w_sig.authority_boundary)
                                });
                                entity_role
                                    .get_or_insert_with(|| parse_entity_role(&w_sig.entity_role));
                                observe_scope.get_or_insert_with(|| w_sig.observe_scope.clone());
                                enforce_scope.get_or_insert_with(|| w_sig.enforce_scope.clone());
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            if evidence_name.is_none() {
                                name = url.to_string();
                            }
                            agent_type = InferredAgentType::WebAIApp;
                        }
                    }
                }
                _ => {}
            }
        }

        let baseline = dek_fingerprint_defs::load_latest_baseline();
        let signatures = baseline.signatures.clone();
        let mut decision = crate::identity::resolve(&ctx, &signatures);

        if decision.best.is_none() {
            if let Some(ref exe) = ctx.exe_path_norm {
                if let Some(id) = crate::fingerprint::resolve_by_install_path(exe, &baseline) {
                    decision.best = Some(id);
                    decision.needs_human = false;
                }
            }
        }

        // New signature match logic using process_names, cmd_patterns, etc.
        if decision.best.is_none() {
            let facts = crate::signature_match::ProcessFacts {
                process_name: &ctx.process_name,
                exe_path: ctx.exe_path_norm.as_deref().unwrap_or(""),
                cmdline: &ctx.cmd_redacted,
                installed_paths: &ctx.present_paths,
            };
            if let Some(am) = crate::signature_match::match_process(
                &facts,
                &signatures,
                &baseline.installed_app_signatures,
            ) {
                decision.best = Some(crate::identity::AgentMatch {
                    signature_id: am.id,
                    display_name: am.display_name,
                    vendor: am.vendor,
                    product: None,
                    agent_type: am.agent_type,
                    confidence: am.confidence,
                    capability_tags: am.capability_tags,
                    matched_signals: vec![crate::identity::MatchedSignal {
                        kind: am.matched_by.to_string(),
                        detail: "signature_match".into(),
                        weight: am.confidence,
                    }],
                });
                decision.needs_human = false;
            }
        }

        // If unknown, run claw family heuristic
        if decision.best.is_none() {
            if let Some(claw_match) = crate::identity::claw_family_heuristic(&ctx) {
                decision.best = Some(claw_match);
                decision.needs_human = true;
            }
        }

        let resolved_by_signature = decision.best.is_some();

        if let Some(best) = decision.best {
            matched_signature_id = Some(best.signature_id.clone());
            name = best.display_name.clone();
            vendor = best.vendor.clone();
            product = best.product.clone();
            matched_signals = best
                .matched_signals
                .iter()
                .map(|s| MatchedSignal {
                    kind: s.kind.clone(),
                    detail: s.detail.clone(),
                    weight: s.weight,
                })
                .collect();
            // Map agent_type string to enum
            agent_type = match best.agent_type.as_str() {
                "desktop_agent" => InferredAgentType::DesktopAgent,
                "ide" | "ide_agent" => InferredAgentType::IdeAgent,
                "cli_agent" => InferredAgentType::CliAgent,
                "browser_agent" => InferredAgentType::BrowserAgent,
                "web_ai" | "web_agent" | "chat_ui" => InferredAgentType::WebAIApp,
                "local_model" | "local_model_server" => InferredAgentType::LocalModelServer,
                "automation_agent" => InferredAgentType::AutomationAgent,
                "mcp_server" => InferredAgentType::McpServer,
                "mcp_client" => InferredAgentType::McpClient,
                _ => InferredAgentType::UnknownAiProcess,
            };
            max_confidence = f64::max(max_confidence, best.confidence);
            for cap in best.capability_tags {
                if !capability_tags.contains(&cap) {
                    capability_tags.push(cap);
                }
            }
        }

        // A local-model port probe labels its evidence with the engine's
        // signature id (ollama, vllm, lmstudio, sglang, …). Attribute the
        // candidate to that signature so it shares an identity bucket with the
        // process-scan candidate for the same engine — one agent, not two.
        if matched_signature_id.is_none() {
            if let Some(provider) = &local_model_provider {
                let provider_slug = provider.to_lowercase();
                if let Some(sig) = signatures.iter().find(|s| s.id == provider_slug) {
                    matched_signature_id = Some(sig.id.clone());
                    name = sig.display_name.clone();
                    if vendor.is_none() {
                        vendor = sig.vendor.clone();
                    }
                    if product.is_none() {
                        product = sig.product.clone();
                    }
                    agent_type = InferredAgentType::LocalModelServer;
                    max_confidence = f64::max(max_confidence, 0.9);
                    for cap in &sig.capability_tags {
                        if !capability_tags.contains(cap) {
                            capability_tags.push(cap.clone());
                        }
                    }
                    matched_signals.push(MatchedSignal {
                        kind: "local_model_provider".into(),
                        detail: sig.id.clone(),
                        weight: 0.9,
                    });
                }
            }
        }

        if name == "Unknown Agent" && !ctx.process_name.is_empty() {
            name = format!("Possible AI Agent ({})", ctx.process_name);
        }

        if !resolved_by_signature || best_hint.confidence >= 1.0 {
            let hint_is_web_ai = matches!(
                best_hint.agent_type.as_ref(),
                Some(InferredAgentType::WebAIApp)
            );
            if let Some(n) = best_hint
                .name
                .filter(|n| !n.is_empty() && n != "Unknown Agent")
            {
                let n_lower = n.to_lowercase();
                let signature_hint = if hint_is_web_ai {
                    None
                } else {
                    signatures.iter().find(|s| {
                        s.display_name.to_lowercase() == n_lower
                            || s.process_names.iter().any(|pn| {
                                let pn_lower = pn.to_lowercase();
                                let generic = [
                                    "node",
                                    "node.exe",
                                    "python",
                                    "python.exe",
                                    "chrome",
                                    "msedge",
                                    "firefox",
                                    "safari",
                                    "brave",
                                    "code",
                                    "chat",
                                ];
                                !generic.contains(&pn_lower.as_str()) && n_lower.contains(&pn_lower)
                            })
                            || s.id.to_lowercase().replace("_", " ") == n_lower
                    })
                };

                if let Some(sig) = signature_hint {
                    name = sig.display_name.clone();
                    vendor = sig.vendor.clone();
                    product = sig.product.clone();
                    agent_type = match sig.agent_type.as_str() {
                        "desktop_agent" => InferredAgentType::DesktopAgent,
                        "ide_agent" => InferredAgentType::IdeAgent,
                        "cli_agent" => InferredAgentType::CliAgent,
                        "browser_agent" => InferredAgentType::BrowserAgent,
                        "mcp_server" => InferredAgentType::McpServer,
                        "mcp_client" => InferredAgentType::McpClient,
                        _ => InferredAgentType::AutomationAgent,
                    };
                    max_confidence = f64::max(max_confidence, f64::max(0.8, best_hint.confidence));
                    for cap in &sig.capability_tags {
                        if !capability_tags.contains(cap) {
                            capability_tags.push(cap.clone());
                        }
                    }
                    matched_signals.push(MatchedSignal {
                        kind: "identity_hint_signature".into(),
                        detail: sig.id.clone(),
                        weight: best_hint.confidence,
                    });
                    matched_signature_id = Some(sig.id.clone());
                } else {
                    name = n;
                    if vendor.is_none() {
                        vendor = best_hint.vendor;
                    }
                    if product.is_none() {
                        product = best_hint.product;
                    }
                    if let Some(t) = best_hint.agent_type {
                        agent_type = t;
                    }
                    max_confidence = f64::max(max_confidence, best_hint.confidence);
                    for cap in best_hint.capability_tags {
                        if !capability_tags.contains(&cap) {
                            capability_tags.push(cap);
                        }
                    }
                    matched_signals.push(MatchedSignal {
                        kind: "identity_hint".into(),
                        detail: "non-signature discovery hint".into(),
                        weight: best_hint.confidence,
                    });
                }
            }
        }

        let mut computed_risk = 0;
        for cap in &capability_tags {
            let cap_score = match cap.as_str() {
                "browser.control" => 60,
                "automation" => 40,
                "fs.write" => 50,
                "code.exec" => 80,
                "llm.call" => 20,
                "web.search" => 10,
                "net.egress" => 30,
                "net.egress.llm" => 30,
                "tool.use" => 30,
                "model.server" => 40,
                "code.agentic" => 70,
                "web.chat" => 20,
                _ => 10,
            };
            computed_risk += cap_score;
        }
        if computed_risk > risk_score {
            risk_score = std::cmp::min(100, computed_risk);
        }

        if name == "Unknown Agent" || name.starts_with("Possible AI Agent") {
            status = DiscoveryStatus::PendingApproval;
        }

        if decision.needs_human {
            status = DiscoveryStatus::PendingApproval;
        }
        if evidence_requires_human_confirmation {
            status = DiscoveryStatus::PendingApproval;
            duplicate_policy = Some(DuplicatePolicy::NeedsHumanConfirmation);
            grouping_reason.get_or_insert_with(|| {
                "Network-only evidence must be confirmed before Pollek treats this as a controllable AI app.".into()
            });
        }

        let semantic_defaults = semantic_defaults_for_agent_type(&agent_type);
        let canonical_service_id = canonical_service_id.unwrap_or_else(|| {
            default_canonical_service_id(
                &agent_type,
                matched_signature_id.as_deref(),
                &name,
                process_hash.as_deref(),
            )
        });
        let surface_group_id = surface_group_id.unwrap_or_else(|| canonical_service_id.clone());
        let authority_boundary =
            authority_boundary.unwrap_or_else(|| semantic_defaults.authority_boundary.clone());
        let entity_role = entity_role.unwrap_or_else(|| semantic_defaults.entity_role.clone());
        let duplicate_policy = duplicate_policy.unwrap_or(DuplicatePolicy::Standalone);
        let observe_scope =
            observe_scope.unwrap_or_else(|| semantic_defaults.observe_scope.to_string());
        let enforce_scope =
            enforce_scope.unwrap_or_else(|| semantic_defaults.enforce_scope.to_string());

        let mut control_bindings = Vec::new();
        let cand_id = String::new();

        for server in &mcp_servers {
            let binding_id = format!("bind_{}", uuid::Uuid::new_v4());
            if server.transport == "stdio" {
                control_bindings.push(ControlBindingPlan {
                    binding_id,
                    kind: ControlBindingKind::McpStdioWrapper,
                    target_candidate_id: cand_id.clone(),
                    target_config_hash: None, // In real scenario, map from config evidence
                    action: ControlBindingAction::Wrap,
                    requires_user_approval: true,
                    risk: "medium".to_string(),
                    reversible: true,
                    backup_path_hash: None,
                    summary: format!("Wrap stdio MCP server: {}", server.server_name),
                });
            } else if server.transport == "http" || server.transport == "sse" {
                control_bindings.push(ControlBindingPlan {
                    binding_id,
                    kind: ControlBindingKind::McpHttpProxy,
                    target_candidate_id: cand_id.clone(),
                    target_config_hash: None,
                    action: ControlBindingAction::Proxy,
                    requires_user_approval: false,
                    risk: "low".to_string(),
                    reversible: true,
                    backup_path_hash: None,
                    summary: format!("Proxy HTTP/SSE MCP server: {}", server.server_name),
                });
            }
        }

        let preset_id =
            dek_policy_presets::catalog::preset_for_capabilities(&capability_tags, max_confidence);
        let mut labels = BTreeMap::new();
        for tag in &capability_tags {
            labels.insert(format!("capability:{}", tag), "true".into());
        }
        labels.insert(
            "entity.kind".into(),
            entity_kind_for_candidate(&agent_type).into(),
        );
        labels.insert(
            "entity.observe_enforce".into(),
            observe_enforce_class_for_candidate(&agent_type).into(),
        );
        labels.insert("suggested_preset".into(), preset_id.to_string());
        labels.insert("canonical_service_id".into(), canonical_service_id.clone());
        labels.insert("surface_group_id".into(), surface_group_id.clone());
        labels.insert(
            "authority_boundary".into(),
            format!("{:?}", authority_boundary),
        );
        labels.insert("entity_role".into(), format!("{:?}", entity_role));
        labels.insert("duplicate_policy".into(), format!("{:?}", duplicate_policy));

        capability_tags.sort();
        capability_tags.dedup();
        let should_collect_token_usage = capability_tags.iter().any(|cap| {
            matches!(
                cap.as_str(),
                "llm.call" | "llm.chat" | "net.egress.llm" | "web.chat" | "model.server"
            )
        });
        matched_signals.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut mcp_stdio_paths = vec![];
        let mut mcp_http_urls = vec![];
        let mut local_model_urls = vec![];
        let mut browser_ext_evidence = vec![];

        for ev in &group {
            match ev.source {
                EvidenceSource::McpConfig => {
                    if let Some(path) = ev.source_path_redacted.clone() {
                        mcp_stdio_paths.push(path);
                    }
                }
                EvidenceSource::LocalModelServer => {
                    if let Some(obj) = ev.data.as_object() {
                        if let Some(url) = obj.get("endpoint").and_then(|v| v.as_str()) {
                            local_model_urls.push(url.to_string());
                        }
                    }
                }
                EvidenceSource::BrowserExtension => {
                    if let Some(obj) = ev.data.as_object() {
                        if let Some(ext_id) = obj.get("extension_id").and_then(|v| v.as_str()) {
                            browser_ext_evidence.push(ext_id.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        for server in &mcp_servers {
            if server.transport == "http" {
                mcp_http_urls.push(server.server_name.clone());
            }
        }

        let observation_profile =
            observation_profile_for_agent_type(&agent_type, should_collect_token_usage);
        let observation_coverage = observation_coverage_for(&agent_type, &observation_profile);

        candidates.push(DiscoveredAgentCandidateV2 {
            schema_version: "pollek.agent_discovery_candidate.v2".into(),
            candidate_id: cand_id,
            tenant_id: tenant_id.to_string(),
            device_id: device_id.to_string(),
            status,
            canonical_service_id,
            surface_group_id,
            authority_boundary,
            entity_role,
            duplicate_policy,
            control_parent_id,
            grouping_reason,
            observe_scope,
            enforce_scope,
            related_surfaces,
            instance_count: 1,
            matched_signature_id,
            display_name: name.clone(),
            vendor,
            product,
            inferred_agent_type: agent_type.clone(),
            confidence: max_confidence,
            risk_score,
            capability_tags: capability_tags.clone(),
            matched_signals,
            first_seen: chrono::Utc::now().to_rfc3339(),
            last_seen: chrono::Utc::now().to_rfc3339(),
            scan_ids: Vec::new(),
            last_scan_id: None,
            evidence: group,
            discovered_configs: vec![],
            discovered_endpoints: endpoints,
            discovered_mcp_servers: mcp_servers,
            suggested_registration: SuggestedAgentRegistration {
                agent_id: format!("agent_{}", uuid::Uuid::new_v4()),
                name: name.clone(),
                agent_type: format!("{:?}", agent_type),
                runtime_name: "native".into(),
                process_path_hash: process_hash,
                executable_signer: None,
                declared_tools: vec![],
                declared_resources: vec![],
                mcp_stdio_config_paths: mcp_stdio_paths,
                mcp_http_urls,
                local_model_endpoints: local_model_urls,
                browser_extension_evidence: browser_ext_evidence,
                trust_level: "Unknown".into(),
                initial_status: "pending_approval".into(),
            },
            suggested_observation_profile: observation_profile,
            observation_coverage,
            suggested_control_bindings: control_bindings,
            telemetry_plan: TelemetryPlan {
                events_endpoint: "/v1/telemetry/events".into(),
                metrics_endpoint: "/v1/metrics".into(),
                capture_tool_calls: true,
                capture_arguments: true,
                redact_env_keys: redacted_env_keys,
                risk_signals: vec!["mcp_active".into()],
            },
            labels,
        });
    }

    let mut final_candidates: std::collections::HashMap<String, DiscoveredAgentCandidateV2> =
        std::collections::HashMap::new();
    for cand in candidates {
        let key = if matches!(
            cand.authority_boundary,
            AuthorityBoundary::LocalBrowserProfile
        ) || matches!(cand.inferred_agent_type, InferredAgentType::WebAIApp)
        {
            cand.evidence
                .first()
                .and_then(|ev| ev.merge_key.clone())
                .or_else(|| cand.matched_signature_id.clone())
                .unwrap_or_else(|| format!("name:{}", cand.display_name.to_lowercase()))
        } else {
            cand.matched_signature_id
                .clone()
                .or_else(|| cand.evidence.first().and_then(|ev| ev.merge_key.clone()))
                .unwrap_or_else(|| format!("name:{}", cand.display_name.to_lowercase()))
        };
        if let Some(existing) = final_candidates.get_mut(&key) {
            existing.evidence.extend(cand.evidence.clone());
            existing.confidence = f64::max(existing.confidence, cand.confidence);
            existing.labels.extend(cand.labels.clone());
            merge_candidate_semantics(existing, &cand);
            existing
                .discovered_endpoints
                .extend(cand.discovered_endpoints.clone());
            existing
                .discovered_mcp_servers
                .extend(cand.discovered_mcp_servers.clone());
            for cap in cand.capability_tags {
                if !existing.capability_tags.contains(&cap) {
                    existing.capability_tags.push(cap);
                }
            }
            existing.capability_tags.sort();
            existing.capability_tags.dedup();
            existing
                .matched_signals
                .extend(cand.matched_signals.clone());
            existing.matched_signals.sort_by(|a, b| {
                b.weight
                    .partial_cmp(&a.weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            existing.matched_signals.dedup_by(|a, b| {
                a.kind == b.kind
                    && a.detail == b.detail
                    && (a.weight - b.weight).abs() < f64::EPSILON
            });
            existing
                .suggested_registration
                .mcp_stdio_config_paths
                .extend(cand.suggested_registration.mcp_stdio_config_paths.clone());
            existing
                .suggested_registration
                .mcp_http_urls
                .extend(cand.suggested_registration.mcp_http_urls.clone());
            existing
                .suggested_registration
                .local_model_endpoints
                .extend(cand.suggested_registration.local_model_endpoints.clone());
            existing
                .suggested_registration
                .browser_extension_evidence
                .extend(
                    cand.suggested_registration
                        .browser_extension_evidence
                        .clone(),
                );
            if existing.status == crate::model::DiscoveryStatus::Unconfirmed
                && cand.status == crate::model::DiscoveryStatus::Discovered
            {
                existing.status = crate::model::DiscoveryStatus::Discovered;
            }
        } else {
            final_candidates.insert(key, cand);
        }
    }

    final_candidates.into_values().collect()
}

fn npm_pkg_from_argv(argv: &[String]) -> Option<String> {
    argv.iter().find_map(|a| {
        let a = a.replace('\\', "/");
        a.split("node_modules/")
            .nth(1)
            .map(|rest| rest.split('/').next().unwrap_or("").to_string())
            .filter(|p| !p.is_empty())
    })
}

fn basename_no_ext(p: &str) -> String {
    std::path::Path::new(p)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

fn entity_kind_for_candidate(agent_type: &InferredAgentType) -> &'static str {
    match agent_type {
        InferredAgentType::McpServer => "mcp_server",
        InferredAgentType::McpClient => "mcp_client",
        InferredAgentType::LocalModelServer => "local_model_endpoint",
        InferredAgentType::WebAIApp => "web_ai_surface",
        InferredAgentType::BrowserAgent => "browser_surface",
        InferredAgentType::IdeExtension => "ide_extension",
        _ => "ai_agent",
    }
}

fn observe_enforce_class_for_candidate(agent_type: &InferredAgentType) -> &'static str {
    match agent_type {
        InferredAgentType::McpServer
        | InferredAgentType::McpClient
        | InferredAgentType::LocalModelServer
        | InferredAgentType::WebAIApp
        | InferredAgentType::BrowserAgent
        | InferredAgentType::IdeExtension => "observable_surface",
        _ => "agent",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_probe_endpoint_uses_real_url_not_merge_key() {
        let candidates = aggregate_evidence(
            "local",
            "device-local",
            vec![DiscoveryEvidenceV2 {
                evidence_id: "ev_mcp_port_probe".into(),
                source: EvidenceSource::PortProbe,
                confidence: 0.70,
                observed_at: "2026-06-25T00:00:00Z".into(),
                privacy_class: PrivacyClass::PublicMetadata,
                redacted: false,
                data: serde_json::json!({
                    "provider": "mcp_server",
                    "transport": "sse",
                    "endpoint": "http://127.0.0.1:3000/sse",
                }),
                merge_key: Some("mcp_sse_3000".into()),
                source_path_hash: None,
                source_path_redacted: Some("http://127.0.0.1:3000/sse".into()),
            }],
        );

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(candidate.discovered_endpoints.len(), 1);
        assert_eq!(
            candidate.discovered_endpoints[0].url,
            "http://127.0.0.1:3000/sse"
        );
        assert_eq!(candidate.discovered_mcp_servers.len(), 1);
        assert_eq!(candidate.discovered_mcp_servers[0].transport, "sse");
    }

    #[test]
    fn browser_session_origin_resolves_web_ai_identity() {
        let candidates = aggregate_evidence(
            "local",
            "device-local",
            vec![DiscoveryEvidenceV2 {
                evidence_id: "ev_chatgpt_session".into(),
                source: EvidenceSource::BrowserSession,
                confidence: 0.85,
                observed_at: "2026-06-25T00:00:00Z".into(),
                privacy_class: PrivacyClass::InternalMetadata,
                redacted: true,
                data: serde_json::json!({
                    "origin": "https://chatgpt.com",
                    "name": "ChatGPT (Chrome)",
                    "vendor": "OpenAI",
                    "browser_id": "chrome",
                    "browser_name": "Chrome",
                    "capability_tags": ["llm.chat"],
                    "detected_via": "browser_session_open_tab"
                }),
                merge_key: Some("webai:chatgpt_web:chrome".into()),
                source_path_hash: Some("hash".into()),
                source_path_redacted: Some("<browser session>".into()),
            }],
        );

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(candidate.display_name, "ChatGPT (Chrome)");
        assert!(matches!(
            candidate.inferred_agent_type,
            InferredAgentType::WebAIApp
        ));
        assert!(candidate.capability_tags.contains(&"llm.chat".to_string()));
        assert!(candidate.capability_tags.contains(&"web.chat".to_string()));
    }

    #[test]
    fn browser_window_hint_resolves_named_web_ai_identity() {
        let candidates = aggregate_evidence(
            "local",
            "device-local",
            vec![DiscoveryEvidenceV2 {
                evidence_id: "ev_claude_window".into(),
                source: EvidenceSource::BrowserWindow,
                confidence: 0.85,
                observed_at: "2026-06-25T00:00:00Z".into(),
                privacy_class: PrivacyClass::InternalMetadata,
                redacted: true,
                data: serde_json::json!({
                    "origin": "https://claude.ai",
                    "name": "Claude (Chrome)",
                    "vendor": "Anthropic",
                    "browser_id": "chrome",
                    "browser_name": "Chrome",
                    "capability_tags": ["llm.chat", "web.chat"],
                    "detected_via": "browser_window_title"
                }),
                merge_key: Some("webai:claude_web:chrome".into()),
                source_path_hash: None,
                source_path_redacted: Some("chrome.exe".into()),
            }],
        );

        assert_eq!(candidates.len(), 1);
        let candidate = &candidates[0];
        assert_eq!(candidate.display_name, "Claude (Chrome)");
        assert!(matches!(
            candidate.inferred_agent_type,
            InferredAgentType::WebAIApp
        ));
        assert!(candidate.capability_tags.contains(&"llm.chat".to_string()));
    }

    #[test]
    fn same_web_ai_in_multiple_browsers_remains_separate_candidates() {
        let candidates = aggregate_evidence(
            "local",
            "device-local",
            vec![
                DiscoveryEvidenceV2 {
                    evidence_id: "ev_chatgpt_chrome".into(),
                    source: EvidenceSource::BrowserSession,
                    confidence: 0.85,
                    observed_at: "2026-06-25T00:00:00Z".into(),
                    privacy_class: PrivacyClass::InternalMetadata,
                    redacted: true,
                    data: serde_json::json!({
                        "origin": "https://chatgpt.com",
                        "name": "ChatGPT (Chrome)",
                        "vendor": "OpenAI",
                        "browser_id": "chrome",
                        "browser_name": "Chrome",
                        "capability_tags": ["llm.chat", "web.chat"],
                        "detected_via": "browser_session_open_tab"
                    }),
                    merge_key: Some("webai:chatgpt_web:chrome".into()),
                    source_path_hash: Some("hash_chrome".into()),
                    source_path_redacted: Some("<chrome session>".into()),
                },
                DiscoveryEvidenceV2 {
                    evidence_id: "ev_chatgpt_edge".into(),
                    source: EvidenceSource::BrowserSession,
                    confidence: 0.85,
                    observed_at: "2026-06-25T00:00:00Z".into(),
                    privacy_class: PrivacyClass::InternalMetadata,
                    redacted: true,
                    data: serde_json::json!({
                        "origin": "https://chatgpt.com",
                        "name": "ChatGPT (Edge)",
                        "vendor": "OpenAI",
                        "browser_id": "edge",
                        "browser_name": "Edge",
                        "capability_tags": ["llm.chat", "web.chat"],
                        "detected_via": "browser_session_open_tab"
                    }),
                    merge_key: Some("webai:chatgpt_web:edge".into()),
                    source_path_hash: Some("hash_edge".into()),
                    source_path_redacted: Some("<edge session>".into()),
                },
            ],
        );

        let names = candidates
            .iter()
            .map(|candidate| candidate.display_name.as_str())
            .collect::<std::collections::HashSet<_>>();
        let ids = candidates
            .iter()
            .map(|candidate| candidate.candidate_id.as_str())
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(candidates.len(), 2);
        assert_eq!(ids.len(), 2);
        assert!(names.contains("ChatGPT (Chrome)"));
        assert!(names.contains("ChatGPT (Edge)"));
    }

    #[test]
    fn google_ai_studio_is_child_surface_of_antigravity_parent() {
        let candidates = aggregate_evidence(
            "local",
            "device-local",
            vec![
                DiscoveryEvidenceV2 {
                    evidence_id: "ev_antigravity_process".into(),
                    source: EvidenceSource::ProcessScan,
                    confidence: 0.95,
                    observed_at: "2026-06-29T00:00:00Z".into(),
                    privacy_class: PrivacyClass::InternalMetadata,
                    redacted: true,
                    data: serde_json::json!({
                        "resolved_name": "Gemini Pro in Antigravity",
                        "vendor": "Google",
                        "matched_signature_id": "gemini_pro_antigravity",
                        "capability_tags": ["code.agentic", "tool.use", "llm.call"]
                    }),
                    merge_key: Some("process:gemini_pro_antigravity".into()),
                    source_path_hash: Some("antigravity_hash".into()),
                    source_path_redacted: Some("<app>/Antigravity".into()),
                },
                DiscoveryEvidenceV2 {
                    evidence_id: "ev_ai_studio_window".into(),
                    source: EvidenceSource::BrowserWindow,
                    confidence: 0.85,
                    observed_at: "2026-06-29T00:00:01Z".into(),
                    privacy_class: PrivacyClass::InternalMetadata,
                    redacted: true,
                    data: serde_json::json!({
                        "origin": "https://aistudio.google.com",
                        "name": "Google AI Studio (Chrome)",
                        "vendor": "Google",
                        "matched_signature_id": "google_ai_studio_web",
                        "canonical_service_id": "google_ai_studio",
                        "surface_group_id": "google_ai",
                        "entity_role": "web_ai_surface",
                        "authority_boundary": "local_browser_profile",
                        "observe_scope": "browser_metadata_network_and_prompt_guard_extension",
                        "enforce_scope": "browser_extension_or_google_agent_settings",
                        "capability_tags": ["llm.chat", "net.egress.llm"],
                        "matched_domain": "aistudio.google.com"
                    }),
                    merge_key: Some("webai:google_ai_studio_web:chrome".into()),
                    source_path_hash: None,
                    source_path_redacted: Some("chrome.exe".into()),
                },
            ],
        );

        let parent = candidates.iter().find(|candidate| {
            candidate.matched_signature_id.as_deref() == Some("gemini_pro_antigravity")
        });
        let child = candidates
            .iter()
            .find(|candidate| candidate.canonical_service_id == "google_ai_studio");

        if parent.is_none() || child.is_none() {
            assert!(parent.is_some(), "parent candidate should exist");
            assert!(child.is_some(), "child candidate should exist");
            return;
        }

        let Some(parent) = parent else {
            return;
        };
        let Some(child) = child else {
            return;
        };

        assert_eq!(child.duplicate_policy, DuplicatePolicy::ChildSurface);
        assert_eq!(
            child.control_parent_id.as_deref(),
            Some(parent.candidate_id.as_str())
        );
        assert!(parent
            .related_surfaces
            .iter()
            .any(|surface| surface.service_id == "google_ai_studio"));
    }

    #[test]
    fn port_from_url_parses_probe_endpoints() {
        assert_eq!(port_from_url("http://127.0.0.1:11434"), Some(11434));
        assert_eq!(
            port_from_url("http://127.0.0.1:30000/v1/models"),
            Some(30000)
        );
        assert_eq!(port_from_url("http://localhost/nope"), None);
    }

    #[test]
    fn process_scan_and_port_probe_of_same_engine_coalesce_to_one_agent() {
        // The same Ollama instance seen two ways: as a process and as the
        // probed local endpoint on 11434. Must become ONE candidate.
        let candidates = aggregate_evidence(
            "local",
            "device-local",
            vec![
                DiscoveryEvidenceV2 {
                    evidence_id: "ev_ollama_process".into(),
                    source: EvidenceSource::ProcessScan,
                    confidence: 0.9,
                    observed_at: "2026-07-14T00:00:00Z".into(),
                    privacy_class: PrivacyClass::InternalMetadata,
                    redacted: true,
                    data: serde_json::json!({
                        "resolved_name": "Ollama",
                        "vendor": "Ollama",
                        "matched_signature_id": "ollama",
                        "capability_tags": ["model.server"],
                        "confirmed": true,
                    }),
                    merge_key: Some("process:ollama".into()),
                    source_path_hash: Some("ollama_hash".into()),
                    source_path_redacted: Some("<bin>/ollama".into()),
                },
                DiscoveryEvidenceV2 {
                    evidence_id: "ev_ollama_probe".into(),
                    source: EvidenceSource::LocalModelServer,
                    confidence: 0.95,
                    observed_at: "2026-07-14T00:00:01Z".into(),
                    privacy_class: PrivacyClass::PublicMetadata,
                    redacted: false,
                    data: serde_json::json!({
                        "provider": "ollama",
                        "endpoint": "http://127.0.0.1:11434",
                        "models": ["llama3.2:latest"],
                    }),
                    merge_key: Some("http://127.0.0.1:11434".into()),
                    source_path_hash: None,
                    source_path_redacted: Some("http://127.0.0.1:11434".into()),
                },
            ],
        );

        let ollama_candidates: Vec<_> = candidates
            .iter()
            .filter(|c| c.matched_signature_id.as_deref() == Some("ollama"))
            .collect();
        assert_eq!(
            ollama_candidates.len(),
            1,
            "process + probe must coalesce into one Ollama candidate, got {:?}",
            candidates
                .iter()
                .map(|c| (&c.display_name, &c.matched_signature_id))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            ollama_candidates[0].evidence.len(),
            2,
            "both evidence records belong to the single candidate"
        );
    }

    #[test]
    fn near_duplicate_signature_ids_bucket_as_one_agent() {
        // The catalog has both `openclaw` (gateway process signature) and
        // `openclaw_agent` (installed binary signature) for the same product.
        let candidates = aggregate_evidence(
            "local",
            "device-local",
            vec![
                DiscoveryEvidenceV2 {
                    evidence_id: "ev_openclaw_gateway".into(),
                    source: EvidenceSource::ProcessScan,
                    confidence: 0.9,
                    observed_at: "2026-07-14T00:00:00Z".into(),
                    privacy_class: PrivacyClass::InternalMetadata,
                    redacted: true,
                    data: serde_json::json!({
                        "resolved_name": "OpenClaw Gateway",
                        "vendor": "OpenClaw",
                        "matched_signature_id": "openclaw",
                        "confirmed": true,
                    }),
                    merge_key: Some("process:openclaw".into()),
                    source_path_hash: Some("openclaw_hash".into()),
                    source_path_redacted: Some("<bin>/node".into()),
                },
                DiscoveryEvidenceV2 {
                    evidence_id: "ev_openclaw_installed".into(),
                    source: EvidenceSource::ProcessScan,
                    confidence: 0.85,
                    observed_at: "2026-07-14T00:00:01Z".into(),
                    privacy_class: PrivacyClass::InternalMetadata,
                    redacted: true,
                    data: serde_json::json!({
                        "resolved_name": "OpenClaw Agent",
                        "vendor": "OpenClaw",
                        "matched_signature_id": "openclaw_agent",
                        "confirmed": true,
                    }),
                    merge_key: Some("installed:openclaw_agent".into()),
                    source_path_hash: Some("openclaw_hash".into()),
                    source_path_redacted: Some("<bin>/openclaw".into()),
                },
            ],
        );

        let claw_candidates: Vec<_> = candidates
            .iter()
            .filter(|c| {
                c.matched_signature_id.as_deref() == Some("openclaw")
                    || c.matched_signature_id.as_deref() == Some("openclaw_agent")
            })
            .collect();
        assert_eq!(
            claw_candidates.len(),
            1,
            "openclaw + openclaw_agent must group into one agent, got {:?}",
            candidates
                .iter()
                .map(|c| (&c.display_name, &c.matched_signature_id))
                .collect::<Vec<_>>()
        );
        assert_eq!(claw_candidates[0].instance_count, 2);
    }

    #[test]
    fn observation_profile_is_tailored_per_agent_type() {
        // An MCP server exposes tools but emits no model tokens itself.
        let server = observation_profile_for_agent_type(&InferredAgentType::McpServer, true);
        assert!(server.collect_mcp_tool_metadata);
        assert!(!server.collect_token_usage, "mcp server has no tokens");
        assert!(server.collect_file_metadata);

        // A CLI coding agent runs locally: MCP tools + local files + tokens.
        let cli = observation_profile_for_agent_type(&InferredAgentType::CliAgent, true);
        assert!(cli.collect_mcp_tool_metadata);
        assert!(cli.collect_file_metadata);
        assert!(cli.collect_token_usage);

        // A web AI surface has no local process/file footprint.
        let web = observation_profile_for_agent_type(&InferredAgentType::WebAIApp, true);
        assert!(!web.collect_process_metadata);
        assert!(!web.collect_file_metadata);
        assert!(web.collect_network_metadata);

        // The profiles are genuinely different, not a single generic one.
        assert_ne!(
            server.collect_token_usage, cli.collect_token_usage,
            "server and cli profiles must differ on token usage"
        );
        assert_ne!(
            web.collect_process_metadata, cli.collect_process_metadata,
            "web and cli profiles must differ on process metadata"
        );
    }

    #[test]
    fn observation_coverage_reports_status_and_method_per_type() {
        // Non-panicking lookup so this test cannot trip the panic-guard scan.
        fn signal(coverage: &[ObservationSignalCoverage], name: &str) -> ObservationSignalCoverage {
            coverage
                .iter()
                .find(|c| c.signal == name)
                .cloned()
                .unwrap_or(ObservationSignalCoverage {
                    signal: format!("MISSING:{name}"),
                    label: String::new(),
                    status: ObservationSignalStatus::NotApplicable,
                    method: String::new(),
                })
        }

        // Local model server: token usage is Active and comes from the response
        // body parser (Ollama-style prompt_eval_count/eval_count).
        let profile =
            observation_profile_for_agent_type(&InferredAgentType::LocalModelServer, true);
        let coverage = observation_coverage_for(&InferredAgentType::LocalModelServer, &profile);
        let token = signal(&coverage, "token_usage");
        assert_eq!(token.status, ObservationSignalStatus::Active);
        assert!(
            token.method.contains("response body"),
            "local model token method should be response-body based, got {}",
            token.method
        );

        // MCP server: token usage is NotApplicable.
        let server_profile =
            observation_profile_for_agent_type(&InferredAgentType::McpServer, true);
        let server_cov = observation_coverage_for(&InferredAgentType::McpServer, &server_profile);
        assert_eq!(
            signal(&server_cov, "token_usage").status,
            ObservationSignalStatus::NotApplicable
        );

        // Web AI: process/file are NotApplicable, network is Active, token is
        // estimated from browser network traffic.
        let web_profile = observation_profile_for_agent_type(&InferredAgentType::WebAIApp, true);
        let web_cov = observation_coverage_for(&InferredAgentType::WebAIApp, &web_profile);
        assert_eq!(
            signal(&web_cov, "process_metadata").status,
            ObservationSignalStatus::NotApplicable
        );
        assert_eq!(
            signal(&web_cov, "file_metadata").status,
            ObservationSignalStatus::NotApplicable
        );
        assert_eq!(
            signal(&web_cov, "network_metadata").status,
            ObservationSignalStatus::Active
        );
        assert!(signal(&web_cov, "token_usage").method.contains("browser"));

        // Every type reports all five canonical signals.
        assert_eq!(coverage.len(), 5);
        assert_eq!(server_cov.len(), 5);
        assert_eq!(web_cov.len(), 5);
    }

    #[test]
    fn aggregated_candidate_carries_observation_coverage() {
        let candidates = aggregate_evidence(
            "local",
            "device-local",
            vec![DiscoveryEvidenceV2 {
                evidence_id: "ev_ollama".into(),
                source: EvidenceSource::LocalModelServer,
                confidence: 0.9,
                observed_at: "2026-07-14T00:00:00Z".into(),
                privacy_class: PrivacyClass::PublicMetadata,
                redacted: false,
                data: serde_json::json!({
                    "endpoint": "http://127.0.0.1:11434",
                    "provider": "ollama",
                    "capability_tags": ["model.server", "net.egress.llm"],
                }),
                merge_key: Some("local_model:ollama:11434".into()),
                source_path_hash: None,
                source_path_redacted: Some("http://127.0.0.1:11434".into()),
            }],
        );

        assert_eq!(candidates.len(), 1);
        let coverage = &candidates[0].observation_coverage;
        assert_eq!(coverage.len(), 5, "candidate exposes all five signals");
        assert!(
            coverage.iter().any(|c| c.signal == "token_usage"),
            "candidate coverage includes token usage"
        );
    }
}
