//! Read-model construction: query the store, build the entity graph via
//! GraphBuilder, assemble the activity timeline, and derive the entity-360
//! view; plus graph filtering by query params.

use super::*;

pub(super) async fn build_read_model(state: &AppState, tenant: &str) -> anyhow::Result<ReadModel> {
    let mut builder = GraphBuilder::default();
    let mut activity = Vec::new();

    let agents = state
        .registry_store
        .list_agents(tenant)
        .await
        .unwrap_or_default();
    let tools = state
        .registry_store
        .list_tools(tenant)
        .await
        .unwrap_or_default();
    let resources = state
        .registry_store
        .list_resources(tenant)
        .await
        .unwrap_or_default();
    let entities = state
        .registry_store
        .list_entities(tenant)
        .await
        .unwrap_or_default();
    let providers = state
        .registry_store
        .list_blackbox_ai(tenant)
        .await
        .unwrap_or_default();
    let relationships = state
        .registry_store
        .list_relationships(tenant)
        .await
        .unwrap_or_default();
    let policies = state
        .policy_store
        .list_policies(tenant)
        .await
        .unwrap_or_default();
    let observations = state
        .observability_store
        .list_observation_events(tenant)
        .await
        .unwrap_or_default();
    let usage_events = state
        .observability_store
        .list_ai_usage_events(AiUsageQuery {
            tenant_id: tenant.to_string(),
            limit: Some(250),
            ..AiUsageQuery::default()
        })
        .await
        .unwrap_or_default();
    let mut guard_events = state
        .telemetry_store
        .list_telemetry(tenant, "guard_incident")
        .await
        .unwrap_or_default();
    if let Ok(mut events) = state
        .telemetry_store
        .list_telemetry(tenant, "guard_event")
        .await
    {
        guard_events.append(&mut events);
    }

    for agent in agents {
        let raw = serde_json::to_value(&agent).unwrap_or(Value::Null);
        let agent_id = string_path(&raw, &["agent_id"]).unwrap_or_default();
        let label = string_path(&raw, &["name"]).unwrap_or_else(|| agent_id.clone());
        let runtime = string_path(&raw, &["runtime", "runtime_name"]);
        let agent_type = string_path(&raw, &["agent_type"]);
        let status = string_path(&raw, &["enforcement_mode"])
            .or_else(|| string_path(&raw, &["meta", "status"]))
            .unwrap_or_else(|| "registered".to_string());
        let mut badges = Vec::new();
        if string_path(&raw, &["identity", "spiffe_id"]).is_some() {
            badges.push("SPIFFE bound".to_string());
        }
        if let Some(bindings) = raw
            .get("identity")
            .and_then(|v| v.get("token_bindings"))
            .and_then(Value::as_array)
        {
            if !bindings.is_empty() {
                badges.push(format!("{} token binding(s)", bindings.len()));
            }
        }
        if let Some(trust) = string_path(&raw, &["trust_level"]) {
            badges.push(format!("Trust: {trust}"));
        }
        builder.add_node(GraphNode {
            id: node_key("agent", &agent_id),
            node_type: "agent".to_string(),
            entity_id: agent_id.clone(),
            label,
            subtitle: runtime.or(agent_type),
            status: status.to_lowercase(),
            risk: string_path(&raw, &["trust_level"]),
            mode: string_path(&raw, &["enforcement_mode"]).map(|m| m.to_lowercase()),
            badges,
            metrics: vec![
                GraphMetric {
                    label: "Tools".to_string(),
                    value: array_strings(&raw, &["declared_tools"]).len().to_string(),
                },
                GraphMetric {
                    label: "Resources".to_string(),
                    value: array_strings(&raw, &["declared_resources"])
                        .len()
                        .to_string(),
                },
            ],
            href: route_for("agent", &agent_id),
            raw: Some(raw.clone()),
        });
        for tool_id in array_strings(&raw, &["declared_tools"]) {
            builder.add_edge(
                "agent",
                &agent_id,
                "tool",
                &tool_id,
                "uses",
                "Declared by registered agent",
                false,
                false,
            );
        }
        for resource_id in array_strings(&raw, &["declared_resources"]) {
            builder.add_edge(
                "agent",
                &agent_id,
                "resource",
                &resource_id,
                "accesses",
                "Declared by registered agent",
                false,
                false,
            );
        }
        if let Some(spiffe) = string_path(&raw, &["identity", "spiffe_id"]) {
            builder.add_edge(
                "agent",
                &agent_id,
                "identity",
                &spiffe,
                "bound_to",
                "SPIFFE identity binding",
                false,
                false,
            );
        }
    }

    for tool in tools {
        let raw = serde_json::to_value(&tool).unwrap_or(Value::Null);
        let tool_id = string_path(&raw, &["tool_id"]).unwrap_or_default();
        let label = string_path(&raw, &["name"]).unwrap_or_else(|| tool_id.clone());
        builder.add_node(GraphNode {
            id: node_key("tool", &tool_id),
            node_type: "tool".to_string(),
            entity_id: tool_id.clone(),
            label,
            subtitle: string_path(&raw, &["description"])
                .or_else(|| string_path(&raw, &["category"])),
            status: string_path(&raw, &["meta", "status"])
                .unwrap_or_else(|| "registered".to_string()),
            risk: string_path(&raw, &["risk_level"]),
            mode: None,
            badges: compact_badges(vec![
                string_path(&raw, &["category"]),
                string_path(&raw, &["data_access_level"]).map(|v| format!("Data: {v}")),
                string_path(&raw, &["side_effect_level"]).map(|v| format!("Effects: {v}")),
            ]),
            metrics: Vec::new(),
            href: route_for("tool", &tool_id),
            raw: Some(raw),
        });
    }

    for resource in resources {
        let raw = serde_json::to_value(&resource).unwrap_or(Value::Null);
        let resource_id = string_path(&raw, &["resource_id"])
            .or_else(|| string_path(&raw, &["id"]))
            .or_else(|| string_path(&raw, &["uri"]))
            .unwrap_or_default();
        let label = string_path(&raw, &["name"]).unwrap_or_else(|| resource_id.clone());
        builder.add_node(GraphNode {
            id: node_key("resource", &resource_id),
            node_type: "resource".to_string(),
            entity_id: resource_id.clone(),
            label,
            subtitle: string_path(&raw, &["uri"]).or_else(|| string_path(&raw, &["resource_type"])),
            status: string_path(&raw, &["meta", "status"])
                .unwrap_or_else(|| "registered".to_string()),
            risk: string_path(&raw, &["classification"]),
            mode: None,
            badges: compact_badges(vec![
                string_path(&raw, &["resource_type"]),
                string_path(&raw, &["classification"]).map(|v| format!("Class: {v}")),
            ]),
            metrics: Vec::new(),
            href: route_for("resource", &resource_id),
            raw: Some(raw),
        });
    }

    for entity in entities {
        let raw = serde_json::to_value(&entity).unwrap_or(Value::Null);
        let entity_id = string_path(&raw, &["entity_id"]).unwrap_or_default();
        let label = string_path(&raw, &["display_name"]).unwrap_or_else(|| entity_id.clone());
        let kind = string_path(&raw, &["entity_type"]).unwrap_or_else(|| "identity".to_string());
        builder.add_node(GraphNode {
            id: node_key("identity", &entity_id),
            node_type: "identity".to_string(),
            entity_id: entity_id.clone(),
            label,
            subtitle: Some(kind.clone()),
            status: string_path(&raw, &["meta", "status"])
                .unwrap_or_else(|| "registered".to_string()),
            risk: None,
            mode: None,
            badges: compact_badges(vec![
                Some(kind),
                raw.get("roles")
                    .and_then(Value::as_array)
                    .map(|roles| format!("{} role(s)", roles.len())),
            ]),
            metrics: Vec::new(),
            href: route_for("identity", &entity_id),
            raw: Some(raw),
        });
    }

    for provider in providers {
        let raw = serde_json::to_value(&provider).unwrap_or(Value::Null);
        let provider_id = string_path(&raw, &["provider_id"]).unwrap_or_default();
        let label = string_path(&raw, &["name"]).unwrap_or_else(|| provider_id.clone());
        builder.add_node(GraphNode {
            id: node_key("provider", &provider_id),
            node_type: "provider".to_string(),
            entity_id: provider_id.clone(),
            label,
            subtitle: string_path(&raw, &["provider_type"]),
            status: string_path(&raw, &["meta", "status"])
                .unwrap_or_else(|| "registered".to_string()),
            risk: string_path(&raw, &["trust_level"]),
            mode: None,
            badges: compact_badges(vec![
                string_path(&raw, &["provider_type"]),
                string_path(&raw, &["trust_level"]).map(|v| format!("Trust: {v}")),
            ]),
            metrics: Vec::new(),
            href: route_for("provider", &provider_id),
            raw: Some(raw),
        });
    }

    for policy in policies {
        let raw = serde_json::to_value(&policy).unwrap_or(Value::Null);
        let policy_id = string_path(&raw, &["policy_id"]).unwrap_or_default();
        let label = string_path(&raw, &["name"]).unwrap_or_else(|| policy_id.clone());
        builder.add_node(GraphNode {
            id: node_key("policy", &policy_id),
            node_type: "policy".to_string(),
            entity_id: policy_id.clone(),
            label,
            subtitle: string_path(&raw, &["description"])
                .or_else(|| string_path(&raw, &["policy_type"])),
            status: string_path(&raw, &["meta", "status"]).unwrap_or_else(|| "draft".to_string()),
            risk: None,
            mode: Some("govern".to_string()),
            badges: compact_badges(vec![
                string_path(&raw, &["policy_type"]),
                string_path(&raw, &["meta", "source"]).map(|v| format!("Source: {v}")),
            ]),
            metrics: vec![GraphMetric {
                label: "Targets".to_string(),
                value: target_count(&raw).to_string(),
            }],
            href: route_for("policy", &policy_id),
            raw: Some(raw.clone()),
        });
        for agent_id in array_strings(&raw, &["targets", "agent_ids"]) {
            builder.add_edge(
                "policy",
                &policy_id,
                "agent",
                &agent_id,
                "governs",
                "Policy target: agent",
                false,
                true,
            );
        }
        for tool_id in array_strings(&raw, &["targets", "tool_ids"]) {
            builder.add_edge(
                "policy",
                &policy_id,
                "tool",
                &tool_id,
                "governs",
                "Policy target: tool",
                false,
                true,
            );
        }
        for resource_id in array_strings(&raw, &["targets", "resource_ids"]) {
            builder.add_edge(
                "policy",
                &policy_id,
                "resource",
                &resource_id,
                "protects",
                "Policy target: resource",
                false,
                true,
            );
        }
        for entity_id in array_strings(&raw, &["targets", "entity_ids"]) {
            builder.add_edge(
                "policy",
                &policy_id,
                "identity",
                &entity_id,
                "governs",
                "Policy target: identity",
                false,
                true,
            );
        }
    }

    for relationship in relationships {
        let raw = serde_json::to_value(&relationship).unwrap_or(Value::Null);
        let subject_type = string_path(&raw, &["subject", "object_type"]).unwrap_or_default();
        let subject_id = string_path(&raw, &["subject", "object_id"]).unwrap_or_default();
        let object_type = string_path(&raw, &["object", "object_type"]).unwrap_or_default();
        let object_id = string_path(&raw, &["object", "object_id"]).unwrap_or_default();
        let relation = string_path(&raw, &["relation"]).unwrap_or_else(|| "related_to".to_string());
        builder.add_edge(
            &subject_type,
            &subject_id,
            &object_type,
            &object_id,
            &relation,
            "Registered relationship",
            false,
            false,
        );
    }

    for event in observations {
        let actor_id = event
            .agent_id
            .clone()
            .or_else(|| event.shadow_candidate_id.clone())
            .unwrap_or_else(|| "unknown-agent".to_string());
        if let Some(label) = system_actor_label(&actor_id) {
            builder.ensure_node("agent", &actor_id, label, "Pollek system activity");
        } else if event.agent_id.is_none() && event.shadow_candidate_id.is_none() {
            builder.ensure_node("agent", &actor_id, "Unknown AI app", "Observed activity");
        }

        let tool_id = event
            .tool_id
            .clone()
            .or_else(|| event.tool_call.as_ref().map(|tool| tool.tool_name.clone()));
        let resource_id = event.resource_id.clone().or_else(|| {
            event
                .resource_access
                .as_ref()
                .map(|resource| resource.target_redacted.clone())
        });
        if let (Some(resource_id), Some(resource)) = (&resource_id, &event.resource_access) {
            if resource.resource_type == "plugin" {
                builder.ensure_node(
                    "resource",
                    resource_id,
                    &resource.target_redacted,
                    "Plugin marketplace audit event",
                );
            }
        }

        if let Some(tool_id) = &tool_id {
            builder.add_edge(
                "agent",
                &actor_id,
                "tool",
                tool_id,
                "uses",
                "Observed tool invocation",
                true,
                decision_enforced(&event),
            );
        }
        if let Some(resource_id) = &resource_id {
            builder.add_edge(
                "agent",
                &actor_id,
                "resource",
                resource_id,
                "accesses",
                "Observed resource access",
                true,
                decision_enforced(&event),
            );
        }
        if let (Some(tool_id), Some(resource_id)) = (&tool_id, &resource_id) {
            builder.add_edge(
                "tool",
                tool_id,
                "resource",
                resource_id,
                "touches",
                "Observed through tool activity",
                true,
                decision_enforced(&event),
            );
        }
        let policy_ids = event
            .decision
            .as_ref()
            .map(|decision| decision.matched_policy_ids.clone())
            .unwrap_or_default();
        for policy_id in &policy_ids {
            builder.add_edge(
                "policy",
                policy_id,
                "agent",
                &actor_id,
                "matched_policy",
                "Matched in observed decision",
                true,
                decision_enforced(&event),
            );
        }
        activity.push(activity_from_observation(
            &builder.nodes,
            &event,
            &actor_id,
            tool_id,
            resource_id,
        ));
    }

    for event in guard_events {
        let (actor_id, resource_id, label) = guard_event_refs(&event);
        builder.ensure_node(
            "agent",
            &actor_id,
            &actor_id,
            "Prompt Guard incident source",
        );
        builder.ensure_node(
            "resource",
            &resource_id,
            &label,
            "Prompt Guard safety category",
        );
        builder.add_edge(
            "agent",
            &actor_id,
            "resource",
            &resource_id,
            "guarded",
            "Prompt Guard incident",
            true,
            guard_event_action(&event) == "deny",
        );
        activity.push(activity_from_guard_event(&builder.nodes, &event));
    }

    for event in usage_events {
        let agent_id = event
            .agent_id
            .clone()
            .or_else(|| event.shadow_candidate_id.clone())
            .unwrap_or_else(|| "unknown-agent".to_string());
        if let Some(provider) = &event.provider {
            builder.add_edge(
                "agent",
                &agent_id,
                "provider",
                provider,
                "uses_provider",
                "AI usage event",
                true,
                event.control_mode.as_deref() == Some("enforce"),
            );
        }
        if let Some(model) = &event.model {
            builder.ensure_node("model", model, model, "AI model seen in usage telemetry");
            builder.add_edge(
                "agent",
                &agent_id,
                "model",
                model,
                "uses_model",
                "AI usage event",
                true,
                event.control_mode.as_deref() == Some("enforce"),
            );
        }
        if let Some(tool_id) = &event.tool_id {
            builder.add_edge(
                "agent",
                &agent_id,
                "tool",
                tool_id,
                "uses",
                "AI usage event",
                true,
                event.control_mode.as_deref() == Some("enforce"),
            );
        }
        if let Some(resource_id) = &event.resource_id {
            builder.add_edge(
                "agent",
                &agent_id,
                "resource",
                resource_id,
                "accesses",
                "AI usage event",
                true,
                event.control_mode.as_deref() == Some("enforce"),
            );
        }
        for policy_id in &event.policy_ids {
            builder.add_edge(
                "policy",
                policy_id,
                "agent",
                &agent_id,
                "matched_policy",
                "AI usage policy reference",
                true,
                event.control_mode.as_deref() == Some("enforce"),
            );
        }
        activity.push(activity_from_usage(&builder.nodes, &event, &agent_id));
    }

    let graph = builder.finish(tenant, None);
    Ok(ReadModel { graph, activity })
}

pub(super) fn build_entity_360(
    model: ReadModel,
    tenant: &str,
    entity_type: &str,
    entity_id: &str,
) -> Option<Entity360Response> {
    let normalized_type = normalize_type(entity_type);
    let center_key = node_key(&normalized_type, entity_id);
    let center = model
        .graph
        .nodes
        .iter()
        .find(|node| node.id == center_key)
        .cloned()?;

    let mut keep = BTreeSet::new();
    keep.insert(center.id.clone());
    for edge in &model.graph.edges {
        if edge.source == center.id {
            keep.insert(edge.target.clone());
        }
        if edge.target == center.id {
            keep.insert(edge.source.clone());
        }
    }

    let nodes: Vec<_> = model
        .graph
        .nodes
        .iter()
        .filter(|node| keep.contains(&node.id))
        .cloned()
        .collect();
    let edges: Vec<_> = model
        .graph
        .edges
        .iter()
        .filter(|edge| keep.contains(&edge.source) && keep.contains(&edge.target))
        .cloned()
        .collect();
    let summaries = summaries_from_nodes_edges(&nodes, &edges);
    let warnings = coverage_warnings(&nodes, &edges);
    let activity: Vec<_> = model
        .activity
        .into_iter()
        .filter(|item| activity_matches_entity(item, &normalized_type, entity_id))
        .take(50)
        .collect();
    let generated_at = chrono::Utc::now().to_rfc3339();
    let graph = EntityGraphResponse {
        schema_version: "entity-graph.v1".to_string(),
        tenant_id: tenant.to_string(),
        generated_at: generated_at.clone(),
        center: Some(center.clone()),
        nodes,
        edges,
        summaries: summaries.clone(),
        warnings: warnings.clone(),
    };

    Some(Entity360Response {
        schema_version: "entity-360.v1".to_string(),
        tenant_id: tenant.to_string(),
        generated_at,
        entity: center,
        graph,
        summaries,
        activity,
        warnings,
    })
}

pub(super) fn filter_graph(
    mut graph: EntityGraphResponse,
    query: &GraphQuery,
) -> EntityGraphResponse {
    let allowed_types: Option<BTreeSet<String>> = query.types.as_ref().map(|types| {
        types
            .split(',')
            .map(|item| normalize_type(item.trim()))
            .collect::<BTreeSet<_>>()
    });
    let statuses: Option<BTreeSet<String>> = query.status.as_ref().map(|statuses| {
        statuses
            .split(',')
            .map(|item| item.trim().to_lowercase())
            .collect::<BTreeSet<_>>()
    });
    let search = query.q.as_ref().map(|q| q.to_lowercase());
    let limit = query.limit.unwrap_or(500).min(1000);

    graph.nodes = graph
        .nodes
        .into_iter()
        .filter(|node| {
            allowed_types
                .as_ref()
                .map(|types| types.contains(&node.node_type))
                .unwrap_or(true)
                && statuses
                    .as_ref()
                    .map(|values| values.contains(&node.status.to_lowercase()))
                    .unwrap_or(true)
                && search
                    .as_ref()
                    .map(|q| {
                        node.label.to_lowercase().contains(q)
                            || node.entity_id.to_lowercase().contains(q)
                            || node
                                .subtitle
                                .as_ref()
                                .map(|subtitle| subtitle.to_lowercase().contains(q))
                                .unwrap_or(false)
                    })
                    .unwrap_or(true)
        })
        .take(limit)
        .collect();
    let keep: BTreeSet<_> = graph.nodes.iter().map(|node| node.id.clone()).collect();
    graph
        .edges
        .retain(|edge| keep.contains(&edge.source) && keep.contains(&edge.target));
    graph.summaries = summaries_from_nodes_edges(&graph.nodes, &graph.edges);
    graph.warnings = coverage_warnings(&graph.nodes, &graph.edges);
    graph
}
