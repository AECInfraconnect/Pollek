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
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LogEntry {
    pub device_id: String,
    pub timestamp: String,
    pub action: String,
    pub decision: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct AuditLog {
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub details: String,
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

#[derive(Clone)]
pub struct AppState {
    pub revision: Arc<AtomicUsize>,
    pub rsa_public_key_pem: String,
    /// device_code -> poll count
    pub pending: Arc<Mutex<HashMap<String, u32>>>,
    /// device_id -> DeviceStatus
    pub devices: Arc<Mutex<HashMap<String, DeviceStatus>>>,
    /// decision logs buffer
    pub decision_logs: Arc<Mutex<VecDeque<LogEntry>>>,
    /// rollout config
    pub rollout: Arc<Mutex<RolloutConfig>>,
    /// admin audit logs
    pub audit_logs: Arc<Mutex<Vec<AuditLog>>>,
    /// Pending policy publications for maker-checker
    pub pending_policies: Arc<Mutex<HashMap<String, PolicyBundle>>>,
    // New registries
    pub tenants: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    pub agents: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    pub entities: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    pub resources: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    pub relationships: Arc<Mutex<Vec<serde_json::Value>>>,
    pub trusted_keys: Arc<Mutex<Vec<serde_json::Value>>>,
}

pub fn rand_hex(n_bytes: usize) -> String {
    use rand_core::RngCore;
    let mut b = vec![0u8; n_bytes];
    rand_core::OsRng.fill_bytes(&mut b);
    b.iter().map(|x| format!("{:02x}", x)).collect()
}
