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

async fn bridge_exact_usage_from_telemetry(
    state: &AppState,
    tenant: &str,
    counts: &mut ObserveCounts,
    exact_agents: &mut HashSet<String>,
) {
    if let Ok(events) = state
        .telemetry_store
        .list_telemetry(tenant, "ai_usage_event")
        .await
    {
        for ev in events {
            let value = payload_or_self(ev);
            if let Ok(mut usage) = serde_json::from_value::<AiUsageEventV1>(value) {
                usage.metadata = crate::usage_api::merge_usage_metadata(
                    usage.metadata,
                    json!({
                        "capture_quality": if usage.tokens.estimated { "estimated_forwarded_usage" } else { "exact_forwarded_usage" },
                        "capture_source": "telemetry_ai_usage_event"
                    }),
                );
                if crate::usage_api::persist_usage_event(state, tenant, usage.clone())
                    .await
                    .is_ok()
                {
                    counts.usage_events += 1;
                    if usage.tokens.estimated {
                        counts.estimated_usage_events += 1;
                        counts
                            .capture_quality
                            .insert("estimated_forwarded_usage".to_string());
                    } else {
                        counts.exact_usage_events += 1;
                        counts
                            .capture_quality
                            .insert("exact_forwarded_usage".to_string());
                        if let Some(agent_id) = usage.agent_id {
                            exact_agents.insert(agent_id);
                        }
                    }
                }
            }
        }
    }

    if let Ok(events) = state
        .telemetry_store
        .list_telemetry(tenant, "agent_observation")
        .await
    {
        for ev in events {
            let value = payload_or_self(ev);
            if let Ok(obs) = serde_json::from_value::<AgentObservationEvent>(value.clone()) {
                let _ = state
                    .observability_store
                    .insert_observation_event(&obs)
                    .await;
                if obs.token_usage.is_some() {
                    let mut usage =
                        AiUsageEventV1::from_legacy_observation(&obs, obs.provider.clone());
                    usage.metadata = crate::usage_api::merge_usage_metadata(
                        usage.metadata,
                        json!({
                            "capture_quality": "exact_agent_observation",
                            "capture_source": obs.pep_type.clone().unwrap_or_else(|| "agent_observation".to_string())
                        }),
                    );
                    if crate::usage_api::persist_usage_event(state, tenant, usage.clone())
                        .await
                        .is_ok()
                    {
                        counts.usage_events += 1;
                        counts.exact_usage_events += 1;
                        counts
                            .capture_quality
                            .insert("exact_agent_observation".to_string());
                        if let Some(agent_id) = usage.agent_id {
                            exact_agents.insert(agent_id);
                        }
                    }
                }
            }
        }
    }
}

async fn bridge_exact_usage_from_local_logs(
    state: &AppState,
    tenant: &str,
    counts: &mut ObserveCounts,
    exact_agents: &mut HashSet<String>,
) {
    let paths = collect_usage_log_paths_with_user_inputs(state).await;
    for path in paths.into_iter().take(60) {
        let Ok(events) = extract_exact_usage_events_from_path(tenant, &path) else {
            continue;
        };
        for event in events {
            let agent_id = event.agent_id.clone();
            if crate::usage_api::persist_usage_event(state, tenant, event)
                .await
                .is_ok()
            {
                counts.usage_events += 1;
                counts.exact_usage_events += 1;
                counts.capture_quality.insert("exact_local_log".to_string());
                if let Some(agent_id) = agent_id {
                    exact_agents.insert(agent_id);
                }
            }
        }
    }
}

async fn bridge_detailed_resource_traces_from_local_logs(
    state: &AppState,
    tenant: &str,
    counts: &mut ObserveCounts,
) {
    let mut seen = HashSet::new();
    for path in collect_usage_log_paths_with_user_inputs(state)
        .await
        .into_iter()
        .take(60)
    {
        let Ok(events) = extract_resource_trace_events_from_path(tenant, &path) else {
            continue;
        };
        for (event_id, payload) in events {
            if !seen.insert(event_id.clone()) {
                continue;
            }
            record_resource_observation(state, tenant, &event_id, &payload).await;
            publish_payload(state, tenant, "resource_access", &event_id, payload, true).await;
            counts.resource_events += 1;
            counts
                .capture_quality
                .insert("resource_trace_local_log".to_string());
        }
    }
}

async fn collect_usage_log_paths_with_user_inputs(state: &AppState) -> Vec<PathBuf> {
    let mut paths = state.observe_accuracy_store.local_usage_log_paths().await;
    paths.extend(collect_usage_log_paths());
    paths.sort();
    paths.dedup();
    paths
}

fn collect_usage_log_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(value) = std::env::var("POLLEK_USAGE_LOG_PATHS") {
        for item in value.split(';').flat_map(|chunk| chunk.split(',')) {
            let item = item.trim();
            if !item.is_empty() {
                paths.push(PathBuf::from(item));
            }
        }
    }

    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    if !home.is_empty() {
        for relative in [
            ".codex/sessions",
            ".codex/logs",
            ".claude",
            ".gemini",
            ".continue",
            ".cursor",
        ] {
            paths.push(PathBuf::from(&home).join(relative));
        }
    }

    let mut files = Vec::new();
    for path in paths {
        collect_usage_files(&path, &mut files, 0);
    }
    files.sort();
    files.dedup();
    // Newest sessions first: real agents keep appending fresh session files
    // (e.g. Codex writes a new rollout file per session) and the scan is
    // capped, so recency decides which files make the cut.
    files.sort_by_key(|path| {
        std::cmp::Reverse(
            std::fs::metadata(path)
                .and_then(|meta| meta.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
        )
    });
    files.truncate(60);
    files
}

fn collect_usage_files(path: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    // Codex session logs live four levels deep
    // (~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl), so the walk must go
    // deeper than the agent config roots themselves.
    if out.len() >= 400 || depth > 6 || !path.exists() {
        return;
    }
    if path.is_file() {
        if is_usage_file(path) {
            out.push(path.to_path_buf());
        }
        return;
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        collect_usage_files(&entry.path(), out, depth + 1);
        if out.len() >= 400 {
            break;
        }
    }
}

fn is_usage_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "json" | "jsonl" | "ndjson" | "log"
    )
}

fn extract_exact_usage_events_from_path(
    tenant: &str,
    path: &Path,
) -> anyhow::Result<Vec<AiUsageEventV1>> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > 5 * 1024 * 1024 {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    let mut events = Vec::new();

    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "jsonl" | "ndjson" | "log"
            )
        })
        .unwrap_or(false)
    {
        // Codex CLI rollout files carry the model in `turn_context` lines and
        // token counts in later `token_count` event lines; remember the most
        // recent model while streaming so each usage event gets attributed.
        let mut current_model: Option<String> = None;
        for line in content.lines().take(20_000) {
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                if let Some(model) = model_hint_from_line(&value) {
                    current_model = Some(model);
                }
                if let Some(event) =
                    codex_token_count_event(tenant, path, &value, current_model.as_deref())
                {
                    if events.len() < 100 {
                        events.push(event);
                    }
                    continue;
                }
                collect_exact_usage_from_value(tenant, path, &value, &mut events);
            }
        }
    } else if let Ok(value) = serde_json::from_str::<Value>(&content) {
        collect_exact_usage_from_value(tenant, path, &value, &mut events);
    }

    Ok(events)
}

/// Remembers the model named in a Codex rollout `turn_context` line (or any
/// line that carries a plain `model` string) for later token_count events.
fn model_hint_from_line(value: &Value) -> Option<String> {
    let model = value
        .get("payload")
        .and_then(|payload| payload.get("model"))
        .or_else(|| value.get("model"))
        .or_else(|| {
            value
                .get("payload")
                .and_then(|payload| payload.get("info"))
                .and_then(|info| info.get("model"))
        })?
        .as_str()?
        .trim();
    if model.is_empty() {
        return None;
    }
    Some(model.to_string())
}

/// Parses a Codex CLI rollout `token_count` event line:
/// `{"timestamp":"…","type":"event_msg","payload":{"type":"token_count",
///   "info":{"total_token_usage":{…},"last_token_usage":{…}}}}`
/// The per-turn `last_token_usage` is preferred so consecutive lines do not
/// double-count; `total_token_usage` is the fallback for older files.
fn codex_token_count_event(
    tenant: &str,
    path: &Path,
    value: &Value,
    current_model: Option<&str>,
) -> Option<AiUsageEventV1> {
    let payload = value.get("payload")?;
    let payload_type = payload.get("type").and_then(Value::as_str)?;
    if payload_type != "token_count" {
        return None;
    }
    let info = payload.get("info")?;
    let usage = info
        .get("last_token_usage")
        .or_else(|| info.get("total_token_usage"))?;
    let token_field = |key: &str| usage.get(key).and_then(Value::as_i64).unwrap_or(0);

    let input_tokens = token_field("input_tokens");
    let output_tokens = token_field("output_tokens");
    let cached_input_tokens = token_field("cached_input_tokens");
    let reasoning_output_tokens = token_field("reasoning_output_tokens");
    let total_tokens = {
        let reported = token_field("total_tokens");
        if reported > 0 {
            reported
        } else {
            input_tokens + output_tokens
        }
    };
    if total_tokens == 0 {
        return None;
    }

    let model = current_model.unwrap_or("codex-session").to_string();
    let source_path_hash = hash_hex(&path.to_string_lossy());
    let event_id = stable_event_id(
        "usage_log_codex",
        &[
            tenant,
            &model,
            &source_path_hash,
            &hash_hex(&usage.to_string()),
        ],
    );
    let occurred_at = value
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    let tokens = CanonicalTokenUsage {
        input_tokens,
        output_tokens,
        cached_input_tokens,
        reasoning_output_tokens,
        total_tokens,
        estimated: false,
        source: UsageSource::ProviderResponse,
        ..CanonicalTokenUsage::default()
    };

    Some(
        AiUsageEventV1 {
            schema_version: AiUsageEventV1::SCHEMA_VERSION.to_string(),
            event_id,
            event_kind: AiUsageEventKind::ModelCallCompleted,
            occurred_at,
            received_at: Utc::now(),
            tenant_id: tenant.to_string(),
            workspace_id: Some("default".to_string()),
            device_id: Some(local_device_id()),
            actor_id_hash: None,
            actor_kind: None,
            trace_id: format!("trace_{}", uuid::Uuid::new_v4()),
            span_id: format!("span_{}", uuid::Uuid::new_v4()),
            parent_span_id: None,
            session_id: string_path(value, &["session_id"]),
            task_id: None,
            agent_run_id: None,
            agent_step_id: None,
            invocation_id: None,
            agent_id: Some("codex_cli".to_string()),
            agent_instance_id: None,
            agent_type: AgentType::CodexCli,
            parent_agent_id: None,
            subagent_id: None,
            shadow_candidate_id: None,
            provider: Some("openai".to_string()),
            provider_api: None,
            provider_request_id: None,
            model: Some(model),
            model_version: None,
            service_tier: None,
            inference_region: None,
            surface: "local_usage_log".to_string(),
            pep_type: Some("local_log_reader".to_string()),
            control_mode: Some("observe".to_string()),
            policy_ids: vec![],
            tokens,
            cost: CanonicalCostBreakdown::default(),
            tool_id: None,
            tool_name: None,
            mcp_server_id: None,
            resource_id: None,
            resource_type: None,
            latency_ms: None,
            status: "ok".to_string(),
            error_code: None,
            provider_usage_raw: usage.clone(),
            metadata: json!({
                "capture_quality": "exact_local_log",
                "capture_source": "codex_rollout_token_count",
                "source_path_hash": source_path_hash,
                "source_path_redacted": redact_path(path),
                "raw_prompt_or_response_stored": false
            }),
            local_sequence: None,
            cloud_sync_status: Some("pending".to_string()),
            idempotency_key: String::new(),
        }
        .finalize(),
    )
}

fn extract_resource_trace_events_from_path(
    tenant: &str,
    path: &Path,
) -> anyhow::Result<Vec<(String, Value)>> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > 5 * 1024 * 1024 {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    let mut events = Vec::new();

    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "jsonl" | "ndjson" | "log"
            )
        })
        .unwrap_or(false)
    {
        for line in content.lines().take(20_000) {
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                collect_resource_traces_from_value(tenant, path, &value, &mut events);
            }
            if events.len() >= 200 {
                break;
            }
        }
    } else if let Ok(value) = serde_json::from_str::<Value>(&content) {
        collect_resource_traces_from_value(tenant, path, &value, &mut events);
    }

    events.sort_by(|a, b| a.0.cmp(&b.0));
    events.dedup_by(|a, b| a.0 == b.0);
    Ok(events)
}

fn collect_resource_traces_from_value(
    tenant: &str,
    source_path: &Path,
    value: &Value,
    out: &mut Vec<(String, Value)>,
) {
    if out.len() >= 200 {
        return;
    }
    if let Some(event) = resource_trace_event_from_value(tenant, source_path, value) {
        out.push(event);
    }
    // Codex rollout tool calls carry their arguments as a JSON *string*
    // (`{"type":"function_call","name":"shell","arguments":"{…}"}`), so plain
    // recursion never sees inside them. Unwrap once and recurse into the
    // parsed arguments with the tool name attached for context.
    if let Some(unwrapped) = unwrap_function_call_arguments(value) {
        collect_resource_traces_from_value(tenant, source_path, &unwrapped, out);
    }
    match value {
        Value::Object(map) => {
            for value in map.values() {
                collect_resource_traces_from_value(tenant, source_path, value, out);
                if out.len() >= 200 {
                    break;
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_resource_traces_from_value(tenant, source_path, value, out);
                if out.len() >= 200 {
                    break;
                }
            }
        }
        _ => {}
    }
}

/// Parses the stringified `arguments` of an agent tool call (Codex
/// `function_call`, OpenAI-style tool call records) into a real object so the
/// resource extractors can see the file paths / commands / URLs inside.
fn unwrap_function_call_arguments(value: &Value) -> Option<Value> {
    let name = value.get("name").and_then(Value::as_str)?;
    let arguments = value.get("arguments").and_then(Value::as_str)?;
    let parsed: Value = serde_json::from_str(arguments).ok()?;
    let mut object = match parsed {
        Value::Object(map) => map,
        _ => return None,
    };
    object.insert("tool_name".to_string(), json!(name));
    // Mark as an executed tool call so command extraction can distinguish
    // this from static launcher configs that also have a `command` key.
    object.insert("observed_tool_call".to_string(), json!(true));
    Some(Value::Object(object))
}

fn resource_trace_event_from_value(
    tenant: &str,
    source_path: &Path,
    value: &Value,
) -> Option<(String, Value)> {
    if let Some((target, mut details, mode, kind)) = file_or_folder_trace(value) {
        add_local_log_provenance(&mut details, source_path);
        let agent_id = agent_id_from_value(value);
        let event_id = stable_event_id(
            "resource_log",
            &[
                tenant,
                &agent_id,
                &target,
                mode,
                &hash_hex(&details.to_string()),
            ],
        );
        return Some((
            event_id,
            json!({
                "agent_id": agent_id,
                "agent_label": agent_label_from_value(value),
                "scope": "local",
                "kind": kind,
                "target_redacted": target,
                "target_hash": hash_hex(&target),
                "mode": mode,
                "decision": "observed",
                "enforced_for_real": false,
                "count": 1,
                "classification": string_any(value, &["classification", "sensitivity"]),
                "details": details,
                "observed_at": timestamp_from_value(value).unwrap_or_else(Utc::now),
            }),
        ));
    }

    if let Some((target, mut details, mode)) = database_trace(value) {
        add_local_log_provenance(&mut details, source_path);
        let agent_id = agent_id_from_value(value);
        let event_id = stable_event_id(
            "resource_db_log",
            &[
                tenant,
                &agent_id,
                &target,
                mode,
                &hash_hex(&details.to_string()),
            ],
        );
        return Some((
            event_id,
            json!({
                "agent_id": agent_id,
                "agent_label": agent_label_from_value(value),
                "scope": "local",
                "kind": "database_local",
                "target_redacted": target,
                "target_hash": hash_hex(&target),
                "mode": mode,
                "decision": "observed",
                "enforced_for_real": false,
                "count": 1,
                "classification": string_any(value, &["classification", "sensitivity"]),
                "details": details,
                "observed_at": timestamp_from_value(value).unwrap_or_else(Utc::now),
            }),
        ));
    }

    if let Some((target, mut details, mode, kind)) = web_or_email_trace(value) {
        add_local_log_provenance(&mut details, source_path);
        let agent_id = agent_id_from_value(value);
        let event_id = stable_event_id(
            "resource_web_log",
            &[
                tenant,
                &agent_id,
                &target,
                mode,
                &hash_hex(&details.to_string()),
            ],
        );
        return Some((
            event_id,
            json!({
                "agent_id": agent_id,
                "agent_label": agent_label_from_value(value),
                "scope": "local",
                "kind": kind,
                "target_redacted": target,
                "target_hash": hash_hex(&target),
                "mode": mode,
                "decision": "observed",
                "enforced_for_real": false,
                "count": 1,
                "classification": string_any(value, &["classification", "sensitivity"]),
                "details": details,
                "observed_at": timestamp_from_value(value).unwrap_or_else(Utc::now),
            }),
        ));
    }

    if let Some((target, mut details)) = command_trace(value) {
        add_local_log_provenance(&mut details, source_path);
        let agent_id = agent_id_from_value(value);
        let event_id = stable_event_id(
            "resource_cmd_log",
            &[tenant, &agent_id, &target, &hash_hex(&details.to_string())],
        );
        return Some((
            event_id,
            json!({
                "agent_id": agent_id,
                "agent_label": agent_label_from_value(value),
                "scope": "local",
                "kind": "command",
                "target_redacted": target,
                "target_hash": hash_hex(&target),
                "mode": "execute",
                "decision": "observed",
                "enforced_for_real": false,
                "count": 1,
                "classification": Value::Null,
                "details": details,
                "observed_at": timestamp_from_value(value).unwrap_or_else(Utc::now),
            }),
        ));
    }

    None
}

/// Extracts a web (or email-service) access from an agent tool call: only the
/// scheme + host are kept — never the full URL path, query, or content.
fn web_or_email_trace(value: &Value) -> Option<(String, Value, &'static str, &'static str)> {
    let url = string_any(
        value,
        &["url", "uri", "request_url", "href", "link", "web_url"],
    )?;
    let trimmed = url.trim();
    let (scheme, rest) = trimmed.split_once("://")?;
    let scheme_lower = scheme.to_ascii_lowercase();
    if !matches!(
        scheme_lower.as_str(),
        "http" | "https" | "smtp" | "smtps" | "imap" | "imaps" | "pop3" | "pop3s" | "mailto"
    ) {
        return None;
    }
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(rest)
        .split('@')
        .next_back()
        .unwrap_or(rest)
        .split(':')
        .next()
        .unwrap_or(rest)
        .trim()
        .to_ascii_lowercase();
    if host.is_empty() || host == "localhost" || host.starts_with("127.") || host.starts_with("[") {
        // Local endpoints are model/MCP servers, not web activity.
        return None;
    }

    let is_email = matches!(
        scheme_lower.as_str(),
        "smtp" | "smtps" | "imap" | "imaps" | "pop3" | "pop3s" | "mailto"
    ) || is_email_service_host(&host);
    let kind = if is_email { "email" } else { "web_domain" };
    let mode = if is_email && scheme_lower.starts_with("smtp") {
        "send"
    } else {
        "connect"
    };

    let mut details = Map::new();
    details.insert(
        "trace_source".to_string(),
        json!("known_agent_log_or_session_file"),
    );
    details.insert(
        "capture_quality".to_string(),
        json!("exact_local_log_metadata"),
    );
    details.insert("scheme".to_string(), json!(scheme_lower));
    details.insert("host".to_string(), json!(host));
    details.insert("full_url_stored".to_string(), json!(false));
    if let Some(tool) = string_any(value, &["tool_name", "name", "tool"]) {
        details.insert("tool_name".to_string(), json!(tool));
    }
    Some((host, Value::Object(details), mode, kind))
}

/// Hosts that indicate email/calendar service access rather than plain web.
fn is_email_service_host(host: &str) -> bool {
    const EMAIL_HOSTS: &[&str] = &[
        "graph.microsoft.com",
        "outlook.office365.com",
        "outlook.office.com",
        "smtp.office365.com",
        "gmail.googleapis.com",
        "mail.google.com",
        "imap.gmail.com",
        "smtp.gmail.com",
        "api.mailgun.net",
        "api.sendgrid.com",
        "api.postmarkapp.com",
        "api.resend.com",
    ];
    if EMAIL_HOSTS.contains(&host) {
        return true;
    }
    host.starts_with("smtp.")
        || host.starts_with("imap.")
        || host.starts_with("pop.")
        || host.starts_with("pop3.")
        || host.starts_with("mail.")
        || host.starts_with("webmail.")
}

/// Extracts a command execution from an agent tool call. Only the program
/// name and argument count are kept — never the full command line. To avoid
/// misreading static launcher configs (MCP server entries also have a
/// `command` key), a trace is only emitted when the record shows evidence of
/// actual execution: an argv array (Codex shell calls), an unwrapped tool
/// call, or execution artifacts like an exit code / output next to it.
fn command_trace(value: &Value) -> Option<(String, Value)> {
    let object = value.as_object()?;
    let command_value = object.get("command").or_else(|| object.get("cmd"))?;

    let executed = command_value.is_array()
        || object
            .get("observed_tool_call")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        || [
            "exit_code",
            "exitCode",
            "status",
            "stdout",
            "output",
            "duration_ms",
        ]
        .iter()
        .any(|key| object.contains_key(*key))
        || string_any(value, &["tool_name", "name"])
            .map(|tool| {
                matches!(
                    tool.to_ascii_lowercase().as_str(),
                    "bash" | "shell" | "exec" | "exec_command" | "run_command" | "terminal"
                )
            })
            .unwrap_or(false);
    if !executed {
        return None;
    }

    let argv: Vec<String> = match command_value {
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Value::String(line) => line.split_whitespace().map(str::to_string).collect(),
        _ => return None,
    };
    let program = argv.first()?;
    let program_name = std::path::Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
        .to_string();
    if program_name.is_empty() {
        return None;
    }

    let mut details = Map::new();
    details.insert(
        "trace_source".to_string(),
        json!("known_agent_log_or_session_file"),
    );
    details.insert(
        "capture_quality".to_string(),
        json!("exact_local_log_metadata"),
    );
    details.insert("program".to_string(), json!(program_name));
    details.insert("arg_count".to_string(), json!(argv.len().saturating_sub(1)));
    details.insert("full_command_stored".to_string(), json!(false));
    if let Some(tool) = string_any(value, &["tool_name", "name"]) {
        details.insert("tool_name".to_string(), json!(tool));
    }
    Some((program_name, Value::Object(details)))
}

fn file_or_folder_trace(value: &Value) -> Option<(String, Value, &'static str, &'static str)> {
    let path = string_any(
        value,
        &[
            "file_path",
            "filepath",
            "fileName",
            "filename",
            "file",
            "folder_path",
            "folder",
            "directory",
            "cwd",
            "workspace_path",
            "workspace",
            "path",
        ],
    )?;
    if !looks_like_local_path(&path) {
        return None;
    }

    let target = redact_local_path_string(&path);
    let mut details = path_trace_details(&target);
    let path_kind = details
        .get("path_kind")
        .and_then(Value::as_str)
        .unwrap_or("file");
    if path_kind == "folder" && !is_likely_folder_key(value) {
        return None;
    }
    let kind = if details.get("db_system").is_some() {
        "database_local"
    } else if path_kind == "folder" {
        "folder"
    } else {
        "file"
    };
    details.insert(
        "trace_source".to_string(),
        json!("known_agent_log_or_session_file"),
    );
    details.insert(
        "capture_quality".to_string(),
        json!("exact_local_log_metadata"),
    );
    Some((target, Value::Object(details), mode_from_value(value), kind))
}

fn database_trace(value: &Value) -> Option<(String, Value, &'static str)> {
    let sql = string_any(value, &["sql", "query", "statement", "db_statement"]);
    let table = string_any(
        value,
        &[
            "table",
            "table_name",
            "db_table",
            "collection",
            "collection_name",
            "db_collection",
        ],
    );
    let database = string_any(
        value,
        &[
            "database",
            "database_name",
            "db",
            "db_name",
            "schema",
            "namespace",
            "db_namespace",
        ],
    );
    let db_path = string_any(
        value,
        &["database_path", "db_path", "sqlite_path", "duckdb_path"],
    );
    if sql.is_none() && table.is_none() && db_path.is_none() {
        return None;
    }

    let operation = sql
        .as_deref()
        .and_then(sql_operation)
        .or_else(|| {
            string_any(value, &["operation", "db_operation"]).map(|op| normalize_db_operation(&op))
        })
        .unwrap_or("read");
    let table_from_sql = sql.as_deref().and_then(sql_table_name);
    let table = table.or(table_from_sql);
    let system = string_any(value, &["db_system", "db.system", "driver"])
        .or_else(|| {
            db_path
                .as_deref()
                .and_then(database_system_from_path)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".to_string());
    let namespace = database
        .or_else(|| db_path.as_deref().map(redact_local_path_string))
        .unwrap_or_else(|| "unknown".to_string());

    let mut details = Map::new();
    details.insert(
        "trace_source".to_string(),
        json!("known_agent_log_or_session_file"),
    );
    details.insert(
        "capture_quality".to_string(),
        json!("exact_local_log_metadata"),
    );
    details.insert(
        "trace_granularity".to_string(),
        json!(if table.is_some() {
            "db_table"
        } else {
            "database"
        }),
    );
    details.insert("db_system".to_string(), json!(system));
    details.insert("db_namespace".to_string(), json!(namespace));
    details.insert("db_operation".to_string(), json!(operation));
    if let Some(table) = &table {
        details.insert("db_table".to_string(), json!(table));
    }
    if let Some(sql) = sql {
        details.insert("query_summary".to_string(), json!(sql_summary(&sql)));
        details.insert(
            "query_fingerprint".to_string(),
            json!(hash_hex(&normalize_sql_for_hash(&sql))),
        );
    }
    if let Some(db_path) = db_path {
        merge_detail_map(
            &mut details,
            path_trace_details(&redact_local_path_string(&db_path)),
        );
    }

    let table_key = table.unwrap_or_else(|| "unknown_table".to_string());
    let target = format!(
        "db:{}:{}/{}",
        details
            .get("db_system")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        details
            .get("db_namespace")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        table_key
    );
    Some((
        target,
        Value::Object(details),
        mode_for_db_operation(operation),
    ))
}

fn add_local_log_provenance(details: &mut Value, source_path: &Path) {
    let source_path_hash = hash_hex(&source_path.to_string_lossy());
    if let Value::Object(map) = details {
        map.insert("source_path_hash".to_string(), json!(source_path_hash));
        map.insert(
            "source_path_redacted".to_string(),
            json!(redact_path(source_path)),
        );
        map.insert("raw_content_stored".to_string(), json!(false));
        map.insert(
            "limitations".to_string(),
            json!(["Exact for the local log record; use OS audit, EndpointSecurity, fanotify/eBPF, or a database hook for kernel/runtime-level proof."]),
        );
    }
}

fn collect_exact_usage_from_value(
    tenant: &str,
    path: &Path,
    value: &Value,
    out: &mut Vec<AiUsageEventV1>,
) {
    if out.len() >= 100 {
        return;
    }
    if has_usage_object(value) {
        if let Some(event) = exact_usage_event_from_raw_value(tenant, path, value) {
            out.push(event);
        }
    }
    match value {
        Value::Object(map) => {
            for value in map.values() {
                collect_exact_usage_from_value(tenant, path, value, out);
                if out.len() >= 100 {
                    break;
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_exact_usage_from_value(tenant, path, value, out);
                if out.len() >= 100 {
                    break;
                }
            }
        }
        _ => {}
    }
}

fn exact_usage_event_from_raw_value(
    tenant: &str,
    path: &Path,
    value: &Value,
) -> Option<AiUsageEventV1> {
    let provider = infer_provider(value)?;
    let host = host_for_provider(&provider);
    let (_provider, usage) = dek_agent_observer::egress_parser::parse_llm_usage(host, value)?;
    let usage_json = usage_subtree(value);
    let source_path_hash = hash_hex(&path.to_string_lossy());
    let event_id = stable_event_id(
        "usage_log",
        &[
            tenant,
            &provider,
            usage.model.as_deref().unwrap_or("unknown-model"),
            &source_path_hash,
            &hash_hex(&usage_json.to_string()),
        ],
    );

    let tokens = CanonicalTokenUsage {
        input_tokens: usage.input_tokens.unwrap_or(0),
        output_tokens: usage.output_tokens.unwrap_or(0),
        total_tokens: usage
            .total_tokens
            .unwrap_or_else(|| usage.input_tokens.unwrap_or(0) + usage.output_tokens.unwrap_or(0)),
        estimated: false,
        source: UsageSource::ProviderResponse,
        ..CanonicalTokenUsage::default()
    };

    Some(
        AiUsageEventV1 {
            schema_version: AiUsageEventV1::SCHEMA_VERSION.to_string(),
            event_id,
            event_kind: AiUsageEventKind::ModelCallCompleted,
            occurred_at: timestamp_from_value(value).unwrap_or_else(Utc::now),
            received_at: Utc::now(),
            tenant_id: tenant.to_string(),
            workspace_id: Some("default".to_string()),
            device_id: Some(local_device_id()),
            actor_id_hash: None,
            actor_kind: None,
            trace_id: format!("trace_{}", uuid::Uuid::new_v4()),
            span_id: format!("span_{}", uuid::Uuid::new_v4()),
            parent_span_id: None,
            session_id: string_path(value, &["session_id"]),
            task_id: string_path(value, &["task_id"]),
            agent_run_id: None,
            agent_step_id: None,
            invocation_id: string_path(value, &["id"]),
            agent_id: string_path(value, &["agent_id"]),
            agent_instance_id: None,
            agent_type: AgentType::Unknown,
            parent_agent_id: None,
            subagent_id: None,
            shadow_candidate_id: None,
            provider: Some(provider),
            provider_api: None,
            provider_request_id: string_path(value, &["id"]),
            model: usage.model,
            model_version: string_path(value, &["modelVersion"]),
            service_tier: None,
            inference_region: None,
            surface: "local_usage_log".to_string(),
            pep_type: Some("local_log_reader".to_string()),
            control_mode: Some("observe".to_string()),
            policy_ids: vec![],
            tokens,
            cost: CanonicalCostBreakdown::default(),
            tool_id: None,
            tool_name: None,
            mcp_server_id: None,
            resource_id: None,
            resource_type: None,
            latency_ms: None,
            status: "ok".to_string(),
            error_code: None,
            provider_usage_raw: usage_json,
            metadata: json!({
                "capture_quality": "exact_local_log",
                "capture_source": "known_agent_log_or_session_file",
                "source_path_hash": source_path_hash,
                "source_path_redacted": redact_path(path),
                "raw_prompt_or_response_stored": false
            }),
            local_sequence: None,
            cloud_sync_status: Some("pending".to_string()),
            idempotency_key: String::new(),
        }
        .finalize(),
    )
}

fn has_usage_object(value: &Value) -> bool {
    value.get("usage").is_some()
        || value.get("usageMetadata").is_some()
        || value.get("prompt_eval_count").is_some()
        || value.get("eval_count").is_some()
}

async fn persist_estimated_presence_usage(
    state: &AppState,
    tenant: &str,
    candidate: &DiscoveredAgentCandidateV2,
    scan_id: &str,
) -> Option<AiUsageEventV1> {
    let agent_id = canonical_agent_id(candidate);
    let now = Utc::now();
    let provider = provider_for_candidate(candidate);
    let usage_source = if matches!(candidate.inferred_agent_type, InferredAgentType::WebAIApp) {
        UsageSource::BrowserEstimate
    } else {
        UsageSource::LocalTokenizerEstimate
    };
    let tokens = CanonicalTokenUsage {
        input_tokens: 64,
        output_tokens: 16,
        total_tokens: 80,
        estimated: true,
        source: usage_source,
        ..CanonicalTokenUsage::default()
    };
    let event = AiUsageEventV1 {
        schema_version: AiUsageEventV1::SCHEMA_VERSION.to_string(),
        event_id: stable_event_id("usage_estimate", &[tenant, &agent_id, &scan_bucket()]),
        event_kind: AiUsageEventKind::ModelCallCompleted,
        occurred_at: now,
        received_at: now,
        tenant_id: tenant.to_string(),
        workspace_id: Some(state.identity.workspace_id.clone()),
        device_id: Some(local_device_id()),
        actor_id_hash: None,
        actor_kind: None,
        trace_id: format!("trace_{}", scan_id),
        span_id: format!("span_{}", candidate.candidate_id),
        parent_span_id: None,
        session_id: Some(scan_id.to_string()),
        task_id: None,
        agent_run_id: None,
        agent_step_id: None,
        invocation_id: None,
        agent_id: Some(agent_id),
        agent_instance_id: None,
        agent_type: agent_type_for_candidate(candidate),
        parent_agent_id: None,
        subagent_id: None,
        shadow_candidate_id: Some(candidate.candidate_id.clone()),
        provider,
        provider_api: None,
        provider_request_id: None,
        model: Some("unknown-observed-session".to_string()),
        model_version: None,
        service_tier: None,
        inference_region: None,
        surface: "local_observe_metadata".to_string(),
        pep_type: Some("local_observer".to_string()),
        control_mode: Some("observe".to_string()),
        policy_ids: vec![],
        tokens,
        cost: CanonicalCostBreakdown::default(),
        tool_id: None,
        tool_name: None,
        mcp_server_id: None,
        resource_id: None,
        resource_type: None,
        latency_ms: None,
        status: "ok".to_string(),
        error_code: None,
        provider_usage_raw: json!({}),
        metadata: json!({
            "capture_quality": "estimated_metadata_only",
            "capture_source": "process_window_or_config_observation",
            "fallback_reason": "No exact provider response, wrapper telemetry, browser extension usage event, or local usage log was available for this agent during refresh.",
            "exact_first": true,
            "raw_prompt_or_response_stored": false
        }),
        local_sequence: None,
        cloud_sync_status: Some("pending".to_string()),
        idempotency_key: String::new(),
    }
    .finalize();

    crate::usage_api::persist_usage_event(state, tenant, event.clone())
        .await
        .ok()
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

fn payload_or_self(value: Value) -> Value {
    value.get("payload").cloned().unwrap_or(value)
}

fn canonical_agent_id(candidate: &DiscoveredAgentCandidateV2) -> String {
    if !candidate.suggested_registration.agent_id.is_empty() {
        candidate.suggested_registration.agent_id.clone()
    } else {
        candidate.candidate_id.clone()
    }
}

fn candidate_collects_token_usage(candidate: &DiscoveredAgentCandidateV2) -> bool {
    candidate.suggested_observation_profile.collect_token_usage
        || candidate.capability_tags.iter().any(|tag| {
            matches!(
                tag.as_str(),
                "llm.call" | "llm.chat" | "web.chat" | "net.egress.llm" | "model.server"
            )
        })
}

fn agent_type_for_candidate(candidate: &DiscoveredAgentCandidateV2) -> AgentType {
    match candidate.inferred_agent_type {
        InferredAgentType::WebAIApp | InferredAgentType::BrowserAgent => AgentType::BrowserAi,
        InferredAgentType::CliAgent => {
            let name = candidate.display_name.to_ascii_lowercase();
            if name.contains("claude") {
                AgentType::ClaudeCode
            } else if name.contains("codex") {
                AgentType::CodexCli
            } else {
                AgentType::CodingAgent
            }
        }
        InferredAgentType::McpClient => AgentType::McpClient,
        InferredAgentType::McpServer => AgentType::McpServerAgent,
        _ => AgentType::LocalAgent,
    }
}

fn provider_for_candidate(candidate: &DiscoveredAgentCandidateV2) -> Option<String> {
    let joined = format!(
        "{} {} {}",
        candidate.display_name,
        candidate.vendor.clone().unwrap_or_default(),
        candidate.product.clone().unwrap_or_default()
    )
    .to_ascii_lowercase();
    if joined.contains("openai") || joined.contains("chatgpt") || joined.contains("codex") {
        Some("openai".into())
    } else if joined.contains("anthropic") || joined.contains("claude") {
        Some("anthropic".into())
    } else if joined.contains("google") || joined.contains("gemini") {
        Some("google".into())
    } else if joined.contains("deepseek") {
        Some("deepseek".into())
    } else if joined.contains("mistral") {
        Some("mistral".into())
    } else if joined.contains("ollama") {
        Some("ollama".into())
    } else {
        None
    }
}

fn infer_provider(value: &Value) -> Option<String> {
    let text = [
        string_path(value, &["provider"]),
        string_path(value, &["host"]),
        string_path(value, &["model"]),
        string_path(value, &["modelVersion"]),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
    .to_ascii_lowercase();

    if text.contains("openai") || text.contains("gpt") || text.contains("chatgpt") {
        Some("openai".into())
    } else if text.contains("anthropic") || text.contains("claude") {
        Some("anthropic".into())
    } else if text.contains("google") || text.contains("gemini") {
        Some("google".into())
    } else if text.contains("deepseek") {
        Some("deepseek".into())
    } else if text.contains("mistral") || text.contains("mixtral") {
        Some("mistral".into())
    } else if text.contains("cohere") {
        Some("cohere".into())
    } else if value.get("prompt_eval_count").is_some() || value.get("eval_count").is_some() {
        Some("ollama".into())
    } else {
        None
    }
}

fn host_for_provider(provider: &str) -> &'static str {
    match provider {
        "openai" => "api.openai.com",
        "anthropic" => "api.anthropic.com",
        "google" | "gemini" => "generativelanguage.googleapis.com",
        "deepseek" => "api.deepseek.com",
        "mistral" => "api.mistral.ai",
        "cohere" => "api.cohere.com",
        "ollama" => "127.0.0.1:11434",
        _ => "local",
    }
}

fn usage_subtree(value: &Value) -> Value {
    value
        .get("usage")
        .or_else(|| value.get("usageMetadata"))
        .or_else(|| value.get("message_delta").and_then(|m| m.get("usage")))
        .cloned()
        .unwrap_or_else(|| {
            let mut usage = Map::new();
            for key in ["prompt_eval_count", "eval_count", "total_duration"] {
                if let Some(v) = value.get(key) {
                    usage.insert(key.to_string(), v.clone());
                }
            }
            Value::Object(usage)
        })
}

fn timestamp_from_value(value: &Value) -> Option<DateTime<Utc>> {
    for key in ["occurred_at", "timestamp", "created_at", "time"] {
        if let Some(raw) = value.get(key).and_then(Value::as_str) {
            if let Ok(ts) = DateTime::parse_from_rfc3339(raw) {
                return Some(ts.with_timezone(&Utc));
            }
        }
    }
    None
}

fn string_path(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for part in path {
        current = current.get(*part)?;
    }
    current.as_str().map(str::to_string)
}

fn string_any(value: &Value, keys: &[&str]) -> Option<String> {
    let map = value.as_object()?;
    for key in keys {
        if let Some(raw) = map.get(*key).and_then(Value::as_str) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn agent_id_from_value(value: &Value) -> String {
    string_any(
        value,
        &[
            "agent_id",
            "agentId",
            "agent",
            "app",
            "process_name",
            "processName",
        ],
    )
    .unwrap_or_else(|| "unknown_agent".to_string())
}

fn agent_label_from_value(value: &Value) -> String {
    string_any(
        value,
        &[
            "agent_label",
            "agentLabel",
            "agent_name",
            "agentName",
            "app_name",
            "process_name",
        ],
    )
    .unwrap_or_else(|| agent_id_from_value(value))
}

fn looks_like_local_path(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() < 2 || trimmed.contains("://") {
        return false;
    }
    trimmed.contains(":\\")
        || trimmed.starts_with("\\\\")
        || trimmed.starts_with('/')
        || trimmed.contains('\\')
        || (trimmed.contains('/') && !looks_like_host(trimmed))
        || trimmed
            .rsplit(['\\', '/'])
            .next()
            .and_then(|leaf| leaf.rsplit_once('.'))
            .map(|(_, ext)| ext.len() <= 8 && ext.chars().all(|ch| ch.is_ascii_alphanumeric()))
            .unwrap_or(false)
}

fn is_likely_folder_key(value: &Value) -> bool {
    value
        .as_object()
        .map(|map| {
            map.keys().any(|key| {
                let key = key.to_ascii_lowercase();
                key.contains("folder")
                    || key.contains("directory")
                    || key == "cwd"
                    || key.contains("workspace")
            })
        })
        .unwrap_or(false)
}

fn mode_from_value(value: &Value) -> &'static str {
    let raw = string_any(
        value,
        &["mode", "access_mode", "action", "verb", "operation"],
    )
    .unwrap_or_default()
    .to_ascii_lowercase();
    if raw.contains("delete") || raw.contains("unlink") || raw.contains("remove") {
        "delete"
    } else if raw.contains("write")
        || raw.contains("save")
        || raw.contains("create")
        || raw.contains("update")
        || raw.contains("insert")
    {
        "write"
    } else if raw.contains("execute") || raw.contains("exec") || raw.contains("run") {
        "execute"
    } else if raw.contains("connect") {
        "connect"
    } else {
        "read"
    }
}

fn sql_operation(sql: &str) -> Option<&'static str> {
    let first = sql
        .split_whitespace()
        .next()?
        .trim_matches(|ch: char| !ch.is_ascii_alphabetic())
        .to_ascii_lowercase();
    Some(normalize_db_operation(&first))
}

fn normalize_db_operation(operation: &str) -> &'static str {
    match operation.trim().to_ascii_lowercase().as_str() {
        "select" | "show" | "describe" | "explain" | "read" => "read",
        "insert" | "update" | "upsert" | "merge" | "create" | "alter" | "write" => "write",
        "delete" | "drop" | "truncate" | "remove" => "delete",
        _ => "invoke",
    }
}

fn mode_for_db_operation(operation: &str) -> &'static str {
    match operation {
        "read" => "read",
        "delete" => "delete",
        "write" => "write",
        _ => "invoke",
    }
}

fn sql_table_name(sql: &str) -> Option<String> {
    let tokens = normalized_sql_tokens(sql);
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i].as_str();
        if matches!(token, "from" | "join" | "into" | "update" | "table") {
            if let Some(next) = tokens.get(i + 1) {
                if !is_sql_noise(next) {
                    return Some(next.trim_matches('"').trim_matches('`').to_string());
                }
            }
        }
        if token == "delete" && tokens.get(i + 1).map(String::as_str) == Some("from") {
            if let Some(next) = tokens.get(i + 2) {
                if !is_sql_noise(next) {
                    return Some(next.trim_matches('"').trim_matches('`').to_string());
                }
            }
        }
        i += 1;
    }
    None
}

fn normalized_sql_tokens(sql: &str) -> Vec<String> {
    sql.split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';' | '(' | ')'))
        .filter_map(|token| {
            let token = token
                .trim_matches(|ch: char| matches!(ch, '"' | '\'' | '`' | '[' | ']'))
                .to_ascii_lowercase();
            if token.is_empty() {
                None
            } else {
                Some(token)
            }
        })
        .collect()
}

fn is_sql_noise(token: &str) -> bool {
    matches!(
        token,
        "select" | "where" | "set" | "values" | "on" | "using" | "returning"
    )
}

fn sql_summary(sql: &str) -> String {
    let operation = sql_operation(sql).unwrap_or("invoke");
    let table = sql_table_name(sql).unwrap_or_else(|| "unknown_table".to_string());
    format!("{operation} {table}")
}

fn normalize_sql_for_hash(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut in_string = false;
    let mut last_space = false;
    for ch in sql.chars() {
        if ch == '\'' || ch == '"' {
            in_string = !in_string;
            if !out.ends_with('?') {
                out.push('?');
            }
            last_space = false;
        } else if in_string {
            continue;
        } else if ch.is_ascii_digit() {
            if !out.ends_with('?') {
                out.push('?');
            }
            last_space = false;
        } else if ch.is_whitespace() {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(ch.to_ascii_lowercase());
            last_space = false;
        }
    }
    out.trim().to_string()
}

fn normalize_target(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(rest) = trimmed.strip_prefix("https://") {
        rest.trim_end_matches('/').to_string()
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        rest.trim_end_matches('/').to_string()
    } else {
        trimmed.trim_end_matches('/').to_string()
    }
}

fn redact_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("<local-usage-log>/{name}"))
        .unwrap_or_else(|| "<local-usage-log>".to_string())
}

fn redact_local_path_string(path: &str) -> String {
    let mut redacted = path.trim().to_string();
    for env_key in ["USERPROFILE", "HOME"] {
        if let Ok(home) = std::env::var(env_key) {
            if !home.is_empty() {
                redacted = redacted.replace(&home, "<home>");
            }
        }
    }
    redacted
}

fn scan_bucket() -> String {
    let now = Utc::now().timestamp();
    (now - (now % 300)).to_string()
}

fn stable_event_id(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    for part in parts {
        hasher.update(b"|");
        hasher.update(part.as_bytes());
    }
    format!("{}_{}", prefix, hex::encode(&hasher.finalize()[..12]))
}

fn hash_hex(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(&hasher.finalize()[..16])
}

fn local_device_id() -> String {
    let seed = format!(
        "{}:{}:{}",
        std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "local".into()),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    format!("dev_{}", hash_hex(&seed))
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
