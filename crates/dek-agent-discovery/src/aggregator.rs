use crate::model::*;
use std::collections::{BTreeMap, HashMap};

pub fn aggregate_evidence(
    tenant_id: &str,
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
        let risk_score = 10;
        let mut agent_type = InferredAgentType::UnknownAiProcess;
        let mut name = "Unknown Agent".to_string();

        let mut process_hash = None;
        let mut mcp_servers = Vec::new();
        let mut endpoints = Vec::new();

        for ev in &group {
            if ev.confidence > max_confidence {
                max_confidence = ev.confidence;
            }

            match ev.source {
                EvidenceSource::ProcessScan => {
                    name = ev.source_path_redacted.clone().unwrap_or(name);
                    agent_type = crate::fingerprint::infer_agent_type_from_name(&name);
                    process_hash = ev.source_path_hash.clone();
                }
                EvidenceSource::McpConfig => {
                    if agent_type == InferredAgentType::UnknownAiProcess {
                        agent_type = InferredAgentType::DesktopAgent;
                        name = "MCP Capable Agent".to_string();
                    }
                    if let Some(data) = ev.data.get("servers") {
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
                    agent_type = InferredAgentType::LocalModelServer;
                    name = "Local Model Server".into();
                    if let Some(key_url) = &ev.merge_key {
                        endpoints.push(DiscoveredEndpointRef {
                            url: key_url.clone(),
                            protocol: "http".into(),
                        });
                    }
                }
                EvidenceSource::IdeExtension => {
                    agent_type = InferredAgentType::IdeExtension;
                    name = "IDE Extension".into();
                }
                _ => {}
            }
        }

        candidates.push(DiscoveredAgentCandidateV2 {
            schema_version: "pollen.agent_discovery_candidate.v2".into(),
            candidate_id: format!("cand_{}", uuid::Uuid::new_v4()),
            tenant_id: tenant_id.to_string(),
            device_id: "device-local".into(),
            status: DiscoveryStatus::Discovered,
            display_name: name.clone(),
            vendor: None,
            product: None,
            inferred_agent_type: agent_type.clone(),
            confidence: max_confidence,
            risk_score,
            first_seen: chrono::Utc::now().to_rfc3339(),
            last_seen: chrono::Utc::now().to_rfc3339(),
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
                trust_level: "medium".into(),
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
            suggested_control_bindings: vec![],
            telemetry_plan: TelemetryPlan {
                events_endpoint: "/v1/telemetry/events".into(),
                metrics_endpoint: "/v1/metrics".into(),
            },
            labels: BTreeMap::new(),
        });
    }

    candidates
}
