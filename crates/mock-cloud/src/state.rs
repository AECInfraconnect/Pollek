// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, serde::Serialize)]
pub struct DeviceStatus {
    pub id: String,
    pub tenant_id: String,
    pub profile: String,
    pub revoked: bool,
    pub last_health: String,
    pub capabilities: dek_domain_schema::EnforcementCapabilities,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct AuditLog {
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub details: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ApprovalRequest {
    pub ref_id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub principal: String,
    pub action: String,
    pub resource: String,
    pub status: String,
    pub timestamp: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PolicyBundle {
    pub version: String,
    pub cedar_src: String,
    pub openfga_store: String,
}

#[derive(Clone, Debug)]
pub struct RolloutConfig {
    pub latest_bundle: PolicyBundle,
    pub canary_bundle: Option<PolicyBundle>,
    pub canary_percentage: u8, // 0-100
}

#[derive(Clone, Debug)]
pub struct ChaosConfig {
    pub outage_enabled: bool,
    pub global_latency_ms: i64,
}

#[derive(Clone)]
pub struct AppState {
    pub revision: Arc<AtomicUsize>,
    pub rsa_public_key_pem: String,
    /// device_code -> poll count
    pub pending: Arc<Mutex<HashMap<String, u32>>>,
    /// device_id -> DeviceStatus
    pub devices: Arc<Mutex<HashMap<String, DeviceStatus>>>,
    /// telemetry events buffer
    pub telemetry_events: Arc<Mutex<VecDeque<serde_json::Value>>>,
    /// rollout config
    pub rollout: Arc<Mutex<RolloutConfig>>,
    /// admin audit logs
    pub audit_logs: Arc<Mutex<Vec<AuditLog>>>,
    pub revocation_list: Arc<Mutex<Vec<String>>>,
    pub active_seed: Arc<Mutex<Vec<u8>>>,
    pub network_rules: Arc<Mutex<Vec<serde_json::Value>>>,
    /// Pending policy publications for maker-checker
    pub pending_policies: Arc<Mutex<HashMap<String, PolicyBundle>>>,
    pub trusted_keys: Arc<Mutex<Vec<serde_json::Value>>>,
    /// chaos testing settings
    pub chaos_config: Arc<Mutex<ChaosConfig>>,
    // Registry state for Phase 1
    pub registry: Arc<Mutex<RegistryState>>,
    /// Human-in-the-loop approvals (ref_id -> ApprovalRequest)
    pub approvals: Arc<Mutex<HashMap<String, ApprovalRequest>>>,
    /// AI cost/token usage records synced up from Local Control Planes,
    /// aggregated for per-device / per-user / per-agent / per-tenant reports.
    pub usage_ledger: Arc<Mutex<UsageLedger>>,
}

/// A single AI usage record (one model call) as reported by a Local Control
/// Plane, flattened to the dimensions Cloud reports on. Only privacy-preserving
/// identifiers are kept: the user is a pre-hashed `actor_id_hash`.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct CloudUsageRecord {
    pub event_id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub user_id: String,
    pub user_kind: String,
    pub agent_id: String,
    pub agent_type: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub currency: String,
    pub occurred_at: String,
}

/// Append-only ledger of usage records with dedup by `event_id`, so a Local
/// Control Plane re-pushing a batch (at-least-once delivery) does not
/// double-count cost or tokens.
#[derive(Default)]
pub struct UsageLedger {
    pub records: Vec<CloudUsageRecord>,
    seen_event_ids: std::collections::HashSet<String>,
}

impl UsageLedger {
    /// Records a usage event; returns false if this `event_id` was already
    /// ingested (duplicate ignored).
    pub fn record(&mut self, record: CloudUsageRecord) -> bool {
        if !self.seen_event_ids.insert(record.event_id.clone()) {
            return false;
        }
        self.records.push(record);
        // Bound memory the same way the telemetry buffer is bounded.
        if self.records.len() > 50_000 {
            let overflow = self.records.remove(0);
            self.seen_event_ids.remove(&overflow.event_id);
        }
        true
    }
}

#[derive(Clone, Default)]
pub struct RegistryState {
    pub tenants: HashMap<String, dek_domain_schema::Tenant>,
    pub principals: HashMap<String, dek_domain_schema::Principal>,
    pub devices: HashMap<String, dek_domain_schema::DekDevice>,
    pub agents: HashMap<String, dek_domain_schema::AiAgent>,
    pub mcp_servers: HashMap<String, dek_domain_schema::McpServer>,
    pub tools: HashMap<String, dek_domain_schema::Tool>,
    pub resources: HashMap<String, dek_domain_schema::Resource>,
    pub relationships: Vec<dek_domain_schema::Relationship>,
    pub policies: HashMap<String, dek_domain_schema::Policy>,
    pub pep_deployments: HashMap<String, dek_domain_schema::PepDeployment>,
    /// Latest raw registry-sync snapshot from a Local Control Plane, keyed by
    /// item type (e.g. `agent`, `discovery_entity`, `discovered_capability`).
    /// Local pushes a full snapshot each cycle, so each sync replaces the
    /// previous list for the types it carries.
    pub synced_objects: HashMap<String, Vec<serde_json::Value>>,
}

/// Minimal fully-defaulted [`AppState`] for in-process router tests.
#[cfg(test)]
pub(crate) fn test_app_state() -> AppState {
    AppState {
        revision: Arc::new(AtomicUsize::new(1)),
        rsa_public_key_pem: String::new(),
        pending: Arc::new(Mutex::new(HashMap::new())),
        devices: Arc::new(Mutex::new(HashMap::new())),
        telemetry_events: Arc::new(Mutex::new(VecDeque::new())),
        rollout: Arc::new(Mutex::new(RolloutConfig {
            latest_bundle: PolicyBundle {
                version: "1.0".to_string(),
                cedar_src: String::new(),
                openfga_store: String::new(),
            },
            canary_bundle: None,
            canary_percentage: 0,
        })),
        audit_logs: Arc::new(Mutex::new(Vec::new())),
        revocation_list: Arc::new(Mutex::new(Vec::new())),
        active_seed: Arc::new(Mutex::new(vec![0u8; 32])),
        network_rules: Arc::new(Mutex::new(Vec::new())),
        pending_policies: Arc::new(Mutex::new(HashMap::new())),
        trusted_keys: Arc::new(Mutex::new(Vec::new())),
        chaos_config: Arc::new(Mutex::new(ChaosConfig {
            outage_enabled: false,
            global_latency_ms: 0,
        })),
        registry: Arc::new(Mutex::new(RegistryState::default())),
        approvals: Arc::new(Mutex::new(HashMap::new())),
        usage_ledger: Arc::new(Mutex::new(UsageLedger::default())),
    }
}

pub fn rand_hex(n_bytes: usize) -> String {
    use rand_core::RngCore;
    let mut b = vec![0u8; n_bytes];
    rand_core::OsRng.fill_bytes(&mut b);
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

/// Extracts a [`CloudUsageRecord`] from a telemetry envelope whose
/// `event_type` is `ai_usage_event`. The envelope carries `tenant_id`,
/// `device_id`, and `event_id` at the top level and the canonical
/// `AiUsageEventV1` under `payload`. Returns `None` if the envelope is not a
/// usage event or is missing the fields needed to attribute cost.
pub fn usage_record_from_envelope(env: &serde_json::Value) -> Option<CloudUsageRecord> {
    if env.get("event_type").and_then(|v| v.as_str()) != Some("ai_usage_event") {
        return None;
    }
    // The usage event itself lives under `payload`; some callers may send the
    // event unwrapped, so fall back to the envelope root.
    let payload = env.get("payload").unwrap_or(env);

    let str_at = |v: &serde_json::Value, key: &str| {
        v.get(key)
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    };
    let unknown = || "unknown".to_string();

    let event_id = str_at(env, "event_id")
        .or_else(|| str_at(payload, "event_id"))
        .unwrap_or_else(|| format!("usage-{}", rand_hex(8)));
    let tenant_id = str_at(env, "tenant_id")
        .or_else(|| str_at(payload, "tenant_id"))
        .unwrap_or_else(unknown);
    let device_id = str_at(env, "device_id")
        .or_else(|| str_at(payload, "device_id"))
        .unwrap_or_else(unknown);

    let tokens = payload.get("tokens");
    let cost = payload.get("cost");
    let i64_at = |v: Option<&serde_json::Value>, key: &str| {
        v.and_then(|t| t.get(key))
            .and_then(|x| x.as_i64())
            .unwrap_or(0)
    };

    Some(CloudUsageRecord {
        event_id,
        tenant_id,
        device_id,
        user_id: str_at(payload, "actor_id_hash").unwrap_or_else(unknown),
        user_kind: str_at(payload, "actor_kind").unwrap_or_else(|| "unknown".to_string()),
        agent_id: str_at(payload, "agent_id").unwrap_or_else(unknown),
        agent_type: str_at(payload, "agent_type").unwrap_or_else(unknown),
        provider: str_at(payload, "provider").unwrap_or_else(unknown),
        model: str_at(payload, "model").unwrap_or_else(unknown),
        input_tokens: i64_at(tokens, "input_tokens"),
        output_tokens: i64_at(tokens, "output_tokens"),
        total_tokens: i64_at(tokens, "total_tokens"),
        total_cost: cost
            .and_then(|c| c.get("total_cost"))
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0),
        currency: str_at(cost.unwrap_or(&serde_json::Value::Null), "currency")
            .unwrap_or_else(|| "USD".to_string()),
        occurred_at: str_at(payload, "occurred_at")
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
    })
}

impl AppState {
    /// Records an `ai_usage_event` telemetry envelope into the usage ledger.
    /// No-op for non-usage envelopes or duplicates.
    pub fn record_usage_envelope(&self, env: &serde_json::Value) -> bool {
        match usage_record_from_envelope(env) {
            Some(record) => {
                let mut ledger = self
                    .usage_ledger
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                ledger.record(record)
            }
            None => false,
        }
    }

    pub fn audit_push(&self, actor: &str, action: &str, details: &str) {
        let mut logs = self.audit_logs.lock().unwrap(); //
        logs.push(AuditLog {
            timestamp: chrono::Utc::now().to_rfc3339(),
            actor: actor.to_string(),
            action: action.to_string(),
            details: details.to_string(),
        });
    }
}
