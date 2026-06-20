use crate::model::*;
use crate::process_scan::scan_processes;
use anyhow::Result;
use std::collections::BTreeMap;
use dek_control_plane_api::registry::AiAgent;

pub async fn run_scan(_tenant: &str, _req: &serde_json::Value) -> Result<Vec<DiscoveredAgentCandidate>> {
    let mut candidates = Vec::new();

    // 1. Process Scan
    if let Ok(processes) = scan_processes() {
        for p in processes {
            let conf = crate::fingerprint::fingerprint_process(&p.process_name);
            if conf > 0.5 {
                let evidence = DiscoveryEvidence {
                    evidence_id: uuid::Uuid::new_v4().to_string(),
                    source: EvidenceSource::ProcessScan,
                    confidence: conf,
                    observed_at: chrono::Utc::now().to_rfc3339(),
                    privacy_class: PrivacyClass::InternalMetadata,
                    redacted: true,
                    data: serde_json::to_value(&p).unwrap_or_default(),
                };
                
                let agent_type = crate::fingerprint::infer_agent_type_from_name(&p.process_name);

                candidates.push(DiscoveredAgentCandidate {
                    schema_version: "agent-discovery-candidate.v1".into(),
                    candidate_id: format!("cand_{}", uuid::Uuid::new_v4()),
                    tenant_id: _tenant.to_string(),
                    device_id: "device-local".into(),
                    status: DiscoveryStatus::Discovered,
                    display_name: p.process_name.clone(),
                    inferred_agent_type: agent_type,
                    confidence: conf,
                    risk_score: 50,
                    first_seen: chrono::Utc::now().to_rfc3339(),
                    last_seen: chrono::Utc::now().to_rfc3339(),
                    evidence: vec![evidence],
                    suggested_registration: SuggestedAgentRegistration {
                        agent_id: format!("agent_{}", uuid::Uuid::new_v4()),
                        name: p.process_name,
                        agent_type: "unknown".into(),
                        runtime_name: "native".into(),
                        process_path_hash: p.exe_path_hash,
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
                        collect_mcp_tool_metadata: true,
                        collect_token_usage: true,
                        collect_file_metadata: false,
                        collect_raw_prompt: false,
                        collect_raw_response: false,
                        retention_days: 14,
                    },
                    labels: BTreeMap::new(),
                });
            }
        }
    }

    Ok(candidates)
}

pub fn to_registry_agent(
    _tenant: &str,
    candidate: &DiscoveredAgentCandidate,
    req: &serde_json::Value,
) -> Result<AiAgent> {
    let name = req.get("agent_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&candidate.suggested_registration.name);
    
    Ok(AiAgent {
        meta: dek_control_plane_api::registry::ObjectMeta {
            schema_version: "pollen.agent.v1".into(),
            tenant_id: _tenant.to_string(),
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
        agent_type: dek_control_plane_api::registry::AgentType::Unknown,
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
        },
        trust_level: dek_control_plane_api::registry::TrustLevel::Medium,
        capabilities: vec![],
        labels: std::collections::HashMap::new(),
    })
}
