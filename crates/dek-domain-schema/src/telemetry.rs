use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DecisionLog {
    pub principal: String,
    pub tool: String,
    pub method: String,
    pub engine: String,
    pub verdict: String,
    pub reason: String,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SecurityAlert {
    pub alert_id: String,
    pub severity: String,
    pub description: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Heartbeat {
    pub status: String,
    pub telemetry_queue_depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", content = "data")]
pub enum TelemetryPayload {
    DecisionLog(DecisionLog),
    SecurityAlert(SecurityAlert),
    Heartbeat(Heartbeat),
    Custom(serde_json::Value),
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TelemetryEnvelope {
    pub schema_version: String,
    pub event_id: String,
    pub event_type: String,
    pub timestamp: String,
    pub tenant_id: String,
    pub device_id: String,
    pub payload: TelemetryPayload,
}
