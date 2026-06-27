// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_plugin_sdk::RedactionFinding;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputFinding {
    pub kind: String,
    pub start: usize,
    pub end: usize,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputGuardReport {
    pub secrets: Vec<OutputFinding>,
    pub risky_markup: Vec<OutputFinding>,
    pub prompt_leak: bool,
    pub canary_leak: bool,
}

static RE_OPENAI_KEY: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"\bsk-[A-Za-z0-9]{20,}\b"));
static RE_AWS_AKID: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"\bAKIA[0-9A-Z]{16}\b"));
static RE_SLACK_BOT: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"\bxoxb-[0-9A-Za-z\-]{10,}\b"));
static RE_GH_PAT: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"\bghp_[A-Za-z0-9]{36}\b"));
static RE_JWT: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"\beyJ[A-Za-z0-9_\-]+\.eyJ[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+\b"));
static RE_PRIVATE_KEY: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----"));
static RE_GENERIC_KV: Lazy<Result<Regex, regex::Error>> = Lazy::new(|| {
    Regex::new(
        r#"(?i)\b(?:api[_-]?key|secret|password|access[_-]?token)\s*[:=]\s*['"]?(?P<value>[A-Za-z0-9_\-\.]{12,})"#,
    )
});

static RE_SCRIPT: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"(?i)<script\b[^>]*>?"));
static RE_JAVASCRIPT_URL: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"(?i)javascript:"));
static RE_EVENT_HANDLER: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"(?i)\bon(?:error|load)\s*="));
static RE_DATA_HTML: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"(?i)data:text/html"));
static RE_EMBED_OBJECT: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"(?i)<(?:iframe|object)\b[^>]*>?"));
static RE_MARKDOWN_JS: Lazy<Result<Regex, regex::Error>> =
    Lazy::new(|| Regex::new(r"(?i)\]\(\s*javascript:"));

const SECRET_PATTERNS: &[(&str, &Lazy<Result<Regex, regex::Error>>)] = &[
    ("OPENAI_KEY", &RE_OPENAI_KEY),
    ("AWS_AKID", &RE_AWS_AKID),
    ("SLACK_BOT", &RE_SLACK_BOT),
    ("GH_PAT", &RE_GH_PAT),
    ("JWT", &RE_JWT),
    ("PRIVATE_KEY", &RE_PRIVATE_KEY),
];

const RISKY_MARKUP_PATTERNS: &[(&str, &Lazy<Result<Regex, regex::Error>>)] = &[
    ("SCRIPT_TAG", &RE_SCRIPT),
    ("JAVASCRIPT_URL", &RE_JAVASCRIPT_URL),
    ("EVENT_HANDLER", &RE_EVENT_HANDLER),
    ("DATA_HTML", &RE_DATA_HTML),
    ("EMBED_OBJECT", &RE_EMBED_OBJECT),
    ("MARKDOWN_JAVASCRIPT", &RE_MARKDOWN_JS),
];

const PROMPT_LEAK_NEEDLES: &[&str] = &[
    "you are pollek",
    "system prompt",
    "developer message",
    "hidden instructions",
    "internal policy",
    "my instructions are",
    "<<untrusted_data",
];

pub fn scan_output(text: &str, canary: Option<&str>) -> OutputGuardReport {
    let lower = text.to_ascii_lowercase();
    OutputGuardReport {
        secrets: detect_secrets(text),
        risky_markup: detect_risky_markup(text),
        prompt_leak: detect_prompt_leak(&lower),
        canary_leak: canary
            .filter(|token| !token.is_empty())
            .is_some_and(|token| text.contains(token)),
    }
}

pub fn shannon_entropy(value: &str) -> f32 {
    if value.is_empty() {
        return 0.0;
    }

    let mut counts = HashMap::new();
    for ch in value.chars() {
        let count = counts.entry(ch).or_insert(0u32);
        *count += 1;
    }
    let len = value.chars().count() as f32;
    -counts
        .values()
        .map(|count| {
            let probability = *count as f32 / len;
            probability * probability.log2()
        })
        .sum::<f32>()
}

pub fn detect_secrets(text: &str) -> Vec<OutputFinding> {
    let mut findings = Vec::new();
    for (kind, regex) in SECRET_PATTERNS {
        let Ok(regex) = regex.as_ref() else {
            continue;
        };
        for matched in regex.find_iter(text) {
            findings.push(OutputFinding {
                kind: format!("SECRET_{kind}"),
                start: matched.start(),
                end: matched.end(),
                confidence: 0.99,
            });
        }
    }

    if let Ok(regex) = RE_GENERIC_KV.as_ref() {
        for captures in regex.captures_iter(text) {
            let Some(value) = captures.name("value") else {
                continue;
            };
            if shannon_entropy(value.as_str()) < 3.0 {
                continue;
            }
            findings.push(OutputFinding {
                kind: "SECRET_GENERIC_KV".to_string(),
                start: value.start(),
                end: value.end(),
                confidence: 0.85,
            });
        }
    }

    dedupe_overlaps(findings)
}

pub fn detect_risky_markup(text: &str) -> Vec<OutputFinding> {
    let mut findings = Vec::new();
    for (kind, regex) in RISKY_MARKUP_PATTERNS {
        let Ok(regex) = regex.as_ref() else {
            continue;
        };
        for matched in regex.find_iter(text) {
            findings.push(OutputFinding {
                kind: format!("RISKY_MARKUP_{kind}"),
                start: matched.start(),
                end: matched.end(),
                confidence: 0.90,
            });
        }
    }
    dedupe_overlaps(findings)
}

pub fn detect_prompt_leak(output_lower: &str) -> bool {
    PROMPT_LEAK_NEEDLES
        .iter()
        .any(|needle| output_lower.contains(needle))
}

pub fn redact_value(value: &Value) -> (Value, Vec<RedactionFinding>) {
    let mut findings = Vec::new();
    let redacted = redact_value_at_path(value, "$", &mut findings);
    (redacted, findings)
}

fn redact_value_at_path(value: &Value, path: &str, findings: &mut Vec<RedactionFinding>) -> Value {
    match value {
        Value::String(text) => {
            let (redacted, mut local_findings) = redact_text_at_path(text, path);
            findings.append(&mut local_findings);
            Value::String(redacted)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    redact_value_at_path(item, &format!("{path}[{index}]"), findings)
                })
                .collect(),
        ),
        Value::Object(map) => {
            let mut redacted = Map::new();
            for (key, child) in map {
                redacted.insert(
                    key.clone(),
                    redact_value_at_path(child, &format!("{path}.{key}"), findings),
                );
            }
            Value::Object(redacted)
        }
        _ => value.clone(),
    }
}

fn redact_text_at_path(text: &str, path: &str) -> (String, Vec<RedactionFinding>) {
    let mut findings = detect_secrets(text);
    findings.extend(detect_risky_markup(text));
    let findings = dedupe_overlaps(findings);
    let mut redacted = text.to_string();
    let mut redactions = Vec::new();

    for finding in findings.iter().rev() {
        let replacement = format!("[REDACTED_{}]", finding.kind);
        redacted.replace_range(finding.start..finding.end, &replacement);
        redactions.push(RedactionFinding {
            kind: finding.kind.clone(),
            confidence: finding.confidence,
            path: format!("{path}:offset:{}", finding.start),
            replacement,
        });
    }

    (redacted, redactions)
}

fn dedupe_overlaps(mut findings: Vec<OutputFinding>) -> Vec<OutputFinding> {
    findings.sort_by(|left, right| right.confidence.total_cmp(&left.confidence));
    let mut selected: Vec<OutputFinding> = Vec::new();
    for finding in findings {
        if !selected
            .iter()
            .any(|existing| ranges_overlap(existing, &finding))
        {
            selected.push(finding);
        }
    }
    selected.sort_by_key(|finding| finding.start);
    selected
}

fn ranges_overlap(left: &OutputFinding, right: &OutputFinding) -> bool {
    left.start < right.end && right.start < left.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_openai_secret_key() {
        let findings = detect_secrets("tool echoed sk-1234567890abcdefghijklmnop");

        assert!(findings
            .iter()
            .any(|finding| finding.kind == "SECRET_OPENAI_KEY"));
    }

    #[test]
    fn generic_kv_low_entropy_is_not_secret() {
        let findings = detect_secrets("password=aaaaaaaaaaaaaa");

        assert!(!findings
            .iter()
            .any(|finding| finding.kind == "SECRET_GENERIC_KV"));
    }

    #[test]
    fn generic_kv_high_entropy_is_secret() {
        let findings = detect_secrets("api_key=aB93kLm22_Zxq900");

        assert!(findings
            .iter()
            .any(|finding| finding.kind == "SECRET_GENERIC_KV"));
    }

    #[test]
    fn canary_token_marks_prompt_leak() {
        let report = scan_output(
            "model leaked POLLEK_CANARY_TEST",
            Some("POLLEK_CANARY_TEST"),
        );

        assert!(report.canary_leak);
    }

    #[test]
    fn risky_markup_is_redacted_in_nested_payload() {
        let (redacted, findings) = redact_value(&json!({
            "html": "<script>alert(1)</script>"
        }));
        let rendered = redacted.to_string();

        assert!(rendered.contains("[REDACTED_RISKY_MARKUP_SCRIPT_TAG]"));
        assert!(!rendered.contains("<script"));
        assert!(findings
            .iter()
            .any(|finding| finding.kind == "RISKY_MARKUP_SCRIPT_TAG"));
    }

    #[test]
    fn prompt_leak_needles_are_detected() {
        assert!(detect_prompt_leak("the system prompt is visible"));
        assert!(!detect_prompt_leak("normal answer"));
    }
}
