use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DekDevice {
    pub schema_version: String,
    pub device_id: String,
    pub tenant_id: String,
    pub user_id: String,
    pub hostname: String,
    pub os: String,
    pub dek_version: String,
    pub spiffe_id: String,
    pub pep_capabilities: Vec<String>,
    pub enforcement_ceiling: String,
    pub status: String,
    pub last_seen_at: String,
}
