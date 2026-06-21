use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateSource {
    Bundle,
    OutOfBand,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EbpfMapUpdate {
    pub schema_version: String,
    pub map_name: String,
    pub operation: String, // e.g. "insert", "delete"
    pub source: UpdateSource,
    pub tenant_id: String,
    pub device_id: String,
    pub generation: u64,
    pub key: Value,
    pub value: Value,
    pub signature: Option<String>,
}
