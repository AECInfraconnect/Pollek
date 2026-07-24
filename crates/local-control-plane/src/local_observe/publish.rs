//! Candidate -> observation publishing pipeline: upsert a discovered
//! candidate, derive its resource seeds, and publish agent-observation and
//! resource-access events into the read model (with detail-map + path helpers).

use super::*;

pub(super) async fn upsert_candidate(
    state: &AppState,
    tenant: &str,
    candidate: &DiscoveredAgentCandidateV2,
) -> anyhow::Result<()> {
    let mut candidate = candidate.clone();
    if let Some(existing_raw) = state
        .registry_store
        .get_raw(tenant, "discovery_candidate", &candidate.candidate_id)
        .await?
    {
        if let Ok(existing) = serde_json::from_value::<DiscoveredAgentCandidateV2>(existing_raw) {
            candidate.first_seen = existing.first_seen;
            for scan_id in existing.scan_ids {
                if !candidate.scan_ids.iter().any(|id| id == &scan_id) {
                    candidate.scan_ids.push(scan_id);
                }
            }
        }
    }
    state
        .registry_store
        .upsert_raw(
            tenant,
            "discovery_candidate",
            &candidate.candidate_id,
            &serde_json::to_value(&candidate)?,
        )
        .await?;
    Ok(())
}

pub(super) async fn publish_candidate_observations(
    state: &AppState,
    tenant: &str,
    candidate: &DiscoveredAgentCandidateV2,
    _scan_id: &str,
    counts: &mut ObserveCounts,
) {
    let agent_id = canonical_agent_id(candidate);
    let agent_label = candidate.display_name.clone();
    let now = Utc::now();
    let bucket = scan_bucket();
    let spiffe_id = format!("spiffe://local/pollek/agent/{}", agent_id);

    let identity_payload = json!({
        "agent_id": agent_id.clone(),
        "agent_label": agent_label.clone(),
        "scope": "local",
        "identity_kind": "spiffe_id",
        "identity_id": spiffe_id,
        "identity_label": format!("{} local workload identity", candidate.display_name),
        "provider": candidate.vendor.clone(),
        "spiffe_id": format!("spiffe://local/pollek/agent/{}", agent_id),
        "action": "access",
        "decision": "observed",
        "enforced_for_real": false,
        "observed_at": now,
    });
    publish_payload(
        state,
        tenant,
        "identity_access",
        &stable_event_id("identity", &[tenant, &agent_id, &bucket]),
        identity_payload,
        true,
    )
    .await;
    counts.identity_events += 1;

    for resource in resources_for_candidate(candidate) {
        let event_id = stable_event_id(
            "resource",
            &[tenant, &agent_id, &resource.target_redacted, &bucket],
        );
        let payload = json!({
            "agent_id": agent_id.clone(),
            "agent_label": agent_label.clone(),
            "scope": resource.scope,
            "kind": resource.kind,
            "target_redacted": resource.target_redacted,
            "target_hash": resource.target_hash,
            "mode": resource.mode,
            "decision": "observed",
            "control_method": resource.control_method,
            "enforced_for_real": false,
            "bytes": resource.bytes,
            "count": 1,
            "classification": resource.classification,
            "details": resource.details,
            "observed_at": now,
        });
        record_resource_observation(state, tenant, &event_id, &payload).await;
        publish_payload(state, tenant, "resource_access", &event_id, payload, true).await;
        counts.resource_events += 1;
    }

    for server in &candidate.discovered_mcp_servers {
        let tool_name = if server.server_name.is_empty() {
            "mcp-server"
        } else {
            &server.server_name
        };
        publish_payload(
            state,
            tenant,
            "tool_usage",
            &stable_event_id("tool", &[tenant, &agent_id, tool_name, &bucket]),
            json!({
                "agent_id": agent_id.clone(),
                "agent_label": agent_label.clone(),
                "tool_kind": "mcp_tool",
                "tool_name": tool_name,
                "server": server.transport,
                "decision": "observed",
                "enforced_for_real": false,
                "args_redacted": "<not captured by discovery>",
                "observed_at": now,
            }),
            true,
        )
        .await;
        counts.tool_events += 1;
    }
}

#[derive(Debug, Clone)]
pub(super) struct ObservedResourceSeed {
    scope: &'static str,
    kind: &'static str,
    target_redacted: String,
    target_hash: String,
    mode: &'static str,
    classification: Option<String>,
    control_method: Option<&'static str>,
    bytes: Option<i64>,
    details: Value,
}

pub(super) struct ResourceSeedSpec {
    scope: &'static str,
    kind: &'static str,
    target_redacted: String,
    mode: &'static str,
    classification: Option<String>,
    control_method: Option<&'static str>,
    trace_source: &'static str,
}

pub(super) fn resources_for_candidate(
    candidate: &DiscoveredAgentCandidateV2,
) -> Vec<ObservedResourceSeed> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for endpoint in &candidate.discovered_endpoints {
        push_resource(
            &mut out,
            &mut seen,
            ResourceSeedSpec {
                scope: "local",
                kind: "api",
                target_redacted: endpoint.url.clone(),
                mode: "connect",
                classification: Some("Local model/API endpoint".into()),
                control_method: None,
                trace_source: "discovered_endpoint",
            },
        );
    }

    for path in &candidate.suggested_registration.mcp_stdio_config_paths {
        push_resource(
            &mut out,
            &mut seen,
            ResourceSeedSpec {
                scope: "local",
                kind: "config",
                target_redacted: path.clone(),
                mode: "read",
                classification: Some("MCP configuration".into()),
                control_method: Some("mcp_stdio"),
                trace_source: "mcp_config_path",
            },
        );
    }

    for ev in &candidate.evidence {
        push_resources_from_evidence(&mut out, &mut seen, ev);
    }

    out
}

pub(super) fn push_resources_from_evidence(
    out: &mut Vec<ObservedResourceSeed>,
    seen: &mut HashSet<String>,
    ev: &DiscoveryEvidenceV2,
) {
    let data = &ev.data;
    if let Some(target) = data
        .get("origin")
        .and_then(Value::as_str)
        .or_else(|| data.get("url").and_then(Value::as_str))
        .or_else(|| data.get("matched_domain").and_then(Value::as_str))
        .or_else(|| data.get("domain").and_then(Value::as_str))
    {
        let target = normalize_target(target);
        let kind = match ev.source {
            EvidenceSource::NetworkSni => "api",
            EvidenceSource::BrowserSession
            | EvidenceSource::BrowserWindow
            | EvidenceSource::BrowserHistory => "web",
            _ => "api",
        };
        push_resource(
            out,
            seen,
            ResourceSeedSpec {
                scope: "cloud",
                kind,
                target_redacted: target,
                mode: "connect",
                classification: data
                    .get("base_name")
                    .and_then(Value::as_str)
                    .or_else(|| data.get("name").and_then(Value::as_str))
                    .map(str::to_string),
                control_method: None,
                trace_source: evidence_source_name(&ev.source),
            },
        );
    }

    if matches!(ev.source, EvidenceSource::McpConfig) {
        if let Some(path) = &ev.source_path_redacted {
            push_resource(
                out,
                seen,
                ResourceSeedSpec {
                    scope: "local",
                    kind: "config",
                    target_redacted: path.clone(),
                    mode: "read",
                    classification: Some("MCP configuration".into()),
                    control_method: Some("mcp_stdio"),
                    trace_source: "mcp_config_path",
                },
            );
        }
    } else if let Some(path) = &ev.source_path_redacted {
        if matches!(
            ev.source,
            EvidenceSource::ProcessScan
                | EvidenceSource::CliAgent
                | EvidenceSource::InstalledAppScan
                | EvidenceSource::IdeExtension
        ) {
            push_resource(
                out,
                seen,
                ResourceSeedSpec {
                    scope: "local",
                    kind: "process",
                    target_redacted: path.clone(),
                    mode: "execute",
                    classification: Some(format!("{:?}", ev.source)),
                    control_method: None,
                    trace_source: evidence_source_name(&ev.source),
                },
            );
        }
    }
}

pub(super) fn push_resource(
    out: &mut Vec<ObservedResourceSeed>,
    seen: &mut HashSet<String>,
    spec: ResourceSeedSpec,
) {
    let ResourceSeedSpec {
        scope,
        kind,
        target_redacted,
        mode,
        classification,
        control_method,
        trace_source,
    } = spec;
    let key = format!("{scope}:{kind}:{target_redacted}:{mode}");
    if !seen.insert(key) {
        return;
    }
    out.push(ObservedResourceSeed {
        scope,
        kind,
        target_hash: hash_hex(&target_redacted),
        details: resource_trace_details(kind, &target_redacted, trace_source, control_method),
        target_redacted,
        mode,
        classification,
        control_method,
        bytes: None,
    });
}

pub(super) fn evidence_source_name(source: &EvidenceSource) -> &'static str {
    match source {
        EvidenceSource::NetworkSni => "network_sni",
        EvidenceSource::BrowserSession => "browser_session",
        EvidenceSource::BrowserWindow => "browser_window",
        EvidenceSource::BrowserHistory => "browser_history",
        EvidenceSource::McpConfig => "mcp_config_path",
        EvidenceSource::ProcessScan => "process_scan_path",
        EvidenceSource::CliAgent => "cli_agent_path",
        EvidenceSource::InstalledAppScan => "installed_app_path",
        EvidenceSource::IdeExtension => "ide_extension_path",
        _ => "discovery_evidence",
    }
}

pub(super) fn resource_trace_details(
    kind: &str,
    target: &str,
    trace_source: &str,
    control_method: Option<&str>,
) -> Value {
    let mut details = Map::new();
    details.insert("trace_source".to_string(), json!(trace_source));
    details.insert("capture_quality".to_string(), json!("observed_metadata"));
    details.insert("raw_content_stored".to_string(), json!(false));
    if let Some(method) = control_method {
        details.insert("control_method".to_string(), json!(method));
    }

    if matches!(
        kind,
        "config" | "process" | "file" | "folder" | "database_local"
    ) {
        merge_detail_map(&mut details, path_trace_details(target));
    } else if target.contains("://") || looks_like_host(target) {
        details.insert("host".to_string(), json!(host_part(target)));
        details.insert("trace_granularity".to_string(), json!("host"));
    }

    Value::Object(details)
}

pub(super) fn path_trace_details(path: &str) -> Map<String, Value> {
    let mut details = Map::new();
    let trimmed = path.trim();
    let parts = path_parts(trimmed);
    let leaf = parts.last().copied().unwrap_or(trimmed);
    let extension = leaf
        .rsplit_once('.')
        .map(|(_, ext)| ext.trim())
        .filter(|ext| !ext.is_empty());
    let looks_file = extension.is_some()
        || trimmed.ends_with(".exe")
        || trimmed.ends_with(".json")
        || trimmed.ends_with(".jsonl")
        || trimmed.ends_with(".yaml")
        || trimmed.ends_with(".yml");
    let path_kind = if looks_file { "file" } else { "folder" };
    details.insert("path_kind".to_string(), json!(path_kind));
    details.insert("resource_name".to_string(), json!(leaf));
    details.insert(
        "trace_granularity".to_string(),
        json!(if looks_file {
            "file_path"
        } else {
            "folder_path"
        }),
    );
    if looks_file {
        details.insert("file_name".to_string(), json!(leaf));
        if let Some(ext) = extension {
            details.insert(
                "file_extension".to_string(),
                json!(ext.to_ascii_lowercase()),
            );
        }
        if parts.len() > 1 {
            let folder = join_path_parts(trimmed, &parts[..parts.len() - 1]);
            details.insert("folder_path".to_string(), json!(folder));
            if let Some(folder_name) = parts.get(parts.len().saturating_sub(2)) {
                details.insert("folder_name".to_string(), json!(folder_name));
            }
        }
    } else {
        details.insert("folder_name".to_string(), json!(leaf));
        details.insert("folder_path".to_string(), json!(trimmed));
    }

    if let Some(system) = database_system_from_path(leaf) {
        details.insert("db_system".to_string(), json!(system));
        details.insert("db_namespace".to_string(), json!(leaf));
        details.insert("trace_granularity".to_string(), json!("database_file"));
    }
    details
}

pub(super) fn path_parts(path: &str) -> Vec<&str> {
    path.trim_matches(['\\', '/'])
        .split(['\\', '/'])
        .filter(|part| !part.is_empty())
        .collect()
}

pub(super) fn join_path_parts(original: &str, parts: &[&str]) -> String {
    let separator = if original.contains('\\') { "\\" } else { "/" };
    let prefix = if original.starts_with('\\') || original.starts_with('/') {
        separator
    } else {
        ""
    };
    format!("{prefix}{}", parts.join(separator))
}

pub(super) fn database_system_from_path(file_name: &str) -> Option<&'static str> {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".sqlite") || lower.ends_with(".sqlite3") || lower.ends_with(".db") {
        Some("sqlite")
    } else if lower.ends_with(".duckdb") {
        Some("duckdb")
    } else {
        None
    }
}

pub(super) fn looks_like_host(value: &str) -> bool {
    let value = value.trim();
    value.contains('.') && !value.contains(['\\', '/', ' '])
}

pub(super) fn host_part(value: &str) -> String {
    value
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(value)
        .to_string()
}

pub(super) fn merge_detail_map(target: &mut Map<String, Value>, source: Map<String, Value>) {
    for (key, value) in source {
        target.entry(key).or_insert(value);
    }
}

/// Records a resource-access trace as a real `AgentObservationEvent` so it
/// reaches the activity read model behind the AI Activity page (Files / Web /
/// Email / Commands tiles). Telemetry envelopes alone never get there — they
/// only feed the inventory and cloud-sync paths. Event ids are stable, so a
/// re-scan of the same log line is a no-op (primary-key insert).
pub(super) async fn record_resource_observation(
    state: &AppState,
    tenant: &str,
    event_id: &str,
    payload: &Value,
) {
    let string_field = |key: &str| payload.get(key).and_then(Value::as_str).map(str::to_string);
    let target = string_field("target_redacted").unwrap_or_else(|| "unknown".to_string());
    let kind = string_field("kind").unwrap_or_else(|| "resource".to_string());
    let mode = string_field("mode").unwrap_or_else(|| "read".to_string());
    let event = AgentObservationEvent {
        process_signal: None,
        event_id: format!("obs_{event_id}"),
        tenant_id: tenant.to_string(),
        trace_id: event_id.to_string(),
        agent_id: string_field("agent_id"),
        shadow_candidate_id: None,
        tool_id: None,
        resource_id: Some(target.clone()),
        surface: "local_observe".to_string(),
        action: format!("{kind}.{mode}"),
        pep_type: Some("local_log_reader".to_string()),
        risk_level: None,
        timestamp: string_field("observed_at").unwrap_or_else(|| Utc::now().to_rfc3339()),
        payload_json: payload.to_string(),
        token_usage: None,
        browser_scope: None,
        event_kind: EventKind::ResourceAccess,
        decision: None,
        tool_call: None,
        resource_access: Some(ResourceAccess {
            resource_type: kind,
            target_redacted: target,
            bytes: payload.get("bytes").and_then(Value::as_i64),
            verb: mode,
        }),
        latency_ms: None,
        provider: None,
    };
    if let Err(err) = state
        .observability_store
        .insert_observation_event(&event)
        .await
    {
        // A duplicate stable id from a re-scan is expected; anything else is
        // worth a debug note but must not break the observe refresh.
        tracing::debug!(error = %err, "resource observation insert skipped");
    }
}

pub(super) async fn publish_payload(
    state: &AppState,
    tenant: &str,
    event_type: &str,
    event_id: &str,
    payload: Value,
    redaction_applied: bool,
) {
    let payload = match payload {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    let envelope = pollek_contract::PollekTelemetryEnvelopeV1 {
        schema_version: "telemetry-envelope.v1".to_string(),
        event_id: event_id.to_string(),
        event_type: event_type.to_string(),
        timestamp: Utc::now(),
        tenant_id: tenant.to_string(),
        workspace_id: Some("default".to_string()),
        environment_id: Some(state.identity.environment_id.clone()),
        device_id: local_device_id(),
        trace_id: None,
        span_id: None,
        redaction_applied,
        payload,
    };
    let _ = crate::usage_api::publish_telemetry_envelope(state, envelope).await;
}
