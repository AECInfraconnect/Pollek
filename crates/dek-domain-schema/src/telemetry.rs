use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TelemetryEnvelope {
    pub schema_version: String,
    pub event_id: String,
    pub event_type: String,
    pub timestamp: String,
    pub tenant_id: String,
    pub device_id: String,
    pub payload: Value,
}
