// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

#![deny(clippy::unwrap_used, clippy::expect_used)]

use dek_guard_pipeline::{event, GuardAction, GuardPipeline};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionPointOutcome {
    pub status_code: u16,
    pub action: String,
    pub reason: String,
    pub payload: Value,
    pub guard: Value,
}

pub fn evaluate_response_decision_point(
    payload: Value,
    force_redact: bool,
) -> DecisionPointOutcome {
    let pipeline = GuardPipeline::default();
    let guard = pipeline.scan_response(&payload);
    if guard.action == GuardAction::Deny {
        return DecisionPointOutcome {
            status_code: 403,
            action: "deny".to_string(),
            reason: "output_guard_blocked_risky_tool_response".to_string(),
            payload,
            guard: guard_metadata(&guard, false),
        };
    }

    let mut filtered_payload = payload;
    let mut reasons = Vec::new();
    let mut redaction_applied = false;
    let guard_payload_applied = if let Some(redacted_payload) = guard.redacted_payload.clone() {
        filtered_payload = redacted_payload;
        redaction_applied = true;
        reasons.push("spotlight_untrusted_data".to_string());
        true
    } else {
        false
    };

    if (force_redact || guard.action == GuardAction::Redact) && !guard_payload_applied {
        let (redacted, findings) =
            dek_guard_pipeline::output_guard::redact_value(&filtered_payload);
        filtered_payload = redacted;
        redaction_applied = redaction_applied || !findings.is_empty();
        reasons.push("redact_content".to_string());
    }

    if redaction_applied {
        DecisionPointOutcome {
            status_code: 200,
            action: "redact".to_string(),
            reason: reasons.join(","),
            payload: filtered_payload,
            guard: guard_metadata(&guard, true),
        }
    } else {
        DecisionPointOutcome {
            status_code: 200,
            action: "allow".to_string(),
            reason: "allow".to_string(),
            payload: filtered_payload,
            guard: guard_metadata(&guard, false),
        }
    }
}

fn guard_metadata(outcome: &dek_guard_pipeline::GuardOutcome, redaction_applied: bool) -> Value {
    let findings_summary = event::summarize_findings(&outcome.findings);
    let remediation = event::remediation_for(outcome.action, &outcome.categories);
    let severity = event::severity_for(outcome.action, &outcome.categories);
    json!({
        "plugin_id": "dek.guard-pipeline",
        "action": guard_action_label(outcome.action),
        "categories": outcome.categories,
        "confidence": outcome.confidence,
        "findings_count": outcome.findings.len(),
        "findings": findings_summary,
        "severity": severity,
        "remediation": remediation,
        "redaction": {
            "applied": redaction_applied
        },
    })
}

fn guard_action_label(action: GuardAction) -> &'static str {
    match action {
        GuardAction::Allow => "allow",
        GuardAction::Redact => "redact",
        GuardAction::Deny => "deny",
    }
}
