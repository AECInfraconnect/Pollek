use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PepDeployment {
    pub schema_version: String,
    pub pep_deployment_id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub pep_type: String,
    pub mode: String,
    pub fail_mode: String,
    pub capabilities: Vec<String>,
    pub status: String,
}
