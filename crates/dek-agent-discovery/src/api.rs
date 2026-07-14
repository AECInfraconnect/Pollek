use crate::config::DiscoveryConfig;
use crate::model::*;
use crate::process_scan::scan_processes;
use anyhow::Result;
use dek_control_plane_api::registry::AiAgent;
use std::collections::BTreeMap;

pub async fn run_scan(
    tenant: &str,
    _req: &serde_json::Value,
) -> Result<Vec<DiscoveredAgentCandidate>> {
    let mut candidates = Vec::new();
    let config = DiscoveryConfig::default();

    let hostname = "device-local".to_string();

    // 1. Process Scan
    match scan_processes() {
        Ok(processes) => {
            let defs = dek_fingerprint_defs::load_latest_baseline();
            let sigs = defs.signatures;
            let apps = defs.installed_app_signatures;
            let browsers = defs.browser_processes;
            let hints = defs.ai_process_hints;
            for p in processes {
                if crate::browser_window_scan::is_browser_process(&p.process_name, &browsers) {
                    continue;
                }
                let cmdline = p.cmd_template.join(" ");
                let facts = crate::fingerprint::ProcessFacts {
                    process_name: &p.process_name,
                    exe_path: p.exe_path_redacted.as_deref().unwrap_or(""),
                    cmdline: &cmdline,
                };
                let resolved = crate::fingerprint::fingerprint_process_v2_with_hints(
                    &facts,
                    &sigs,
                    &apps,
                    Some(&hints),
                );

                let above = resolved.confidence >= config.min_fingerprint_confidence;
                if above || resolved.confidence >= config.min_unconfirmed_confidence {
                    let evidence = DiscoveryEvidence {
                        evidence_id: uuid::Uuid::new_v4().to_string(),
                        source: EvidenceSource::ProcessScan,
                        confidence: resolved.confidence,
                        observed_at: chrono::Utc::now().to_rfc3339(),
                        privacy_class: PrivacyClass::InternalMetadata,
                        redacted: true,
                        data: serde_json::json!({
                            "process": p,
                            "resolved_name": resolved.display_name,
                            "vendor": resolved.vendor,
                            "matched_signature_id": resolved.matched_signature_id,
                            "confirmed": above,
                        }),
                    };

                    let agent_type = resolved.inferred_type;
                    let display_name = resolved
                        .display_name
                        .clone()
                        .unwrap_or_else(|| p.process_name.clone());
                    let mut labels = BTreeMap::new();
                    for cap in &resolved.capability_tags {
                        labels.insert(format!("capability:{cap}"), "true".into());
                    }

                    candidates.push(DiscoveredAgentCandidate {
                        schema_version: "pollek.agent_discovery_candidate.v1".into(),
                        candidate_id: format!("cand_{}", uuid::Uuid::new_v4()),
                        tenant_id: tenant.to_string(),
                        device_id: hostname.clone(),
                        status: DiscoveryStatus::Discovered,
                        display_name: display_name.clone(),
                        inferred_agent_type: agent_type.clone(),
                        confidence: resolved.confidence,
                        risk_score: 50,
                        first_seen: chrono::Utc::now().to_rfc3339(),
                        last_seen: chrono::Utc::now().to_rfc3339(),
                        evidence: vec![evidence],
                        suggested_registration: SuggestedAgentRegistration {
                            agent_id: format!("agent_{}", uuid::Uuid::new_v4()),
                            name: display_name,
                            agent_type: format!("{:?}", agent_type),
                            runtime_name: "native".into(),
                            process_path_hash: p.exe_path_hash,
                            executable_signer: None,
                            declared_tools: vec![],
                            declared_resources: vec![],
                            mcp_stdio_config_paths: vec![],
                            mcp_http_urls: vec![],
                            local_model_endpoints: vec![],
                            browser_extension_evidence: vec![],
                            trust_level: "Unknown".into(),
                            initial_status: "pending_approval".into(),
                        },
                        suggested_observation_profile: ObservationProfile {
                            mode: ObservationMode::ObserveOnly,
                            collect_process_metadata: true,
                            collect_network_metadata: true,
                            collect_mcp_tool_metadata: true,
                            collect_token_usage: true,
                            collect_file_metadata: false,
                            collect_raw_prompt: false,
                            collect_raw_response: false,
                            retention_days: config.default_retention_days,
                        },
                        labels,
                    });
                }
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "Process scan failed, skipping");
        }
    }

    Ok(candidates)
}

pub fn to_registry_agent(
    tenant: &str,
    candidate: &DiscoveredAgentCandidate,
    req: &serde_json::Value,
) -> Result<AiAgent> {
    let name = req
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&candidate.suggested_registration.name);

    let mut capabilities = capabilities_from_labels(&candidate.labels);

    Ok(AiAgent {
        meta: dek_control_plane_api::registry::ObjectMeta {
            schema_version: "pollek.agent.v1".into(),
            tenant_id: tenant.to_string(),
            workspace_id: "default".into(),
            environment_id: "local".into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            created_by: "system".into(),
            updated_by: "system".into(),
            source: dek_control_plane_api::registry::RegistrationSource::Discovery,
            status: dek_control_plane_api::registry::RegistryStatus::Registered,
            tags: vec!["auto-discovered".into()],
        },
        agent_id: candidate.suggested_registration.agent_id.clone(),
        name: name.to_string(),
        agent_type: registry_agent_type(&candidate.inferred_agent_type, None, name),
        vendor: None,
        runtime: dek_control_plane_api::registry::AgentRuntime {
            runtime_name: candidate.suggested_registration.runtime_name.clone(),
            version: None,
        },
        entrypoints: vec![],
        declared_tools: candidate.suggested_registration.declared_tools.clone(),
        declared_resources: candidate.suggested_registration.declared_resources.clone(),
        identity: dek_control_plane_api::registry::AgentIdentity {
            spiffe_id: None,
            process_path: candidate.suggested_registration.process_path_hash.clone(),
            user_subject: None,
            signing_key_fingerprint: None,
            token_bindings: vec![],
        },
        trust_level: dek_control_plane_api::registry::TrustLevel::Medium,
        capabilities: std::mem::take(&mut capabilities),
        labels: std::collections::HashMap::new(),
    })
}

pub async fn run_scan_v2(
    tenant: &str,
    scan_id: &str,
    req: &serde_json::Value,
    sni_source: Option<std::sync::Arc<dyn crate::web_ai_scan::SniFlowSource>>,
    tx: Option<tokio::sync::mpsc::Sender<DiscoveredAgentCandidateV2>>,
    definitions: std::sync::Arc<dek_fingerprint_defs::model::FingerprintDefinition>,
) -> Result<(DiscoveryScanJob, Vec<DiscoveredAgentCandidateV2>)> {
    let mut orchestrator = crate::orchestrator::DiscoveryOrchestrator::new(tenant, definitions);
    if let Some(src) = sni_source {
        orchestrator = orchestrator.with_sni_source(src);
    }
    orchestrator.run_scan(scan_id, req, tx).await
}

pub fn stable_agent_key(candidate: &DiscoveredAgentCandidateV2) -> String {
    use sha2::{Digest, Sha256};
    let mut parts = vec![
        candidate.tenant_id.clone(),
        candidate.device_id.clone(),
        format!("{:?}", candidate.inferred_agent_type),
        candidate.display_name.to_ascii_lowercase(),
    ];

    if let Some(hash) = candidate
        .evidence
        .iter()
        .find_map(|e| e.source_path_hash.clone())
    {
        parts.push(hash);
    }

    let joined = parts.join("|");
    let mut hasher = Sha256::new();
    hasher.update(joined.as_bytes());
    let result = hasher.finalize();
    format!("agent_{}", hex::encode(&result[..8]))
}

pub fn to_registry_agent_v2(
    tenant: &str,
    candidate: &DiscoveredAgentCandidateV2,
    req: &serde_json::Value,
) -> Result<AiAgent> {
    let name = req
        .get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&candidate.suggested_registration.name);

    let agent_id = stable_agent_key(candidate);

    let mut entrypoints = Vec::new();
    for mcp in &candidate.discovered_mcp_servers {
        if let Some(cmd) = &mcp.command {
            let mut parts = cmd.split_whitespace();
            if let Some(command) = parts.next() {
                let args = parts.map(|s| s.to_string()).collect();
                entrypoints.push(dek_control_plane_api::registry::AgentEntrypoint {
                    command: command.to_string(),
                    args,
                });
            }
        }
    }

    let capabilities = capabilities_for_candidate(candidate);

    Ok(AiAgent {
        meta: dek_control_plane_api::registry::ObjectMeta {
            schema_version: "pollek.agent.v1".into(),
            tenant_id: tenant.to_string(),
            workspace_id: "default".into(),
            environment_id: "local".into(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            created_by: "system".into(),
            updated_by: "system".into(),
            source: dek_control_plane_api::registry::RegistrationSource::Discovery,
            status: dek_control_plane_api::registry::RegistryStatus::Registered,
            tags: vec!["auto-discovered".into()],
        },
        agent_id,
        name: name.to_string(),
        agent_type: registry_agent_type(
            &candidate.inferred_agent_type,
            candidate.vendor.as_deref().or(candidate.product.as_deref()),
            name,
        ),
        vendor: candidate.vendor.clone(),
        runtime: dek_control_plane_api::registry::AgentRuntime {
            runtime_name: candidate.suggested_registration.runtime_name.clone(),
            version: None,
        },
        entrypoints,
        declared_tools: candidate.suggested_registration.declared_tools.clone(),
        declared_resources: candidate.suggested_registration.declared_resources.clone(),
        identity: dek_control_plane_api::registry::AgentIdentity {
            spiffe_id: None,
            process_path: candidate.suggested_registration.process_path_hash.clone(),
            user_subject: None,
            signing_key_fingerprint: candidate.suggested_registration.executable_signer.clone(),
            token_bindings: vec![],
        },
        trust_level: dek_control_plane_api::registry::TrustLevel::Medium,
        capabilities,
        labels: {
            let mut l: std::collections::HashMap<String, String> =
                candidate.labels.clone().into_iter().collect();
            l.insert(
                "discovery_candidate_id".into(),
                candidate.candidate_id.clone(),
            );
            if let Some(scan_id) = &candidate.last_scan_id {
                l.insert("registered_from_scan_id".into(), scan_id.clone());
            }
            if let Some(merge_key) = candidate
                .evidence
                .iter()
                .find_map(|ev| ev.merge_key.clone())
            {
                l.insert("discovery_candidate_merge_key".into(), merge_key);
            }
            for (i, c) in candidate.discovered_configs.iter().enumerate() {
                l.insert(
                    format!("config_{}_{}", i, c.config_type),
                    c.path_hash.clone(),
                );
            }
            for (i, ep) in candidate.discovered_endpoints.iter().enumerate() {
                l.insert(format!("endpoint_{}_{}", i, ep.protocol), ep.url.clone());
            }
            for (i, mcp) in candidate.discovered_mcp_servers.iter().enumerate() {
                l.insert(
                    format!("mcp_{}_{}", i, mcp.server_name),
                    mcp.transport.clone(),
                );
            }
            l.insert("confidence".into(), candidate.confidence.to_string());
            if let Some(sig) = &candidate.matched_signature_id {
                l.insert("matched_signature_id".into(), sig.clone());
            }
            l.insert(
                "canonical_service_id".into(),
                candidate.canonical_service_id.clone(),
            );
            l.insert(
                "surface_group_id".into(),
                candidate.surface_group_id.clone(),
            );
            l.insert(
                "authority_boundary".into(),
                format!("{:?}", candidate.authority_boundary),
            );
            l.insert("entity_role".into(), format!("{:?}", candidate.entity_role));
            l.insert(
                "duplicate_policy".into(),
                format!("{:?}", candidate.duplicate_policy),
            );
            if let Some(parent_id) = &candidate.control_parent_id {
                l.insert("control_parent_id".into(), parent_id.clone());
            }
            if let Some(reason) = &candidate.grouping_reason {
                l.insert("grouping_reason".into(), reason.clone());
            }
            l.insert("observe_scope".into(), candidate.observe_scope.clone());
            l.insert("enforce_scope".into(), candidate.enforce_scope.clone());
            l.insert(
                "suggested_pep".into(),
                format!("{:?}", candidate.suggested_observation_profile),
            );
            l
        },
    })
}

fn capabilities_from_labels(labels: &BTreeMap<String, String>) -> Vec<String> {
    let mut capabilities = labels
        .keys()
        .filter_map(|k| k.strip_prefix("capability:").map(ToString::to_string))
        .collect::<Vec<_>>();
    capabilities.sort();
    capabilities.dedup();
    capabilities
}

fn capabilities_for_candidate(candidate: &DiscoveredAgentCandidateV2) -> Vec<String> {
    let mut capabilities = candidate.capability_tags.clone();
    capabilities.extend(capabilities_from_labels(&candidate.labels));
    match candidate.inferred_agent_type {
        InferredAgentType::LocalModelServer => capabilities.push("model.server".into()),
        InferredAgentType::McpServer => capabilities.push("mcp.server".into()),
        InferredAgentType::McpClient => capabilities.push("mcp.client".into()),
        InferredAgentType::CliAgent => capabilities.push("cli.agent".into()),
        InferredAgentType::BrowserAgent | InferredAgentType::WebAIApp => {
            capabilities.push("web.chat".into());
        }
        _ => {}
    }
    capabilities.retain(|c| !c.trim().is_empty());
    capabilities.sort();
    capabilities.dedup();
    capabilities
}

fn registry_agent_type(
    inferred: &InferredAgentType,
    vendor_or_product: Option<&str>,
    name: &str,
) -> dek_control_plane_api::registry::AgentType {
    use dek_control_plane_api::registry::AgentType;

    let identity =
        format!("{} {}", vendor_or_product.unwrap_or_default(), name).to_ascii_lowercase();

    if identity.contains("claude") {
        return AgentType::ClaudeDesktop;
    }
    if identity.contains("openai") || identity.contains("codex") || identity.contains("chatgpt") {
        return AgentType::OpenAIAgent;
    }
    if identity.contains("langchain") {
        return AgentType::LangChainAgent;
    }
    if identity.contains("llamaindex")
        || identity.contains("llama_index")
        || identity.contains("llama index")
    {
        return AgentType::LlamaIndexAgent;
    }

    match inferred {
        InferredAgentType::McpClient | InferredAgentType::McpServer => AgentType::CustomMcpClient,
        InferredAgentType::BrowserAgent | InferredAgentType::WebAIApp => AgentType::BrowserAgent,
        InferredAgentType::CliAgent => AgentType::CliAgent,
        _ => AgentType::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate_fixture() -> DiscoveredAgentCandidateV2 {
        let now = "2026-06-25T00:00:00Z".to_string();
        let mut labels = BTreeMap::new();
        labels.insert("capability:net.egress.llm".into(), "true".into());

        DiscoveredAgentCandidateV2 {
            schema_version: "pollek.agent_discovery_candidate.v2".into(),
            candidate_id: "cand_codex".into(),
            tenant_id: "local".into(),
            device_id: "device-local".into(),
            status: DiscoveryStatus::Discovered,
            canonical_service_id: "openai_codex_desktop".into(),
            surface_group_id: "openai_codex".into(),
            authority_boundary: AuthorityBoundary::LocalDevice,
            entity_role: EntityRole::LocalAgentHost,
            duplicate_policy: DuplicatePolicy::Standalone,
            control_parent_id: None,
            grouping_reason: None,
            observe_scope: "local_process_file_network_tool_metadata".into(),
            enforce_scope: "local_policy_pep_when_installed".into(),
            related_surfaces: vec![],
            instance_count: 1,
            matched_signature_id: Some("openai_codex_desktop".into()),
            display_name: "OpenAI Codex (Desktop)".into(),
            vendor: Some("OpenAI".into()),
            product: Some("Codex".into()),
            inferred_agent_type: InferredAgentType::DesktopAgent,
            confidence: 0.95,
            risk_score: 70,
            capability_tags: vec!["code.agentic".into(), "tool.use".into()],
            matched_signals: vec![MatchedSignal {
                kind: "process_name".into(),
                detail: "Codex.exe".into(),
                weight: 0.9,
            }],
            first_seen: now.clone(),
            last_seen: now,
            scan_ids: Vec::new(),
            last_scan_id: None,
            evidence: vec![],
            discovered_configs: vec![],
            discovered_endpoints: vec![],
            discovered_mcp_servers: vec![],
            suggested_registration: SuggestedAgentRegistration {
                agent_id: "agent_ignored".into(),
                name: "OpenAI Codex (Desktop)".into(),
                agent_type: "DesktopAgent".into(),
                runtime_name: "native".into(),
                process_path_hash: Some("hash".into()),
                executable_signer: None,
                declared_tools: vec![],
                declared_resources: vec![],
                mcp_stdio_config_paths: vec![],
                mcp_http_urls: vec![],
                local_model_endpoints: vec![],
                browser_extension_evidence: vec![],
                trust_level: "Unknown".into(),
                initial_status: "pending_approval".into(),
            },
            suggested_observation_profile: ObservationProfile {
                mode: ObservationMode::ObserveOnly,
                collect_process_metadata: true,
                collect_network_metadata: true,
                collect_mcp_tool_metadata: false,
                collect_token_usage: false,
                collect_file_metadata: false,
                collect_raw_prompt: false,
                collect_raw_response: false,
                retention_days: 30,
            },
            observation_coverage: Vec::new(),
            suggested_control_bindings: vec![],
            telemetry_plan: TelemetryPlan {
                events_endpoint: "/v1/telemetry/events".into(),
                metrics_endpoint: "/v1/metrics".into(),
                capture_tool_calls: true,
                capture_arguments: true,
                redact_env_keys: vec![],
                risk_signals: vec![],
            },
            labels,
        }
    }

    #[test]
    fn registry_agent_preserves_discovered_identity_and_capabilities() -> anyhow::Result<()> {
        let candidate = candidate_fixture();
        let agent = to_registry_agent_v2("local", &candidate, &serde_json::json!({}))?;

        assert_eq!(agent.name, "OpenAI Codex (Desktop)");
        assert!(matches!(
            agent.agent_type,
            dek_control_plane_api::registry::AgentType::OpenAIAgent
        ));
        assert!(agent.capabilities.contains(&"code.agentic".to_string()));
        assert!(agent.capabilities.contains(&"tool.use".to_string()));
        assert!(agent.capabilities.contains(&"net.egress.llm".to_string()));
        assert!(agent
            .labels
            .get("matched_signature_id")
            .is_some_and(|v| v == "openai_codex_desktop"));
        Ok(())
    }
}
