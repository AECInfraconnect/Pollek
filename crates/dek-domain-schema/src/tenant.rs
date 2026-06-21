use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TrustDomainStrategy {
    Shared,
    Dedicated,
    Federated,
    CustomerManaged,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Tenant {
    pub schema_version: String,
    pub tenant_id: String,
    pub tenant_type: String,
    pub display_name: String,
    pub data_region: String,
    pub trust_domain_strategy: TrustDomainStrategy,
    pub trust_domain: String,
    pub policy_mode: String,
    pub default_fail_mode: String,
    pub created_at: String,
}
