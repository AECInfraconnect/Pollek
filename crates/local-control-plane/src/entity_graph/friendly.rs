//! The user-friendly activity layer: translate raw timeline items into
//! human-readable agent names, targets, categories, actions, results, and
//! capability notes for the AI Activity page. Pure presentation helpers.

use super::*;

pub(super) fn user_friendly_activity_from_timeline(
    item: &ActivityTimelineItem,
) -> UserFriendlyActivityEvent {
    let category = infer_user_activity_category(item);
    let action = infer_user_activity_action(item, &category);
    let result = infer_user_activity_result(item);
    let raw_agent_label = item.actor.as_ref().map(|actor| actor.label.clone());
    let agent_id = item.actor.as_ref().map(|actor| actor.entity_id.clone());
    let agent_name = friendly_agent_name(raw_agent_label.as_deref(), agent_id.as_deref());
    let target = friendly_target_label(&user_activity_target(item), &category);
    let plain_summary = user_activity_summary(&agent_name, &action, &target, &category);

    UserFriendlyActivityEvent {
        schema_version: "user-friendly-activity.v1".to_string(),
        event_id: item.event_id.clone(),
        timestamp: item.timestamp.clone(),
        agent_id,
        agent_name,
        category: category.clone(),
        action: action.clone(),
        target_label: target,
        target_kind: category_label(&category).to_string(),
        access_mode: access_mode(&action).to_string(),
        result: result.clone(),
        result_label: result_label(&result).to_string(),
        plain_summary,
        rule_label: item.policies.first().map(|policy| policy.label.clone()),
        capability_note: capability_note(&result, &category).to_string(),
        next_step: next_step(&result, &category).to_string(),
        privacy_note: "Pollek shows activity metadata here, not file contents, email bodies, raw prompts, or raw responses.".to_string(),
        cost_usd: item.cost.as_ref().and_then(|cost| cost.total_cost_usd),
        tokens: item.cost.as_ref().and_then(|cost| cost.total_tokens),
        trace_id: item.trace_id.clone(),
        advanced: UserFriendlyActivityAdvanced {
            raw_item: None,
            raw_agent_label,
            decision: Some(item.decision.clone()),
            mode: Some(item.enforcement_mode.clone()),
            pep_plane: item.pep_plane.clone(),
            pdp_engine: item.pdp_engine.clone(),
        },
    }
}

pub(super) fn known_agent_name(value: Option<&str>) -> Option<&'static str> {
    let text = value?.to_lowercase();
    if text.contains("pollek-plugin-marketplace") {
        Some("Pollek Plugin Marketplace")
    } else if text.contains("antigravity") || text.contains("gemini") {
        Some("Google Antigravity")
    } else if text.contains("chatgpt") || text.contains("openai") {
        Some("ChatGPT")
    } else if text.contains("claude") || text.contains("anthropic") {
        Some("Claude")
    } else if text.contains("codex") {
        Some("Codex")
    } else if text.contains("deepseek") {
        Some("DeepSeek")
    } else if text.contains("manus") {
        Some("Manus AI")
    } else {
        None
    }
}

pub(super) fn compact_raw_id(value: &str) -> Option<String> {
    let candidate = value
        .trim()
        .trim_start_matches("agent_")
        .trim_start_matches("agent-")
        .trim_start_matches("agent:");
    let compact: String = candidate
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .take(8)
        .collect();
    if compact.len() >= 6 {
        Some(compact)
    } else {
        None
    }
}

pub(super) fn looks_like_raw_id(value: Option<&str>) -> bool {
    let Some(value) = value else {
        return true;
    };
    let text = value.trim().to_lowercase();
    if text.is_empty()
        || text == "unknown"
        || text == "unknown ai app"
        || text.contains("unknown-observed-session")
    {
        return true;
    }
    if text.starts_with("agent_") || text.starts_with("agent-") || text.starts_with("agent:") {
        let idish = text
            .chars()
            .filter(|ch| ch.is_ascii_hexdigit() || *ch == '-')
            .count();
        return idish >= 8;
    }
    text.len() >= 16 && text.chars().all(|ch| ch.is_ascii_hexdigit() || ch == '-')
}

pub(super) fn friendly_agent_name(label: Option<&str>, id: Option<&str>) -> String {
    if let Some(name) = known_agent_name(label).or_else(|| known_agent_name(id)) {
        return name.to_string();
    }
    if !looks_like_raw_id(label) {
        return label.unwrap_or("Unknown AI app").trim().to_string();
    }
    let suffix = id
        .and_then(compact_raw_id)
        .or_else(|| label.and_then(compact_raw_id));
    suffix
        .map(|value| format!("Unidentified AI app ({value})"))
        .unwrap_or_else(|| "Unidentified AI app".to_string())
}

pub(super) fn friendly_target_label(label: &str, category: &str) -> String {
    if let Some(name) = known_agent_name(Some(label)) {
        return name.to_string();
    }
    let text = label.trim();
    let lower = text.to_lowercase();
    if text.is_empty() || lower == "an unknown target" || lower == "unknown" {
        return match category {
            "files" => "a file or folder Pollek could not name",
            "web" => "a website or network destination",
            "commands" | "apps" => "a local app or command",
            "ai_models" => "an AI model session",
            _ => "local AI activity",
        }
        .to_string();
    }
    if lower.contains("unknown-observed-session") {
        return if category == "ai_models" || category == "cost" {
            "AI model usage observed from this session"
        } else {
            "AI session activity"
        }
        .to_string();
    }
    if looks_like_raw_id(Some(text)) {
        return "local AI session".to_string();
    }
    text.to_string()
}

pub(super) fn user_activity_summary(
    agent_name: &str,
    action: &str,
    target: &str,
    category: &str,
) -> String {
    if target == "AI session activity" || target == "local AI activity" {
        return format!("{agent_name} had activity Pollek could observe");
    }
    if category == "ai_models" && target.contains("AI model") {
        return format!("{agent_name} used an AI model session");
    }
    format!("{agent_name} {} {target}", action_text(action))
}

pub(super) fn user_activity_raw_text(item: &ActivityTimelineItem) -> String {
    [
        Some(item.action.as_str()),
        item.actor.as_ref().map(|actor| actor.label.as_str()),
        item.tool.as_ref().map(|tool| tool.label.as_str()),
        item.resource
            .as_ref()
            .map(|resource| resource.label.as_str()),
        item.resource
            .as_ref()
            .map(|resource| resource.entity_type.as_str()),
        item.explanation.as_deref(),
        Some(item.decision.as_str()),
        Some(item.enforcement_mode.as_str()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
    .to_lowercase()
}

pub(super) fn infer_user_activity_category(item: &ActivityTimelineItem) -> String {
    let text = user_activity_raw_text(item);
    let resource_type = item
        .resource
        .as_ref()
        .map(|resource| resource.entity_type.to_lowercase())
        .unwrap_or_default();
    let tool_type = item
        .tool
        .as_ref()
        .map(|tool| tool.entity_type.to_lowercase())
        .unwrap_or_default();

    if text.contains("plugin")
        || text.contains("marketplace")
        || text.contains("connector")
        || text.contains("definition feed")
        || resource_type.contains("plugin")
    {
        return "plugins".to_string();
    }
    if text.contains("prompt")
        || text.contains("injection")
        || text.contains("redact")
        || text.contains("mask")
        || text.contains("pii")
        || text.contains("secret")
        || text.contains("credential")
        || text.contains("unsafe output")
        || text.contains("guard")
    {
        return "safety".to_string();
    }
    if resource_type.contains("file")
        || resource_type.contains("folder")
        || text.contains("file")
        || text.contains("folder")
        || text.contains("read")
        || text.contains("write")
    {
        return "files".to_string();
    }
    if resource_type.contains("domain")
        || resource_type.contains("url")
        || text.contains("http")
        || text.contains("network")
        || text.contains("domain")
        || text.contains("connect")
    {
        return "web".to_string();
    }
    if text.contains("email") || text.contains("calendar") {
        return "email".to_string();
    }
    if tool_type.contains("terminal")
        || text.contains("terminal")
        || text.contains("shell")
        || text.contains("command")
    {
        return "commands".to_string();
    }
    if text.contains("model") || text.contains("token") || text.contains("llm") {
        return "ai_models".to_string();
    }
    if item.tool.is_some() {
        return "tools".to_string();
    }
    if item
        .cost
        .as_ref()
        .map(|cost| cost.total_cost_usd.is_some() || cost.total_tokens.is_some())
        .unwrap_or(false)
    {
        return "cost".to_string();
    }
    if text.contains("process") || text.contains("app") {
        return "apps".to_string();
    }
    "unknown".to_string()
}

pub(super) fn infer_user_activity_action(item: &ActivityTimelineItem, category: &str) -> String {
    let text = user_activity_raw_text(item);
    if text.contains("write") || text.contains("delete") || text.contains("edit") {
        return "write".to_string();
    }
    if text.contains("read") || text.contains("open") {
        return "read".to_string();
    }
    match category {
        "web" => "connect".to_string(),
        "commands" | "apps" => "run".to_string(),
        "plugins" => {
            if text.contains("uninstall") {
                "uninstall".to_string()
            } else if text.contains("disable") {
                "disable".to_string()
            } else if text.contains("enable") {
                "enable".to_string()
            } else if text.contains("health") {
                "check".to_string()
            } else {
                "install".to_string()
            }
        }
        "email" if text.contains("send") => "send".to_string(),
        "email" => "read".to_string(),
        "ai_models" => "use_model".to_string(),
        "tools" => "call_tool".to_string(),
        "safety" => "redact".to_string(),
        "cost" => "spend".to_string(),
        _ => "watch".to_string(),
    }
}

pub(super) fn infer_user_activity_result(item: &ActivityTimelineItem) -> String {
    let decision = item.decision.to_lowercase();
    let mode = item.enforcement_mode.to_lowercase();
    if decision == "redact" || decision == "mask" {
        return "redacted".to_string();
    }
    if decision == "deny" || decision == "blocked" {
        return "blocked".to_string();
    }
    if decision == "error" {
        return "error".to_string();
    }
    if decision == "warn" {
        return "warned".to_string();
    }
    if decision == "require_approval" {
        return "asked_first".to_string();
    }
    if decision == "asked_and_allowed" {
        return "asked_and_allowed".to_string();
    }
    if decision == "asked_and_denied" {
        return "asked_and_denied".to_string();
    }
    if mode.contains("observe") || decision == "observe" {
        return "watched_only".to_string();
    }
    "allowed".to_string()
}

pub(super) fn user_activity_target(item: &ActivityTimelineItem) -> String {
    item.resource
        .as_ref()
        .map(|resource| resource.label.clone())
        .or_else(|| item.tool.as_ref().map(|tool| tool.label.clone()))
        .or_else(|| item.cost.as_ref().and_then(|cost| cost.model.clone()))
        .or_else(|| item.cost.as_ref().and_then(|cost| cost.provider.clone()))
        .unwrap_or_else(|| "an unknown target".to_string())
}

pub(super) fn category_label(category: &str) -> &'static str {
    match category {
        "files" => "Files & folders",
        "web" => "Websites & network",
        "email" => "Email & calendar",
        "apps" => "Apps",
        "commands" => "Commands",
        "ai_models" => "AI models",
        "tools" => "AI tools",
        "plugins" => "Plugins & connectors",
        "safety" => "Prompt & data safety",
        "cost" => "Cost",
        _ => "Other activity",
    }
}

pub(super) fn action_text(action: &str) -> &'static str {
    match action {
        "read" => "read",
        "write" => "changed",
        "connect" => "connected to",
        "run" => "ran",
        "send" => "sent",
        "use_model" => "used",
        "call_tool" => "called",
        "install" => "installed",
        "enable" => "enabled",
        "disable" => "disabled",
        "uninstall" => "uninstalled",
        "check" => "checked",
        "redact" => "protected",
        "spend" => "spent tokens on",
        _ => "was seen using",
    }
}

pub(super) fn access_mode(action: &str) -> &'static str {
    match action {
        "read" | "use_model" | "call_tool" => "read",
        "write" => "write",
        "connect" => "connect",
        "run" => "run",
        "send" => "send",
        "install" | "enable" | "disable" | "uninstall" | "check" => "manage",
        _ => "unknown",
    }
}

pub(super) fn result_label(result: &str) -> &'static str {
    match result {
        "allowed" => "Allowed",
        "blocked" => "Blocked",
        "asked_first" => "Ask first",
        "asked_and_allowed" => "Asked and allowed",
        "asked_and_denied" => "Asked and blocked",
        "watched_only" => "Watched only",
        "warned" => "Warned",
        "redacted" => "Redacted",
        "error" => "Error",
        _ => "Unknown",
    }
}

pub(super) fn capability_note(result: &str, category: &str) -> &'static str {
    if result == "blocked" {
        return "Pollek blocked this action.";
    }
    if result == "redacted" {
        return "Pollek removed or masked sensitive content before it could continue.";
    }
    if result == "allowed" {
        return "Pollek saw this action and it was allowed.";
    }
    if result == "warned" {
        return "Pollek warned about this action.";
    }
    if result == "asked_first" {
        return "Pollek can ask before this kind of action.";
    }
    if category == "files" || category == "web" || category == "commands" {
        return "Pollek can watch this now. Blocking may require OS setup or an agent-specific setting.";
    }
    if category == "safety" {
        return "Pollek can watch prompt and data-safety signals. Blocking or redaction depends on which guard is in the AI app path.";
    }
    if category == "plugins" {
        return "Pollek recorded this plugin registry change so you can audit what extensions were enabled, disabled, or removed.";
    }
    "Pollek can watch this activity and explain what to review next."
}

fn next_step(result: &str, category: &str) -> &'static str {
    if result == "blocked" {
        return "Review the rule if this should be allowed next time.";
    }
    if result == "redacted" {
        return "Review the safety rule and confirm the AI app is using the guard path for prompts and outputs.";
    }
    match category {
        "files" => {
            "Set a rule for this folder, or restrict file access inside the AI app settings."
        }
        "web" => "Set an approved website rule, or restrict network access in the AI app settings.",
        "commands" | "apps" => {
            "Ask before commands, or disable command execution inside the AI app."
        }
        "email" => "Keep email access opt-in and review the connector permissions.",
        "plugins" => "Review installed plugins, granted capabilities, and whether any connector can send data off this device.",
        "safety" => {
            "Keep watching, enable Prompt Guard for this AI app, or tighten the AI app's own safety settings."
        }
        _ => "Keep watching or create a rule from similar activity.",
    }
}
