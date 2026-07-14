use crate::model::{
    CanonicalCapability, DiscoveredAgentCandidateV2, DiscoveredRelationship,
    DiscoveryEntityCandidate, DiscoveryEntityKind, DiscoveryEvidenceV2, EvidenceSource,
    PrivacyClass,
};
use std::collections::{BTreeMap, BTreeSet};

pub fn capabilities_for_candidate(
    candidate: &DiscoveredAgentCandidateV2,
) -> Vec<CanonicalCapability> {
    let mut by_id = BTreeMap::<String, CanonicalCapability>::new();

    for tag in candidate
        .capability_tags
        .iter()
        .filter(|tag| !tag.is_empty())
    {
        let id = capability_id(&candidate.candidate_id, "tag", tag);
        by_id
            .entry(id.clone())
            .or_insert_with(|| CanonicalCapability {
                capability_id: id,
                candidate_id: candidate.candidate_id.clone(),
                capability_kind: "tag".into(),
                name: tag.clone(),
                description: Some(format!("Inferred from discovery capability tag `{tag}`.")),
                input_schema: None,
                output_schema: None,
                modality: modality_for_tag(tag),
                actions: actions_for_tag(tag),
                source: "candidate.capability_tags".into(),
                confidence: candidate.confidence,
                risk_tags: risk_tags_for_capability(tag),
                evidence_ids: evidence_ids_for_capability(&candidate.evidence, tag),
                privacy_class: PrivacyClass::InternalMetadata,
            });
    }

    for mcp in &candidate.discovered_mcp_servers {
        let name = format!("{} ({})", mcp.server_name, mcp.transport);
        let id = capability_id(&candidate.candidate_id, "mcp_server", &name);
        by_id.entry(id.clone()).or_insert_with(|| CanonicalCapability {
            capability_id: id,
            candidate_id: candidate.candidate_id.clone(),
            capability_kind: "mcp_server".into(),
            name,
            description: Some("MCP server declared by local configuration or runtime evidence. Discovery lists metadata only and does not invoke tools.".into()),
            input_schema: None,
            output_schema: None,
            modality: vec!["tool".into()],
            actions: vec!["list_tools".into(), "list_resources".into(), "list_prompts".into()],
            source: "candidate.discovered_mcp_servers".into(),
            confidence: candidate.confidence,
            risk_tags: vec!["tool_surface".into()],
            evidence_ids: source_evidence_ids(&candidate.evidence, EvidenceSource::McpConfig),
            privacy_class: PrivacyClass::InternalMetadata,
        });
    }

    for endpoint in &candidate.discovered_endpoints {
        let id = capability_id(&candidate.candidate_id, "endpoint", &endpoint.url);
        by_id
            .entry(id.clone())
            .or_insert_with(|| CanonicalCapability {
                capability_id: id,
                candidate_id: candidate.candidate_id.clone(),
                capability_kind: "endpoint".into(),
                name: endpoint.url.clone(),
                description: Some(format!(
                    "Discovered {} endpoint from local metadata.",
                    endpoint.protocol
                )),
                input_schema: None,
                output_schema: None,
                modality: vec!["network".into()],
                actions: vec!["observe_endpoint".into()],
                source: "candidate.discovered_endpoints".into(),
                confidence: candidate.confidence,
                risk_tags: vec!["network_surface".into()],
                evidence_ids: source_evidence_ids(
                    &candidate.evidence,
                    EvidenceSource::LocalModelServer,
                ),
                privacy_class: PrivacyClass::InternalMetadata,
            });
    }

    for config in &candidate.discovered_configs {
        let id = capability_id(&candidate.candidate_id, "config", &config.path_hash);
        by_id
            .entry(id.clone())
            .or_insert_with(|| CanonicalCapability {
                capability_id: id,
                candidate_id: candidate.candidate_id.clone(),
                capability_kind: "configuration".into(),
                name: config.config_type.clone(),
                description: Some(format!(
                    "Configuration metadata observed at {}.",
                    config.path_redacted
                )),
                input_schema: None,
                output_schema: None,
                modality: vec!["configuration".into()],
                actions: vec!["observe_config".into(), "wrap_after_approval".into()],
                source: "candidate.discovered_configs".into(),
                confidence: candidate.confidence,
                risk_tags: vec!["configuration_surface".into()],
                evidence_ids: source_evidence_ids(&candidate.evidence, EvidenceSource::McpConfig),
                privacy_class: PrivacyClass::SensitiveMetadata,
            });
    }

    for evidence in &candidate.evidence {
        for capability in capabilities_from_evidence(candidate, evidence) {
            by_id
                .entry(capability.capability_id.clone())
                .or_insert(capability);
        }
    }

    by_id.into_values().collect()
}

pub fn entity_for_candidate(candidate: &DiscoveredAgentCandidateV2) -> DiscoveryEntityCandidate {
    let capabilities = capabilities_for_candidate(candidate);
    let relationships = relationships_for_candidate(candidate);
    DiscoveryEntityCandidate {
        schema_version: "pollek.discovery_entity_candidate.v1".into(),
        candidate_id: candidate.candidate_id.clone(),
        tenant_id: candidate.tenant_id.clone(),
        device_id: candidate.device_id.clone(),
        entity_kind: entity_kind_for_candidate(candidate),
        display_name: candidate.display_name.clone(),
        vendor: candidate.vendor.clone(),
        product: candidate.product.clone(),
        confidence: candidate.confidence,
        risk_score: candidate.risk_score,
        status: candidate.status.clone(),
        capabilities,
        evidence: candidate.evidence.clone(),
        relationships,
        suggested_registration: serde_json::to_value(&candidate.suggested_registration)
            .unwrap_or_default(),
        suggested_control_bindings: candidate.suggested_control_bindings.clone(),
        observation_coverage: candidate.observation_coverage.clone(),
        privacy_profile: privacy_profile(candidate),
        performance_cost_class: performance_cost_class(candidate),
        first_seen: candidate.first_seen.clone(),
        last_seen: candidate.last_seen.clone(),
    }
}

pub fn entities_for_candidates(
    candidates: &[DiscoveredAgentCandidateV2],
) -> Vec<DiscoveryEntityCandidate> {
    candidates.iter().map(entity_for_candidate).collect()
}

fn capabilities_from_evidence(
    candidate: &DiscoveredAgentCandidateV2,
    evidence: &DiscoveryEvidenceV2,
) -> Vec<CanonicalCapability> {
    if evidence.source == EvidenceSource::PortProbe {
        return capabilities_from_mcp_port_probe(candidate, evidence);
    }
    capability_from_evidence(candidate, evidence)
        .into_iter()
        .collect()
}

fn capabilities_from_mcp_port_probe(
    candidate: &DiscoveredAgentCandidateV2,
    evidence: &DiscoveryEvidenceV2,
) -> Vec<CanonicalCapability> {
    let endpoint = evidence
        .data
        .get("endpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("mcp_endpoint");

    let Some(mcp) = evidence.data.get("mcp") else {
        return vec![CanonicalCapability {
            capability_id: capability_id(&candidate.candidate_id, "mcp_endpoint", endpoint),
            candidate_id: candidate.candidate_id.clone(),
            capability_kind: "mcp_endpoint".into(),
            name: "MCP-compatible endpoint".into(),
            description: Some(format!(
                "A local MCP-compatible endpoint was detected at {endpoint}, but live capability retrieval has not yet succeeded."
            )),
            input_schema: None,
            output_schema: None,
            modality: vec!["tool".into()],
            actions: vec!["observe_endpoint".into()],
            source: "port_probe".into(),
            confidence: evidence.confidence,
            risk_tags: vec!["tool_surface".into()],
            evidence_ids: vec![evidence.evidence_id.clone()],
            privacy_class: evidence.privacy_class.clone(),
        }];
    };

    let server_label = mcp
        .get("server_name")
        .and_then(|v| v.as_str())
        .unwrap_or(endpoint);

    let mut caps = vec![CanonicalCapability {
        capability_id: capability_id(&candidate.candidate_id, "mcp_server_live", server_label),
        candidate_id: candidate.candidate_id.clone(),
        capability_kind: "mcp_server".into(),
        name: server_label.to_string(),
        description: Some(format!(
            "MCP server at {endpoint} responded to a bounded tools/resources/prompts listing. Discovery does not invoke tools or read resource/prompt content."
        )),
        input_schema: None,
        output_schema: None,
        modality: vec!["tool".into()],
        actions: vec!["list_tools".into(), "list_resources".into(), "list_prompts".into()],
        source: "mcp_initialize_live".into(),
        confidence: evidence.confidence,
        risk_tags: vec!["tool_surface".into()],
        evidence_ids: vec![evidence.evidence_id.clone()],
        privacy_class: evidence.privacy_class.clone(),
    }];

    if let Some(tools) = mcp.get("tools").and_then(|v| v.as_array()) {
        for tool in tools {
            let Some(tool_name) = tool.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            caps.push(CanonicalCapability {
                capability_id: capability_id(
                    &candidate.candidate_id,
                    "mcp_tool",
                    &format!("{server_label}_{tool_name}"),
                ),
                candidate_id: candidate.candidate_id.clone(),
                capability_kind: "mcp_tool".into(),
                name: tool_name.to_string(),
                description: tool
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                input_schema: tool.get("inputSchema").cloned(),
                output_schema: None,
                modality: vec!["tool".into()],
                actions: vec!["use_tool".into()],
                source: "mcp_tools_list_live".into(),
                confidence: evidence.confidence,
                risk_tags: vec!["tool_execution".into()],
                evidence_ids: vec![evidence.evidence_id.clone()],
                privacy_class: evidence.privacy_class.clone(),
            });
        }
    }

    if let Some(resources) = mcp.get("resources").and_then(|v| v.as_array()) {
        for resource in resources {
            let Some(uri) = resource.get("uri").and_then(|v| v.as_str()) else {
                continue;
            };
            let name = resource.get("name").and_then(|v| v.as_str()).unwrap_or(uri);
            caps.push(CanonicalCapability {
                capability_id: capability_id(
                    &candidate.candidate_id,
                    "mcp_resource",
                    &format!("{server_label}_{uri}"),
                ),
                candidate_id: candidate.candidate_id.clone(),
                capability_kind: "mcp_resource".into(),
                name: name.to_string(),
                description: resource
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| {
                        Some(format!(
                            "MCP resource exposed at {uri}. Discovery lists resource metadata only and does not read its content."
                        ))
                    }),
                input_schema: None,
                output_schema: None,
                modality: vec!["resource".into()],
                actions: vec!["observe_resource".into()],
                source: "mcp_resources_list_live".into(),
                confidence: evidence.confidence,
                risk_tags: vec!["data_access".into()],
                evidence_ids: vec![evidence.evidence_id.clone()],
                privacy_class: evidence.privacy_class.clone(),
            });
        }
    }

    if let Some(prompts) = mcp.get("prompts").and_then(|v| v.as_array()) {
        for prompt in prompts {
            let Some(prompt_name) = prompt.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            caps.push(CanonicalCapability {
                capability_id: capability_id(
                    &candidate.candidate_id,
                    "mcp_prompt",
                    &format!("{server_label}_{prompt_name}"),
                ),
                candidate_id: candidate.candidate_id.clone(),
                capability_kind: "mcp_prompt".into(),
                name: prompt_name.to_string(),
                description: prompt
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                input_schema: None,
                output_schema: None,
                modality: vec!["prompt".into()],
                actions: vec!["observe_prompt".into()],
                source: "mcp_prompts_list_live".into(),
                confidence: evidence.confidence,
                risk_tags: vec![],
                evidence_ids: vec![evidence.evidence_id.clone()],
                privacy_class: evidence.privacy_class.clone(),
            });
        }
    }

    caps
}

fn capability_from_evidence(
    candidate: &DiscoveredAgentCandidateV2,
    evidence: &DiscoveryEvidenceV2,
) -> Option<CanonicalCapability> {
    match &evidence.source {
        EvidenceSource::BrowserSession
        | EvidenceSource::BrowserWindow
        | EvidenceSource::BrowserHistory
        | EvidenceSource::NetworkSni
        | EvidenceSource::NetworkEgress => {
            let host = evidence
                .data
                .get("host")
                .or_else(|| evidence.data.get("sni_host"))
                .or_else(|| evidence.data.get("domain"))
                .and_then(|v| v.as_str())
                .unwrap_or("browser_ai_session");
            Some(CanonicalCapability {
                capability_id: capability_id(&candidate.candidate_id, "browser_ai", host),
                candidate_id: candidate.candidate_id.clone(),
                capability_kind: "browser_ai_session".into(),
                name: evidence
                    .data
                    .get("name")
                    .or_else(|| evidence.data.get("browser_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Browser AI session")
                    .to_string(),
                description: Some("Browser or network metadata indicates AI usage. No chat content is captured by discovery.".into()),
                input_schema: None,
                output_schema: None,
                modality: vec!["web".into(), "network".into()],
                actions: vec!["observe_session".into()],
                source: format!("{:?}", evidence.source),
                confidence: evidence.confidence,
                risk_tags: vec!["shadow_ai".into()],
                evidence_ids: vec![evidence.evidence_id.clone()],
                privacy_class: evidence.privacy_class.clone(),
            })
        }
        EvidenceSource::Container => Some(CanonicalCapability {
            capability_id: capability_id(
                &candidate.candidate_id,
                "container",
                &evidence.evidence_id,
            ),
            candidate_id: candidate.candidate_id.clone(),
            capability_kind: "container_runtime".into(),
            name: evidence
                .data
                .get("image")
                .or_else(|| evidence.data.get("container_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("Containerized AI runtime")
                .to_string(),
            description: Some(
                "Container metadata indicates a local AI runtime or model service.".into(),
            ),
            input_schema: None,
            output_schema: None,
            modality: vec!["container".into()],
            actions: vec!["observe_container".into()],
            source: "container".into(),
            confidence: evidence.confidence,
            risk_tags: vec!["runtime_surface".into()],
            evidence_ids: vec![evidence.evidence_id.clone()],
            privacy_class: evidence.privacy_class.clone(),
        }),
        EvidenceSource::PythonFramework | EvidenceSource::IdeExtension => {
            Some(CanonicalCapability {
                capability_id: capability_id(
                    &candidate.candidate_id,
                    "framework",
                    &evidence.evidence_id,
                ),
                candidate_id: candidate.candidate_id.clone(),
                capability_kind: "framework_or_extension".into(),
                name: evidence
                    .data
                    .get("package")
                    .or_else(|| evidence.data.get("extension_name"))
                    .or_else(|| evidence.data.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Agent framework")
                    .to_string(),
                description: Some(
                    "Framework or IDE extension metadata indicates agent-building capability."
                        .into(),
                ),
                input_schema: None,
                output_schema: None,
                modality: vec!["framework".into()],
                actions: vec!["observe_framework".into()],
                source: format!("{:?}", evidence.source),
                confidence: evidence.confidence,
                risk_tags: vec!["agentic_runtime".into()],
                evidence_ids: vec![evidence.evidence_id.clone()],
                privacy_class: evidence.privacy_class.clone(),
            })
        }
        _ => None,
    }
}

fn relationships_for_candidate(
    candidate: &DiscoveredAgentCandidateV2,
) -> Vec<DiscoveredRelationship> {
    let mut relationships = Vec::new();

    for mcp in &candidate.discovered_mcp_servers {
        let object_id = capability_id(&candidate.candidate_id, "mcp_server", &mcp.server_name);
        relationships.push(DiscoveredRelationship {
            relationship_id: capability_id(&candidate.candidate_id, "rel_exposes", &object_id),
            subject_candidate_id: candidate.candidate_id.clone(),
            relation: "exposes_mcp_server".into(),
            object_candidate_id: object_id,
            confidence: candidate.confidence,
            evidence_ids: source_evidence_ids(&candidate.evidence, EvidenceSource::McpConfig),
        });
    }

    for endpoint in &candidate.discovered_endpoints {
        let object_id = capability_id(&candidate.candidate_id, "endpoint", &endpoint.url);
        relationships.push(DiscoveredRelationship {
            relationship_id: capability_id(&candidate.candidate_id, "rel_uses", &object_id),
            subject_candidate_id: candidate.candidate_id.clone(),
            relation: "uses_endpoint".into(),
            object_candidate_id: object_id,
            confidence: candidate.confidence,
            evidence_ids: source_evidence_ids(
                &candidate.evidence,
                EvidenceSource::LocalModelServer,
            ),
        });
    }

    for surface in &candidate.related_surfaces {
        let object_id = capability_id(&candidate.candidate_id, "surface", &surface.service_id);
        relationships.push(DiscoveredRelationship {
            relationship_id: capability_id(
                &candidate.candidate_id,
                "rel_related_surface",
                &object_id,
            ),
            subject_candidate_id: candidate.candidate_id.clone(),
            relation: "has_related_surface".into(),
            object_candidate_id: object_id,
            confidence: surface.confidence,
            evidence_ids: candidate
                .evidence
                .iter()
                .map(|evidence| evidence.evidence_id.clone())
                .collect(),
        });
    }

    relationships
}

fn entity_kind_for_candidate(candidate: &DiscoveredAgentCandidateV2) -> DiscoveryEntityKind {
    match candidate.inferred_agent_type {
        crate::model::InferredAgentType::WebAIApp => DiscoveryEntityKind::AgenticHost,
        crate::model::InferredAgentType::McpServer => DiscoveryEntityKind::McpServer,
        crate::model::InferredAgentType::LocalModelServer => DiscoveryEntityKind::ModelProvider,
        crate::model::InferredAgentType::IdeExtension => DiscoveryEntityKind::IdeExtension,
        _ => DiscoveryEntityKind::Agent,
    }
}

fn capability_id(candidate_id: &str, kind: &str, value: &str) -> String {
    let slug: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("_");
    format!(
        "{}::{}::{}",
        candidate_id,
        kind,
        if slug.is_empty() { "unknown" } else { &slug }
    )
}

fn modality_for_tag(tag: &str) -> Vec<String> {
    let mut modality = BTreeSet::new();
    if tag.contains("llm") || tag.contains("chat") {
        modality.insert("text".to_string());
    }
    if tag.contains("vision") || tag.contains("multimodal") {
        modality.insert("vision".to_string());
    }
    if tag.contains("tool") || tag.contains("mcp") {
        modality.insert("tool".to_string());
    }
    if tag.contains("model") {
        modality.insert("model".to_string());
    }
    if modality.is_empty() {
        modality.insert("metadata".to_string());
    }
    modality.into_iter().collect()
}

fn actions_for_tag(tag: &str) -> Vec<String> {
    let mut actions = BTreeSet::new();
    if tag.contains("tool") || tag.contains("mcp") {
        actions.insert("use_tool".to_string());
    }
    if tag.contains("model") || tag.contains("llm") || tag.contains("chat") {
        actions.insert("generate".to_string());
    }
    if tag.contains("browser") || tag.contains("web") {
        actions.insert("web_session".to_string());
    }
    if actions.is_empty() {
        actions.insert("observe".to_string());
    }
    actions.into_iter().collect()
}

fn risk_tags_for_capability(tag: &str) -> Vec<String> {
    let mut risks = BTreeSet::new();
    if tag.contains("web") || tag.contains("llm") {
        risks.insert("external_ai".to_string());
    }
    if tag.contains("tool") || tag.contains("mcp") {
        risks.insert("tool_execution".to_string());
    }
    if tag.contains("file") || tag.contains("resource") {
        risks.insert("data_access".to_string());
    }
    risks.into_iter().collect()
}

fn evidence_ids_for_capability(evidence: &[DiscoveryEvidenceV2], tag: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for ev in evidence {
        let includes_tag = ev
            .data
            .get("capability_tags")
            .and_then(|value| value.as_array())
            .map(|items| items.iter().any(|item| item.as_str() == Some(tag)))
            .unwrap_or(false);
        if includes_tag {
            ids.push(ev.evidence_id.clone());
        }
    }
    ids
}

fn source_evidence_ids(evidence: &[DiscoveryEvidenceV2], source: EvidenceSource) -> Vec<String> {
    evidence
        .iter()
        .filter(|ev| ev.source == source)
        .map(|ev| ev.evidence_id.clone())
        .collect()
}

fn privacy_profile(candidate: &DiscoveredAgentCandidateV2) -> String {
    if candidate
        .evidence
        .iter()
        .any(|ev| ev.privacy_class == PrivacyClass::SensitiveMetadata)
    {
        "sensitive_metadata_only".into()
    } else {
        "metadata_only".into()
    }
}

fn performance_cost_class(candidate: &DiscoveredAgentCandidateV2) -> String {
    if candidate.evidence.iter().any(|ev| {
        matches!(
            ev.source,
            EvidenceSource::NetworkSni | EvidenceSource::NetworkEgress
        )
    }) {
        "passive_network_metadata".into()
    } else if candidate.evidence.iter().any(|ev| {
        matches!(
            ev.source,
            EvidenceSource::LocalModelServer | EvidenceSource::PortProbe
        )
    }) {
        "loopback_probe_metadata".into()
    } else {
        "passive_metadata".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AuthorityBoundary, ControlBindingPlan, DiscoveredConfigRef, DiscoveredEndpointRef,
        DiscoveredMcpServerRef, DiscoveryStatus, DuplicatePolicy, EntityRole, InferredAgentType,
        ObservationMode, ObservationProfile, SuggestedAgentRegistration, TelemetryPlan,
    };

    #[test]
    fn derives_capabilities_from_candidate_evidence() {
        let candidate = sample_candidate();
        let capabilities = capabilities_for_candidate(&candidate);

        assert!(capabilities.iter().any(|cap| cap.name == "llm.chat"));
        assert!(capabilities
            .iter()
            .any(|cap| cap.capability_kind == "mcp_server"));
        assert!(capabilities
            .iter()
            .any(|cap| cap.capability_kind == "endpoint"));
    }

    #[test]
    fn derives_live_mcp_tool_resource_and_prompt_capabilities() {
        let mut candidate = sample_candidate();
        candidate.evidence.push(DiscoveryEvidenceV2 {
            evidence_id: "ev_mcp_live".into(),
            source: EvidenceSource::PortProbe,
            confidence: 0.98,
            observed_at: "2026-06-26T00:00:00Z".into(),
            privacy_class: PrivacyClass::PublicMetadata,
            redacted: false,
            data: serde_json::json!({
                "provider": "mcp_server",
                "transport": "http",
                "endpoint": "http://127.0.0.1:3000/mcp",
                "mcp": {
                    "server_name": "demo-mcp",
                    "server_version": "1.0.0",
                    "protocol_version": "2025-03-26",
                    "tools": [{"name": "search", "description": "Search things", "inputSchema": {"type": "object"}}],
                    "tools_truncated": false,
                    "resources": [{"uri": "file:///demo.txt", "name": "demo"}],
                    "resources_truncated": false,
                    "prompts": [{"name": "greet"}],
                    "prompts_truncated": false,
                },
            }),
            merge_key: Some("mcp_sse_3000".into()),
            source_path_hash: None,
            source_path_redacted: Some("http://127.0.0.1:3000/mcp".into()),
        });

        let capabilities = capabilities_for_candidate(&candidate);

        let tool = capabilities
            .iter()
            .find(|cap| cap.capability_kind == "mcp_tool");
        assert!(tool.is_some(), "expected a live mcp_tool capability");
        if let Some(tool) = tool {
            assert_eq!(tool.name, "search");
            assert!(tool.input_schema.is_some());
        }

        assert!(capabilities
            .iter()
            .any(|cap| cap.capability_kind == "mcp_resource" && cap.name == "demo"));
        assert!(capabilities
            .iter()
            .any(|cap| cap.capability_kind == "mcp_prompt" && cap.name == "greet"));
    }

    #[test]
    fn falls_back_to_generic_capability_when_mcp_probe_has_no_live_data() {
        let mut candidate = sample_candidate();
        candidate.evidence.push(DiscoveryEvidenceV2 {
            evidence_id: "ev_mcp_heuristic".into(),
            source: EvidenceSource::PortProbe,
            confidence: 0.70,
            observed_at: "2026-06-26T00:00:00Z".into(),
            privacy_class: PrivacyClass::PublicMetadata,
            redacted: false,
            data: serde_json::json!({
                "provider": "mcp_server",
                "transport": "sse",
                "endpoint": "http://127.0.0.1:3001/sse",
            }),
            merge_key: Some("mcp_sse_3001".into()),
            source_path_hash: None,
            source_path_redacted: Some("http://127.0.0.1:3001/sse".into()),
        });

        let capabilities = capabilities_for_candidate(&candidate);
        assert!(capabilities
            .iter()
            .any(|cap| cap.capability_kind == "mcp_endpoint"));
    }

    #[test]
    fn builds_entity_with_relationships() {
        let candidate = sample_candidate();
        let entity = entity_for_candidate(&candidate);

        assert_eq!(
            entity.schema_version,
            "pollek.discovery_entity_candidate.v1"
        );
        assert_eq!(entity.entity_kind, DiscoveryEntityKind::Agent);
        assert!(!entity.capabilities.is_empty());
        assert!(entity
            .relationships
            .iter()
            .any(|rel| rel.relation == "exposes_mcp_server"));
    }

    fn sample_candidate() -> DiscoveredAgentCandidateV2 {
        DiscoveredAgentCandidateV2 {
            schema_version: "pollek.agent_discovery_candidate.v2".into(),
            candidate_id: "agent_demo".into(),
            tenant_id: "local".into(),
            device_id: "device".into(),
            status: DiscoveryStatus::Discovered,
            canonical_service_id: "demo_agent".into(),
            surface_group_id: "demo_agent".into(),
            authority_boundary: AuthorityBoundary::LocalDevice,
            entity_role: EntityRole::LocalAgentHost,
            duplicate_policy: DuplicatePolicy::Standalone,
            control_parent_id: None,
            grouping_reason: None,
            observe_scope: "local_process_file_network_tool_metadata".into(),
            enforce_scope: "local_policy_pep_when_installed".into(),
            related_surfaces: vec![],
            instance_count: 1,
            matched_signature_id: None,
            display_name: "Demo Agent".into(),
            vendor: Some("Demo".into()),
            product: Some("Agent".into()),
            inferred_agent_type: InferredAgentType::DesktopAgent,
            confidence: 0.87,
            risk_score: 42,
            capability_tags: vec!["llm.chat".into()],
            matched_signals: vec![],
            first_seen: "2026-06-26T00:00:00Z".into(),
            last_seen: "2026-06-26T00:00:00Z".into(),
            scan_ids: vec!["scan_1".into()],
            last_scan_id: Some("scan_1".into()),
            evidence: vec![DiscoveryEvidenceV2 {
                evidence_id: "ev_1".into(),
                source: EvidenceSource::BrowserWindow,
                confidence: 0.8,
                observed_at: "2026-06-26T00:00:00Z".into(),
                privacy_class: PrivacyClass::InternalMetadata,
                redacted: true,
                data: serde_json::json!({"browser_name": "Edge", "capability_tags": ["llm.chat"]}),
                merge_key: Some("demo".into()),
                source_path_hash: None,
                source_path_redacted: None,
            }],
            discovered_configs: vec![DiscoveredConfigRef {
                path_hash: "cfg_hash".into(),
                path_redacted: "%APPDATA%/Demo/config.json".into(),
                config_type: "mcp".into(),
            }],
            discovered_endpoints: vec![DiscoveredEndpointRef {
                url: "http://127.0.0.1:11434".into(),
                protocol: "http".into(),
            }],
            discovered_mcp_servers: vec![DiscoveredMcpServerRef {
                server_name: "filesystem".into(),
                transport: "stdio".into(),
                command: Some("mcp-server-filesystem".into()),
            }],
            suggested_registration: SuggestedAgentRegistration {
                agent_id: "agent_demo".into(),
                name: "Demo Agent".into(),
                agent_type: "desktop".into(),
                runtime_name: "demo".into(),
                process_path_hash: None,
                executable_signer: None,
                declared_tools: vec![],
                declared_resources: vec![],
                mcp_stdio_config_paths: vec![],
                mcp_http_urls: vec![],
                local_model_endpoints: vec![],
                browser_extension_evidence: vec![],
                trust_level: "medium".into(),
                initial_status: "observed".into(),
            },
            suggested_observation_profile: ObservationProfile {
                mode: ObservationMode::ObserveOnly,
                collect_process_metadata: true,
                collect_network_metadata: true,
                collect_mcp_tool_metadata: true,
                collect_token_usage: false,
                collect_file_metadata: false,
                collect_raw_prompt: false,
                collect_raw_response: false,
                retention_days: 7,
            },
            observation_coverage: Vec::new(),
            suggested_control_bindings: Vec::<ControlBindingPlan>::new(),
            telemetry_plan: TelemetryPlan {
                events_endpoint: "/events".into(),
                metrics_endpoint: "/metrics".into(),
                capture_tool_calls: true,
                capture_arguments: false,
                redact_env_keys: vec![],
                risk_signals: vec![],
            },
            labels: BTreeMap::new(),
        }
    }
}
