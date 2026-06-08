use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExternalIds {
    pub oidc_sub: Option<String>,
    pub email: Option<String>,
    pub spiffe_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IdentityAssurance {
    pub aal: u32,
    pub ial: u32,
    pub auth_time: String,
    pub mfa: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Entity {
    pub schema_version: String,
    pub tenant_id: String,
    pub entity_id: String,
    pub entity_type: String,
    pub external_ids: ExternalIds,
    pub attributes: HashMap<String, String>,
    pub identity_assurance: IdentityAssurance,
    pub status: String,
}
