// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! Pollek Cloud AI cost & token usage reporting.
//!
//! Local Control Planes push `ai_usage_event` telemetry envelopes to
//! `/v1/telemetry/batches`; the telemetry ingest path flattens each into the
//! [`crate::state::UsageLedger`]. These endpoints roll that ledger up into
//! reports grouped by device, user, agent, tenant, model, or provider.

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use std::collections::BTreeMap;

use crate::state::{AppState, CloudUsageRecord};

pub fn router() -> Router<AppState> {
    Router::new()
        // Cross-tenant report (defaults to group_by=tenant).
        .route("/v1/usage/summary", get(usage_summary_all))
        // Per-tenant report (defaults to group_by=device).
        .route(
            "/v1/tenants/:tenant/usage/summary",
            get(usage_summary_tenant),
        )
        // Raw per-tenant records, for verification/debugging.
        .route(
            "/v1/tenants/:tenant/usage/records",
            get(usage_records_tenant),
        )
}

#[derive(serde::Deserialize, Default)]
pub struct UsageQuery {
    /// device | user | agent | tenant | model | provider
    pub group_by: Option<String>,
    /// RFC3339 lower bound (inclusive) on `occurred_at`.
    pub from: Option<String>,
    /// RFC3339 upper bound (inclusive) on `occurred_at`.
    pub to: Option<String>,
    pub limit: Option<usize>,
}

#[derive(serde::Serialize, Default, Clone)]
struct UsageTotals {
    request_count: u64,
    input_tokens: i64,
    output_tokens: i64,
    total_tokens: i64,
    total_cost: f64,
}

impl UsageTotals {
    fn add(&mut self, r: &CloudUsageRecord) {
        self.request_count += 1;
        self.input_tokens += r.input_tokens;
        self.output_tokens += r.output_tokens;
        self.total_tokens += r.total_tokens;
        self.total_cost += r.total_cost;
    }
}

#[derive(serde::Serialize)]
struct UsageGroup {
    /// Grouping dimension value, e.g. the device id or hashed user id.
    key: String,
    #[serde(flatten)]
    totals: UsageTotals,
    /// Distinct counts of the other dimensions within this group, so a
    /// per-tenant row can still say "3 devices / 5 users / 2 agents".
    devices: usize,
    users: usize,
    agents: usize,
    models: usize,
}

fn normalize_group_by(raw: Option<&str>, default: &str) -> String {
    match raw.unwrap_or(default) {
        "device" | "devices" => "device",
        "user" | "users" => "user",
        "agent" | "agents" => "agent",
        "tenant" | "tenants" => "tenant",
        "model" | "models" => "model",
        "provider" | "providers" => "provider",
        _ => default,
    }
    .to_string()
}

fn dimension_value(record: &CloudUsageRecord, group_by: &str) -> String {
    match group_by {
        "device" => record.device_id.clone(),
        "user" => record.user_id.clone(),
        "agent" => record.agent_id.clone(),
        "tenant" => record.tenant_id.clone(),
        "model" => record.model.clone(),
        "provider" => record.provider.clone(),
        _ => "unknown".to_string(),
    }
}

fn within_window(record: &CloudUsageRecord, from: &Option<String>, to: &Option<String>) -> bool {
    if let Some(from) = from {
        if record.occurred_at.as_str() < from.as_str() {
            return false;
        }
    }
    if let Some(to) = to {
        if record.occurred_at.as_str() > to.as_str() {
            return false;
        }
    }
    true
}

/// Builds a grouped report over `records` already filtered to the desired
/// tenant scope and time window.
fn build_report(
    tenant_scope: &str,
    group_by: &str,
    records: &[CloudUsageRecord],
) -> serde_json::Value {
    let mut totals = UsageTotals::default();
    let mut groups: BTreeMap<String, (UsageTotals, DistinctSets)> = BTreeMap::new();
    let mut currency = "USD".to_string();

    for record in records {
        totals.add(record);
        if !record.currency.is_empty() {
            currency = record.currency.clone();
        }
        let key = dimension_value(record, group_by);
        let entry = groups.entry(key).or_default();
        entry.0.add(record);
        entry.1.devices.insert(record.device_id.clone());
        entry.1.users.insert(record.user_id.clone());
        entry.1.agents.insert(record.agent_id.clone());
        entry.1.models.insert(record.model.clone());
    }

    let mut group_list: Vec<UsageGroup> = groups
        .into_iter()
        .map(|(key, (totals, distinct))| UsageGroup {
            key,
            totals,
            devices: distinct.devices.len(),
            users: distinct.users.len(),
            agents: distinct.agents.len(),
            models: distinct.models.len(),
        })
        .collect();
    // Highest cost first; ties broken by token volume then key for stability.
    group_list.sort_by(|a, b| {
        b.totals
            .total_cost
            .partial_cmp(&a.totals.total_cost)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.totals.total_tokens.cmp(&a.totals.total_tokens))
            .then(a.key.cmp(&b.key))
    });

    serde_json::json!({
        "schema_version": "usage-report.v1",
        "tenant_id": tenant_scope,
        "group_by": group_by,
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "currency": currency,
        "totals": totals,
        "groups": group_list,
    })
}

#[derive(Default)]
struct DistinctSets {
    devices: std::collections::HashSet<String>,
    users: std::collections::HashSet<String>,
    agents: std::collections::HashSet<String>,
    models: std::collections::HashSet<String>,
}

async fn usage_summary_all(
    State(s): State<AppState>,
    Query(q): Query<UsageQuery>,
) -> Json<serde_json::Value> {
    let group_by = normalize_group_by(q.group_by.as_deref(), "tenant");
    let ledger = s
        .usage_ledger
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let records: Vec<CloudUsageRecord> = ledger
        .records
        .iter()
        .filter(|r| within_window(r, &q.from, &q.to))
        .cloned()
        .collect();
    drop(ledger);
    Json(build_report("*", &group_by, &records))
}

async fn usage_summary_tenant(
    Path(tenant): Path<String>,
    State(s): State<AppState>,
    Query(q): Query<UsageQuery>,
) -> Json<serde_json::Value> {
    let group_by = normalize_group_by(q.group_by.as_deref(), "device");
    let ledger = s
        .usage_ledger
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let records: Vec<CloudUsageRecord> = ledger
        .records
        .iter()
        .filter(|r| r.tenant_id == tenant && within_window(r, &q.from, &q.to))
        .cloned()
        .collect();
    drop(ledger);
    Json(build_report(&tenant, &group_by, &records))
}

async fn usage_records_tenant(
    Path(tenant): Path<String>,
    State(s): State<AppState>,
    Query(q): Query<UsageQuery>,
) -> Json<serde_json::Value> {
    let limit = q.limit.unwrap_or(500).min(5000);
    let ledger = s
        .usage_ledger
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut records: Vec<CloudUsageRecord> = ledger
        .records
        .iter()
        .filter(|r| r.tenant_id == tenant && within_window(r, &q.from, &q.to))
        .cloned()
        .collect();
    drop(ledger);
    records.sort_by(|a, b| b.occurred_at.cmp(&a.occurred_at));
    records.truncate(limit);
    Json(serde_json::json!({
        "schema_version": "usage-records.v1",
        "tenant_id": tenant,
        "count": records.len(),
        "records": records,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::usage_record_from_envelope;

    #[allow(clippy::too_many_arguments)]
    fn envelope(
        event_id: &str,
        tenant: &str,
        device: &str,
        user: &str,
        agent: &str,
        model: &str,
        tokens: i64,
        cost: f64,
    ) -> serde_json::Value {
        serde_json::json!({
            "event_type": "ai_usage_event",
            "event_id": event_id,
            "tenant_id": tenant,
            "device_id": device,
            "payload": {
                "event_id": event_id,
                "tenant_id": tenant,
                "device_id": device,
                "actor_id_hash": user,
                "actor_kind": "human",
                "agent_id": agent,
                "agent_type": "coding_agent",
                "provider": "fixture",
                "model": model,
                "occurred_at": "2026-07-13T00:00:00Z",
                "tokens": {
                    "input_tokens": tokens,
                    "output_tokens": tokens / 4,
                    "total_tokens": tokens + tokens / 4
                },
                "cost": { "total_cost": cost, "currency": "USD" }
            }
        })
    }

    fn sample_records() -> Vec<CloudUsageRecord> {
        // tenant-a: device-1 (userA, agentX) x2, device-2 (userB, agentY) x1
        // tenant-b: device-3 (userC, agentZ) x1
        [
            envelope(
                "e1", "tenant-a", "device-1", "userA", "agentX", "gpt", 100, 0.10,
            ),
            envelope(
                "e2", "tenant-a", "device-1", "userA", "agentX", "gpt", 200, 0.20,
            ),
            envelope(
                "e3", "tenant-a", "device-2", "userB", "agentY", "claude", 400, 0.40,
            ),
            envelope(
                "e4", "tenant-b", "device-3", "userC", "agentZ", "gpt", 800, 0.80,
            ),
        ]
        .iter()
        .filter_map(usage_record_from_envelope)
        .collect()
    }

    #[test]
    fn extracts_all_dimensions_from_envelope() {
        let recs = sample_records();
        assert_eq!(recs.len(), 4);
        let e1 = &recs[0];
        assert_eq!(e1.tenant_id, "tenant-a");
        assert_eq!(e1.device_id, "device-1");
        assert_eq!(e1.user_id, "userA");
        assert_eq!(e1.agent_id, "agentX");
        assert_eq!(e1.model, "gpt");
        assert_eq!(e1.input_tokens, 100);
        assert_eq!(e1.total_tokens, 125);
        assert!((e1.total_cost - 0.10).abs() < 1e-9);
    }

    #[test]
    fn groups_tenant_by_device() {
        let recs: Vec<_> = sample_records()
            .into_iter()
            .filter(|r| r.tenant_id == "tenant-a")
            .collect();
        let report = build_report("tenant-a", "device", &recs);
        assert_eq!(report["totals"]["request_count"], 3);
        assert!((report["totals"]["total_cost"].as_f64().unwrap() - 0.70).abs() < 1e-9); //
        let groups = report["groups"].as_array().unwrap(); //
        assert_eq!(groups.len(), 2);
        // device-2 has the highest cost (0.40) so it sorts first.
        assert_eq!(groups[0]["key"], "device-2");
        assert_eq!(groups[0]["request_count"], 1);
        assert_eq!(groups[1]["key"], "device-1");
        assert_eq!(groups[1]["request_count"], 2);
        assert!((groups[1]["total_cost"].as_f64().unwrap() - 0.30).abs() < 1e-9);
        //
    }

    #[test]
    fn groups_tenant_by_user_and_agent() {
        let recs: Vec<_> = sample_records()
            .into_iter()
            .filter(|r| r.tenant_id == "tenant-a")
            .collect();
        let by_user = build_report("tenant-a", "user", &recs);
        let users = by_user["groups"].as_array().unwrap(); //
        assert_eq!(users.len(), 2);
        assert!(users
            .iter()
            .any(|g| g["key"] == "userA" && g["request_count"] == 2));

        let by_agent = build_report("tenant-a", "agent", &recs);
        assert_eq!(by_agent["groups"].as_array().unwrap().len(), 2); //
    }

    #[test]
    fn cross_tenant_groups_by_tenant_with_distinct_counts() {
        let recs = sample_records();
        let report = build_report("*", "tenant", &recs);
        assert_eq!(report["totals"]["request_count"], 4);
        assert!((report["totals"]["total_cost"].as_f64().unwrap() - 1.50).abs() < 1e-9); //
        let groups = report["groups"].as_array().unwrap(); //
        assert_eq!(groups.len(), 2);
        // tenant-a: 2 devices, 2 users, 2 agents
        let a = groups.iter().find(|g| g["key"] == "tenant-a").unwrap(); //
        assert_eq!(a["devices"], 2);
        assert_eq!(a["users"], 2);
        assert_eq!(a["agents"], 2);
        assert_eq!(a["request_count"], 3);
    }

    #[test]
    fn ledger_dedups_by_event_id() {
        let mut ledger = crate::state::UsageLedger::default();
        let env = envelope("dup", "t", "d", "u", "a", "gpt", 10, 0.01);
        let rec = usage_record_from_envelope(&env).unwrap(); //
        assert!(ledger.record(rec.clone()));
        assert!(!ledger.record(rec)); // duplicate rejected
        assert_eq!(ledger.records.len(), 1);
    }

    #[test]
    fn ignores_non_usage_envelopes() {
        let other = serde_json::json!({"event_type": "decision_log", "event_id": "x"});
        assert!(usage_record_from_envelope(&other).is_none());
    }

    // End-to-end: a batch POSTed to the real /v1/telemetry/batches route (the
    // exact call the Local Control Plane cloud-sync loop makes) must land in
    // the usage ledger and surface in the per-tenant usage report.
    #[tokio::test]
    async fn telemetry_batch_feeds_usage_report() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::util::ServiceExt;

        let state = crate::state::test_app_state();
        let telemetry_app = crate::telemetry::router().with_state(state.clone());
        let usage_app = super::router().with_state(state.clone());

        let batch = serde_json::json!({
            "schema_version": "telemetry-batch.v1",
            "tenant_id": "tenant-a",
            "device_id": "device-1",
            "batch_id": "batch-1",
            "events": [
                envelope("e1", "tenant-a", "device-1", "userA", "agentX", "gpt", 100, 0.10),
                envelope("e2", "tenant-a", "device-2", "userB", "agentY", "claude", 400, 0.40),
                // Duplicate of e1 (at-least-once redelivery) must not double-count.
                envelope("e1", "tenant-a", "device-1", "userA", "agentX", "gpt", 100, 0.10),
            ]
        });

        let req = Request::builder()
            .method("POST")
            .uri("/v1/telemetry/batches")
            .header("content-type", "application/json")
            .body(Body::from(batch.to_string()))
            .unwrap(); //
        let res = telemetry_app.oneshot(req).await.unwrap(); //
        assert_eq!(res.status(), StatusCode::OK);

        let req = Request::builder()
            .uri("/v1/tenants/tenant-a/usage/summary?group_by=device")
            .body(Body::empty())
            .unwrap(); //
        let res = usage_app.oneshot(req).await.unwrap(); //
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap(); //
        let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap(); //

        // Two unique events (e1 deduped), total cost 0.50 across 2 devices.
        assert_eq!(report["totals"]["request_count"], 2);
        assert!((report["totals"]["total_cost"].as_f64().unwrap() - 0.50).abs() < 1e-9); //
        assert_eq!(report["group_by"], "device");
        assert_eq!(report["groups"].as_array().unwrap().len(), 2); //
    }
}
