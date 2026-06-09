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

use dek_domain_schema::TelemetryEvent;

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
    pub global_latency_ms: u64,
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
    pub telemetry_events: Arc<Mutex<VecDeque<TelemetryEvent>>>,
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
}

pub fn rand_hex(n_bytes: usize) -> String {
    use rand_core::RngCore;
    let mut b = vec![0u8; n_bytes];
    rand_core::OsRng.fill_bytes(&mut b);
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

impl AppState {
    pub fn audit_push(&self, actor: &str, action: &str, details: &str) {
        let mut logs = self.audit_logs.lock().unwrap();
        logs.push(AuditLog {
            timestamp: chrono::Utc::now().to_rfc3339(),
            actor: actor.to_string(),
            action: action.to_string(),
            details: details.to_string(),
        });
    }
}
