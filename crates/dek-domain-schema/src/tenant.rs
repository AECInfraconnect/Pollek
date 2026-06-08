use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Tenant {
    pub schema_version: String,
    pub tenant_id: String,
    pub tenant_type: String,
    pub display_name: String,
    pub trust_domain_strategy: String,
    pub data_region: String,
    pub policy_mode: String,
    pub created_at: String,
}
