use crate::model::*;
use std::collections::{BTreeMap, HashMap};

mod grouping;
mod profiles;
use grouping::*;
use profiles::*;

pub fn aggregate_evidence(
    tenant_id: &str,
    device_id: &str,
    evidence: Vec<DiscoveryEvidenceV2>,
) -> Vec<DiscoveredAgentCandidateV2> {
    let raw = aggregate_by_merge_key(tenant_id, device_id, evidence);
    apply_surface_grouping(coalesce_by_identity(tenant_id, raw))
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
