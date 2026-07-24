//! Leaf helpers for local-observe: SQL shape/summary, path & target
//! redaction, stable hashing, and device-id derivation. Pure functions with
//! no `AppState` or domain-model dependencies.

use super::*;

pub(super) fn sql_operation(sql: &str) -> Option<&'static str> {
    let first = sql
        .split_whitespace()
        .next()?
        .trim_matches(|ch: char| !ch.is_ascii_alphabetic())
        .to_ascii_lowercase();
    Some(normalize_db_operation(&first))
}

pub(super) fn normalize_db_operation(operation: &str) -> &'static str {
    match operation.trim().to_ascii_lowercase().as_str() {
        "select" | "show" | "describe" | "explain" | "read" => "read",
        "insert" | "update" | "upsert" | "merge" | "create" | "alter" | "write" => "write",
        "delete" | "drop" | "truncate" | "remove" => "delete",
        _ => "invoke",
    }
}

pub(super) fn mode_for_db_operation(operation: &str) -> &'static str {
    match operation {
        "read" => "read",
        "delete" => "delete",
        "write" => "write",
        _ => "invoke",
    }
}

pub(super) fn sql_table_name(sql: &str) -> Option<String> {
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

pub(super) fn normalized_sql_tokens(sql: &str) -> Vec<String> {
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

pub(super) fn is_sql_noise(token: &str) -> bool {
    matches!(
        token,
        "select" | "where" | "set" | "values" | "on" | "using" | "returning"
    )
}

pub(super) fn sql_summary(sql: &str) -> String {
    let operation = sql_operation(sql).unwrap_or("invoke");
    let table = sql_table_name(sql).unwrap_or_else(|| "unknown_table".to_string());
    format!("{operation} {table}")
}

pub(super) fn normalize_sql_for_hash(sql: &str) -> String {
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

pub(super) fn normalize_target(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(rest) = trimmed.strip_prefix("https://") {
        rest.trim_end_matches('/').to_string()
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        rest.trim_end_matches('/').to_string()
    } else {
        trimmed.trim_end_matches('/').to_string()
    }
}

pub(super) fn redact_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("<local-usage-log>/{name}"))
        .unwrap_or_else(|| "<local-usage-log>".to_string())
}

pub(super) fn redact_local_path_string(path: &str) -> String {
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

pub(super) fn scan_bucket() -> String {
    let now = Utc::now().timestamp();
    (now - (now % 300)).to_string()
}

pub(super) fn stable_event_id(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    for part in parts {
        hasher.update(b"|");
        hasher.update(part.as_bytes());
    }
    format!("{}_{}", prefix, hex::encode(&hasher.finalize()[..12]))
}

pub(super) fn hash_hex(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(&hasher.finalize()[..16])
}

pub(super) fn local_device_id() -> String {
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
