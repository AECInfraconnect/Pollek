//! Resource-access trace extraction from local agent session logs: file /
//! folder, web / email, shell command, and database traces (host- and
//! metadata-only; never raw content), plus local-log provenance stamping.

use super::*;

pub(super) fn extract_resource_trace_events_from_path(
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

pub(super) fn collect_resource_traces_from_value(
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
pub(super) fn unwrap_function_call_arguments(value: &Value) -> Option<Value> {
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

pub(super) fn resource_trace_event_from_value(
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
pub(super) fn web_or_email_trace(
    value: &Value,
) -> Option<(String, Value, &'static str, &'static str)> {
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
pub(super) fn is_email_service_host(host: &str) -> bool {
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
pub(super) fn command_trace(value: &Value) -> Option<(String, Value)> {
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

pub(super) fn file_or_folder_trace(
    value: &Value,
) -> Option<(String, Value, &'static str, &'static str)> {
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

pub(super) fn database_trace(value: &Value) -> Option<(String, Value, &'static str)> {
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

pub(super) fn add_local_log_provenance(details: &mut Value, source_path: &Path) {
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
