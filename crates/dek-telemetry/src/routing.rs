// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! routing.rs — R2.1: map each telemetry event to its contract endpoint.
//!
//! The Cloud contract (docs/contracts/pollen-cloud-dek-api.md §5) splits
//! telemetry into typed endpoints instead of one firehose:
//!   /v1/telemetry/decision-logs   <- Decision
//!   /v1/telemetry/security-events <- Security
//!   /v1/telemetry/traces          <- Trace
//!   /v1/telemetry/ebpf-events     <- EbpfGuardrail
//!   /v1/metrics                   <- Metric  (also OTLP path)
//!   /v1/telemetry/events          <- everything else (OsGuardrail/OsLifecycle/…)
//!
//! The spooler stays a single queue; routing happens at flush time by reading
//! the event's serde tag. Unknown/!tagged events fall back to /events so we
//! never drop telemetry.

/// Returns the path suffix (appended to the cloud base URL) for an event,
/// based on its serde-tagged kind. The tag field is `event_type` (see
/// dek-domain-schema::TelemetryEvent `#[serde(tag = "event_type", ...)]`).
pub fn endpoint_for(event: &serde_json::Value) -> &'static str {
    match event.get("event_type").and_then(|v| v.as_str()) {
        Some("decision") | Some("decision_log") => "/v1/telemetry/decision-logs",
        Some("security") | Some("security_event") => "/v1/telemetry/security-events",
        Some("trace") => "/v1/telemetry/traces",
        Some("ebpf_guardrail") | Some("os_guardrail_event") => "/v1/telemetry/ebpf-events",
        Some("metric") | Some("runtime_metric") => "/v1/metrics",
        _ => "/v1/telemetry/events",
    }
}

/// Group a batch of events by their target endpoint so the flusher can POST
/// one request per endpoint (preserves batching while honoring the contract).
pub fn group_by_endpoint(
    events: Vec<(i64, serde_json::Value)>,
) -> std::collections::HashMap<&'static str, (Vec<i64>, Vec<serde_json::Value>)> {
    let mut map: std::collections::HashMap<&'static str, (Vec<i64>, Vec<serde_json::Value>)> =
        std::collections::HashMap::new();
    for (id, ev) in events {
        let ep = endpoint_for(&ev);
        let entry = map.entry(ep).or_default();
        entry.0.push(id);
        entry.1.push(ev);
    }
    map
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use serde_json::json;

    #[test]
    fn routes_by_event_type() {
        assert_eq!(
            endpoint_for(&json!({"event_type":"decision"})),
            "/v1/telemetry/decision-logs"
        );
        assert_eq!(
            endpoint_for(&json!({"event_type":"decision_log"})),
            "/v1/telemetry/decision-logs"
        );
        assert_eq!(
            endpoint_for(&json!({"event_type":"security"})),
            "/v1/telemetry/security-events"
        );
        assert_eq!(
            endpoint_for(&json!({"event_type":"security_event"})),
            "/v1/telemetry/security-events"
        );
        assert_eq!(
            endpoint_for(&json!({"event_type":"trace"})),
            "/v1/telemetry/traces"
        );
        assert_eq!(
            endpoint_for(&json!({"event_type":"ebpf_guardrail"})),
            "/v1/telemetry/ebpf-events"
        );
        assert_eq!(
            endpoint_for(&json!({"event_type":"os_guardrail_event"})),
            "/v1/telemetry/ebpf-events"
        );
        assert_eq!(endpoint_for(&json!({"event_type":"metric"})), "/v1/metrics");
        assert_eq!(
            endpoint_for(&json!({"event_type":"runtime_metric"})),
            "/v1/metrics"
        );
        // unknown / audit -> generic events
        assert_eq!(
            endpoint_for(&json!({"event_type":"os_lifecycle"})),
            "/v1/telemetry/events"
        );
        assert_eq!(
            endpoint_for(&json!({"event_type":"audit"})),
            "/v1/telemetry/events"
        );
        assert_eq!(endpoint_for(&json!({})), "/v1/telemetry/events");
    }

    #[test]
    fn groups_batch() {
        let batch = vec![
            (1, json!({"event_type":"decision"})),
            (2, json!({"event_type":"decision"})),
            (3, json!({"event_type":"security"})),
        ];
        let g = group_by_endpoint(batch);
        assert_eq!(g.get("/v1/telemetry/decision-logs").unwrap().0.len(), 2);
        assert_eq!(g.get("/v1/telemetry/security-events").unwrap().0.len(), 1);
    }
}
