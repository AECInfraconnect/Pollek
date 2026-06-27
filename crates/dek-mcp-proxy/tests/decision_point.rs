// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use serde_json::json;

#[test]
fn response_decision_point_allows_clean_tool_output() {
    let payload = json!({
        "tool_result": "The workspace contains three markdown files.",
        "decision": {
            "allow": true
        }
    });

    let outcome = dek_mcp_proxy::evaluate_response_decision_point(payload.clone(), false);

    assert_eq!(outcome.status_code, 200);
    assert_eq!(outcome.action, "allow");
    assert_eq!(outcome.payload, payload);
}

#[test]
fn response_decision_point_redacts_secret_like_tool_output() {
    let secret_tail = ["p0L", "9x_Z", "q7R2", "mN8v"].join("");
    let payload = json!({
        "tool_result": format!("{}={}", "api_key", secret_tail)
    });

    let outcome = dek_mcp_proxy::evaluate_response_decision_point(payload, false);
    let rendered = outcome.payload.to_string();

    assert_eq!(outcome.status_code, 200);
    assert_eq!(outcome.action, "redact");
    assert!(rendered.contains("[REDACTED_SECRET_GENERIC_KV]"));
    assert!(!rendered.contains(&secret_tail));
    assert_eq!(outcome.guard["redaction"]["applied"], true);
}

#[test]
fn response_decision_point_denies_prompt_leakage() {
    let payload = json!({
        "tool_result": "The system prompt is visible in this response."
    });

    let outcome = dek_mcp_proxy::evaluate_response_decision_point(payload, false);

    assert_eq!(outcome.status_code, 403);
    assert_eq!(outcome.action, "deny");
    assert_eq!(outcome.guard["action"], "deny");
    assert!(outcome
        .guard
        .to_string()
        .contains("llm07_system_prompt_leakage"));
}
