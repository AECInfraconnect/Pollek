//! Exact token-usage extraction: bridge usage from telemetry / local agent
//! session logs, discover and walk usage-log files, and parse Codex/other
//! rollout records into canonical AiUsageEventV1 (never storing prompt bodies).

use super::*;

pub(super) async fn bridge_exact_usage_from_telemetry(
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

pub(super) async fn bridge_exact_usage_from_local_logs(
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

pub(super) async fn bridge_detailed_resource_traces_from_local_logs(
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

pub(super) async fn collect_usage_log_paths_with_user_inputs(state: &AppState) -> Vec<PathBuf> {
    let mut paths = state.observe_accuracy_store.local_usage_log_paths().await;
    paths.extend(collect_usage_log_paths());
    paths.sort();
    paths.dedup();
    paths
}

pub(super) fn collect_usage_log_paths() -> Vec<PathBuf> {
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

pub(super) fn collect_usage_files(path: &Path, out: &mut Vec<PathBuf>, depth: usize) {
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

pub(super) fn is_usage_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "json" | "jsonl" | "ndjson" | "log"
    )
}

pub(super) fn extract_exact_usage_events_from_path(
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
pub(super) fn model_hint_from_line(value: &Value) -> Option<String> {
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
pub(super) fn codex_token_count_event(
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

pub(super) fn collect_exact_usage_from_value(
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

pub(super) fn exact_usage_event_from_raw_value(
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

pub(super) fn has_usage_object(value: &Value) -> bool {
    value.get("usage").is_some()
        || value.get("usageMetadata").is_some()
        || value.get("prompt_eval_count").is_some()
        || value.get("eval_count").is_some()
}

pub(super) async fn persist_estimated_presence_usage(
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
