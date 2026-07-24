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

mod publish;
mod resource_trace;
mod usage_extract;
mod util;
mod value;
use publish::*;
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
