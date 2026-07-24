use crate::state::AppState;
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use chrono::{DateTime, Utc};
use dek_agent_discovery::model::{
    DiscoveredAgentCandidateV2, DiscoveryEvidenceV2, EvidenceSource, InferredAgentType,
};
use dek_agent_observer::{
    model::{AgentObservationEvent, EventKind, ResourceAccess},
    usage_model::{
        AgentType, AiUsageEventKind, AiUsageEventV1, CanonicalCostBreakdown, CanonicalTokenUsage,
        UsageSource,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

mod resource_trace;
mod usage_extract;
mod util;
mod value;
use resource_trace::*;
use usage_extract::*;
use util::*;
use value::*;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/v1/tenants/:tenant/local-observe/refresh",
        post(refresh_local_observe),
    )
}

#[derive(Debug, Clone, Deserialize)]
struct LocalObserveRefreshRequest {
    #[serde(default = "default_include_estimates")]
    include_estimates: bool,
    #[serde(default)]
    sources: Option<Vec<String>>,
}

fn default_include_estimates() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
struct LocalObserveRefreshResponse {
    schema_version: String,
    tenant_id: String,
    scan_id: String,
    candidates_found: usize,
    resource_events: usize,
    identity_events: usize,
    tool_events: usize,
    usage_events: usize,
    exact_usage_events: usize,
    estimated_usage_events: usize,
    capture_quality: Vec<String>,
    limitations: Vec<String>,
    next_steps: Vec<LocalObserveNextStep>,
}

#[derive(Debug, Clone, Serialize)]
struct LocalObserveNextStep {
    action_id: String,
    title: String,
    reason: String,
    route: String,
}

#[derive(Default)]
struct ObserveCounts {
    resource_events: usize,
    identity_events: usize,
    tool_events: usize,
    usage_events: usize,
    exact_usage_events: usize,
    estimated_usage_events: usize,
    capture_quality: BTreeSet<String>,
}

async fn refresh_local_observe(
    State(state): State<AppState>,
    AxumPath(tenant): AxumPath<String>,
    Json(req): Json<LocalObserveRefreshRequest>,
) -> impl IntoResponse {
    let scan_id = format!("local_observe_{}", Utc::now().timestamp_millis());
    let scan_req = json!({
        "sources": req.sources.unwrap_or_else(default_sources),
        "reason": "local_observe_refresh"
    });

    let scan = dek_agent_discovery::run_scan_v2(
        &tenant,
        &scan_id,
        &scan_req,
        None,
        None,
        state.def_store.get(),
    )
    .await;

    let (job, candidates) = match scan {
        Ok(result) => result,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": err.to_string(),
                    "schema_version": "local-observe-refresh.v1",
                    "scan_id": scan_id
                })),
            );
        }
    };

    let _ = state
        .registry_store
        .upsert_raw(
            &tenant,
            "discovery_scan",
            &job.scan_id,
            &serde_json::to_value(&job).unwrap_or_else(|_| json!({})),
        )
        .await;

    let mut counts = ObserveCounts::default();
    let mut exact_agents = HashSet::new();

    for candidate in &candidates {
        let _ = upsert_candidate(&state, &tenant, candidate).await;
        publish_candidate_observations(&state, &tenant, candidate, &scan_id, &mut counts).await;
    }

    bridge_exact_usage_from_telemetry(&state, &tenant, &mut counts, &mut exact_agents).await;
    bridge_exact_usage_from_local_logs(&state, &tenant, &mut counts, &mut exact_agents).await;
    bridge_detailed_resource_traces_from_local_logs(&state, &tenant, &mut counts).await;

    if req.include_estimates {
        for candidate in &candidates {
            let agent_id = canonical_agent_id(candidate);
            if !exact_agents.contains(&agent_id)
                && candidate_collects_token_usage(candidate)
                && persist_estimated_presence_usage(&state, &tenant, candidate, &scan_id)
                    .await
                    .is_some()
            {
                counts.usage_events += 1;
                counts.estimated_usage_events += 1;
                counts
                    .capture_quality
                    .insert("estimated_metadata_only".to_string());
            }
        }
    }

    let mut limitations = vec![
        "Exact token and cost capture requires a plaintext integration point: provider response ingestion, SDK wrapper, MCP/HTTP proxy, browser extension, or a local log/session file that contains a provider usage object.".to_string(),
        "Process, window, and encrypted network metadata can prove activity and resource access, but cannot read HTTPS response bodies by itself.".to_string(),
        "Detailed file, folder, and database trace depth is exact only for sources that expose the object name, such as OS audit/ETW/EndpointSecurity/fanotify/eBPF, MCP wrappers, browser extensions, database hooks, or structured local agent logs.".to_string(),
    ];
    if counts.estimated_usage_events > 0 {
        limitations.push(
            "Some usage events are estimates because no exact local usage source was available for those agents during this refresh.".to_string(),
        );
    }

    let response = LocalObserveRefreshResponse {
        schema_version: "local-observe-refresh.v1".to_string(),
        tenant_id: tenant,
        scan_id,
        candidates_found: candidates.len(),
        resource_events: counts.resource_events,
        identity_events: counts.identity_events,
        tool_events: counts.tool_events,
        usage_events: counts.usage_events,
        exact_usage_events: counts.exact_usage_events,
        estimated_usage_events: counts.estimated_usage_events,
        capture_quality: counts.capture_quality.into_iter().collect(),
        limitations,
        next_steps: default_next_steps(),
    };

    (
        StatusCode::OK,
        Json(serde_json::to_value(response).unwrap_or_else(|_| json!({}))),
    )
}

fn default_sources() -> Vec<String> {
    [
        "process",
        "mcp_config",
        "local_model",
        "ide_extension",
        "cli_agent",
        "container",
        "browser_extension",
        "installed_app",
        "web_ai",
        "python_framework",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect()
}

fn default_next_steps() -> Vec<LocalObserveNextStep> {
    vec![
        LocalObserveNextStep {
            action_id: "review_ai_app_native_permissions".into(),
            title: "Review the AI app's own permissions".into(),
            reason: "Observation can show which files, websites, tools, or apps were touched so the user can also tighten permissions inside ChatGPT, Claude, Codex, Manus, Antigravity, or the agent's own settings.".into(),
            route: "/activity".into(),
        },
        LocalObserveNextStep {
            action_id: "route_agents_through_mcp_wrapper_or_proxy".into(),
            title: "Activate MCP wrapper or proxy for exact tool/resource enforcement".into(),
            reason: "MCP wrapper/proxy can see plaintext tool calls and enforce policy before tools run.".into(),
            route: "/capabilities".into(),
        },
        LocalObserveNextStep {
            action_id: "install_browser_extension".into(),
            title: "Install browser extension for browser AI exact session visibility".into(),
            reason: "Browser AI sessions need an approved extension or DevTools-level integration to see prompts, responses, and provider usage fields.".into(),
            route: "/capabilities".into(),
        },
        LocalObserveNextStep {
            action_id: "enable_os_network_pep".into(),
            title: "Enable OS network PEP before device-level egress blocking".into(),
            reason: "Windows WFP, macOS Network Extension, or Linux eBPF must pass warm checks before network enforcement is real.".into(),
            route: "/capabilities".into(),
        },
    ]
}

async fn upsert_candidate(
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

async fn publish_candidate_observations(
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
struct ObservedResourceSeed {
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

struct ResourceSeedSpec {
    scope: &'static str,
    kind: &'static str,
    target_redacted: String,
    mode: &'static str,
    classification: Option<String>,
    control_method: Option<&'static str>,
    trace_source: &'static str,
}

fn resources_for_candidate(candidate: &DiscoveredAgentCandidateV2) -> Vec<ObservedResourceSeed> {
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

fn push_resources_from_evidence(
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

fn push_resource(
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

fn evidence_source_name(source: &EvidenceSource) -> &'static str {
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

fn resource_trace_details(
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

fn path_trace_details(path: &str) -> Map<String, Value> {
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

fn path_parts(path: &str) -> Vec<&str> {
    path.trim_matches(['\\', '/'])
        .split(['\\', '/'])
        .filter(|part| !part.is_empty())
        .collect()
}

fn join_path_parts(original: &str, parts: &[&str]) -> String {
    let separator = if original.contains('\\') { "\\" } else { "/" };
    let prefix = if original.starts_with('\\') || original.starts_with('/') {
        separator
    } else {
        ""
    };
    format!("{prefix}{}", parts.join(separator))
}

fn database_system_from_path(file_name: &str) -> Option<&'static str> {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".sqlite") || lower.ends_with(".sqlite3") || lower.ends_with(".db") {
        Some("sqlite")
    } else if lower.ends_with(".duckdb") {
        Some("duckdb")
    } else {
        None
    }
}

fn looks_like_host(value: &str) -> bool {
    let value = value.trim();
    value.contains('.') && !value.contains(['\\', '/', ' '])
}

fn host_part(value: &str) -> String {
    value
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(value)
        .to_string()
}

fn merge_detail_map(target: &mut Map<String, Value>, source: Map<String, Value>) {
    for (key, value) in source {
        target.entry(key).or_insert(value);
    }
}

/// Records a resource-access trace as a real `AgentObservationEvent` so it
/// reaches the activity read model behind the AI Activity page (Files / Web /
/// Email / Commands tiles). Telemetry envelopes alone never get there — they
/// only feed the inventory and cloud-sync paths. Event ids are stable, so a
/// re-scan of the same log line is a no-op (primary-key insert).
async fn record_resource_observation(
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

async fn publish_payload(
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn exact_local_log_extracts_usage_without_prompt_body() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("usage.jsonl");
        let mut file = std::fs::File::create(&path)?;
        writeln!(
            file,
            "{}",
            json!({
                "provider": "openai",
                "model": "gpt-4o",
                "usage": {
                    "prompt_tokens": 12,
                    "completion_tokens": 8,
                    "total_tokens": 20
                },
                "output": "should not be persisted"
            })
        )?;

        let events = extract_exact_usage_events_from_path("local", &path)?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].tokens.total_tokens, 20);
        assert_eq!(events[0].metadata["capture_quality"], "exact_local_log");
        assert!(events[0].provider_usage_raw.get("prompt_tokens").is_some());
        assert!(events[0].provider_usage_raw.get("output").is_none());
        Ok(())
    }

    #[test]
    fn codex_rollout_token_count_extracts_exact_usage_with_model() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("rollout-2026-07-14.jsonl");
        let mut file = std::fs::File::create(&path)?;
        // Real Codex CLI rollout shape: turn_context carries the model,
        // token_count carries the usage — no `usage` object anywhere.
        writeln!(
            file,
            "{}",
            json!({
                "timestamp": "2026-07-14T09:17:23.456Z",
                "type": "turn_context",
                "payload": { "model": "gpt-5.1-codex", "cwd": "/redacted" }
            })
        )?;
        writeln!(
            file,
            "{}",
            json!({
                "timestamp": "2026-07-14T09:17:41.000Z",
                "type": "event_msg",
                "payload": {
                    "type": "token_count",
                    "info": {
                        "total_token_usage": {
                            "input_tokens": 5000,
                            "cached_input_tokens": 4000,
                            "output_tokens": 900,
                            "reasoning_output_tokens": 300,
                            "total_tokens": 5900
                        },
                        "last_token_usage": {
                            "input_tokens": 1200,
                            "cached_input_tokens": 1000,
                            "output_tokens": 250,
                            "reasoning_output_tokens": 80,
                            "total_tokens": 1450
                        }
                    }
                }
            })
        )?;

        let events = extract_exact_usage_events_from_path("local", &path)?;
        assert_eq!(events.len(), 1, "one usage event per token_count line");
        let event = &events[0];
        assert_eq!(event.model.as_deref(), Some("gpt-5.1-codex"));
        assert_eq!(event.provider.as_deref(), Some("openai"));
        assert_eq!(event.tokens.input_tokens, 1200, "uses per-turn last usage");
        assert_eq!(event.tokens.output_tokens, 250);
        assert_eq!(event.tokens.cached_input_tokens, 1000);
        assert_eq!(event.tokens.reasoning_output_tokens, 80);
        assert_eq!(event.tokens.total_tokens, 1450);
        assert!(!event.tokens.estimated, "local-log capture is exact");
        assert_eq!(
            event.metadata["capture_source"],
            "codex_rollout_token_count"
        );
        Ok(())
    }

    #[test]
    fn codex_shell_function_call_yields_command_trace_without_full_cmdline() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("rollout.jsonl");
        let mut file = std::fs::File::create(&path)?;
        // Real Codex shape: arguments is a JSON *string*.
        writeln!(
            file,
            "{}",
            json!({
                "timestamp": "2026-07-14T10:00:00.000Z",
                "type": "response_item",
                "payload": {
                    "type": "function_call",
                    "name": "shell",
                    "arguments": "{\"command\":[\"bash\",\"-lc\",\"cargo build --release\"]}"
                }
            })
        )?;

        let events = extract_resource_trace_events_from_path("local", &path)?;
        let command_events: Vec<_> = events
            .iter()
            .filter(|(_, payload)| payload["kind"] == "command")
            .collect();
        assert_eq!(command_events.len(), 1, "one command trace expected");
        let payload = &command_events[0].1;
        assert_eq!(payload["target_redacted"], "bash");
        assert_eq!(payload["mode"], "execute");
        assert_eq!(payload["details"]["full_command_stored"], false);
        assert!(
            payload["details"].get("command_line").is_none(),
            "raw command line must not be persisted"
        );
        Ok(())
    }

    #[test]
    fn web_and_email_access_extracted_host_only() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("session.jsonl");
        let mut file = std::fs::File::create(&path)?;
        // Claude Code-style WebFetch tool call.
        writeln!(
            file,
            "{}",
            json!({
                "type": "tool_use",
                "name": "WebFetch",
                "input": { "url": "https://docs.example.com/private/page?token=secret" }
            })
        )?;
        // Email service access.
        writeln!(
            file,
            "{}",
            json!({
                "tool_name": "send_email",
                "url": "smtp://smtp.office365.com:587"
            })
        )?;
        // Local model endpoint must NOT count as web activity.
        writeln!(
            file,
            "{}",
            json!({ "url": "http://127.0.0.1:11434/v1/chat/completions" })
        )?;

        let events = extract_resource_trace_events_from_path("local", &path)?;
        let web: Vec<_> = events
            .iter()
            .filter(|(_, p)| p["kind"] == "web_domain")
            .collect();
        let email: Vec<_> = events
            .iter()
            .filter(|(_, p)| p["kind"] == "email")
            .collect();

        assert_eq!(web.len(), 1, "one web access");
        assert_eq!(web[0].1["target_redacted"], "docs.example.com");
        assert!(
            !web[0].1.to_string().contains("token=secret"),
            "URL path/query must never be persisted"
        );

        assert_eq!(email.len(), 1, "one email access");
        assert_eq!(email[0].1["target_redacted"], "smtp.office365.com");
        assert_eq!(email[0].1["mode"], "send");
        Ok(())
    }

    #[test]
    fn static_mcp_launcher_config_is_not_a_command_trace() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("claude.json");
        std::fs::write(
            &path,
            json!({
                "mcpServers": {
                    "files": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem"] }
                }
            })
            .to_string(),
        )?;

        let events = extract_resource_trace_events_from_path("local", &path)?;
        assert!(
            events.iter().all(|(_, p)| p["kind"] != "command"),
            "launcher config must not be reported as executed command, got {events:?}"
        );
        Ok(())
    }

    #[test]
    fn usage_files_are_found_at_codex_session_depth() -> anyhow::Result<()> {
        // ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl is four levels below
        // the scanned root; the old depth cap silently skipped it.
        let dir = tempfile::tempdir()?;
        let deep = dir.path().join("sessions/2026/07/14");
        std::fs::create_dir_all(&deep)?;
        let file_path = deep.join("rollout-abc.jsonl");
        std::fs::write(&file_path, "{}\n")?;

        let mut found = Vec::new();
        collect_usage_files(dir.path(), &mut found, 0);
        assert!(
            found.contains(&file_path),
            "codex-depth session file must be discovered, got {found:?}"
        );
        Ok(())
    }

    #[test]
    fn resource_trace_extracts_file_and_db_details_without_raw_content() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("session.jsonl");
        let mut file = std::fs::File::create(&path)?;
        writeln!(
            file,
            "{}",
            json!({
                "agent_id": "codex",
                "file_path": "C:\\Users\\Alice\\Documents\\repo\\src\\main.rs",
                "action": "read",
                "prompt": "do not persist"
            })
        )?;
        writeln!(
            file,
            "{}",
            json!({
                "agent_id": "codex",
                "database_path": "C:\\Users\\Alice\\data\\app.sqlite",
                "sql": "SELECT email FROM users WHERE id = 123"
            })
        )?;

        let events = extract_resource_trace_events_from_path("local", &path)?;
        assert_eq!(events.len(), 2);

        let file_event = events
            .iter()
            .map(|(_, payload)| payload)
            .find(|payload| payload["kind"] == "file")
            .ok_or_else(|| anyhow::anyhow!("file event"))?;
        assert_eq!(file_event["details"]["file_name"], "main.rs");
        assert_eq!(
            file_event["details"]["capture_quality"],
            "exact_local_log_metadata"
        );
        assert_eq!(file_event["details"]["raw_content_stored"], false);
        assert!(file_event["details"].get("prompt").is_none());

        let db_event = events
            .iter()
            .map(|(_, payload)| payload)
            .find(|payload| payload["kind"] == "database_local")
            .ok_or_else(|| anyhow::anyhow!("db event"))?;
        assert_eq!(db_event["details"]["db_system"], "sqlite");
        assert_eq!(db_event["details"]["db_table"], "users");
        assert_eq!(db_event["details"]["query_summary"], "read users");
        assert!(db_event["details"].get("sql").is_none());
        Ok(())
    }
}
