use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TelemetryEventEnvelope {
    pub schema_version: String,
    pub event_id: String,
    pub event_type: TelemetryEventType,
    pub timestamp: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub environment_id: String,
    pub device_id: String,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub payload: serde_json::Value,
    pub redaction_applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEventType {
    DecisionLog,
    PolicyBundleActivated,
    PolicyBundleRejected,
    RuntimeMetric,
    SecurityEvent,
    PiiRedactionEvent,
    AdapterHealth,
    SyncHealth,
    OsGuardrailEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DecisionResult {
    pub request_id: String,
    pub trace_id: String,
    pub decision: DecisionEffect,
    pub reason: String,
    pub matched_policy_ids: Vec<String>,
    pub matched_route_id: Option<String>,
    pub adapter_results: Vec<AdapterDecisionResult>,
    pub obligations: Vec<DecisionObligation>,
    pub latency_ms: u64,
    pub selected_engine: Option<String>,
    pub enforcement_plane: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DecisionEffect {
    Allow,
    Deny,
    Redact,
    Mask,
    Warn,
    RequireApproval,
    BreakGlassAllow,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AdapterDecisionResult {
    pub adapter_id: String,
    pub decision: DecisionEffect,
    pub reason: Option<String>,
    pub matched_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DecisionObligation {
    pub obligation_type: String,
    pub fields: Vec<String>,
    pub parameters: std::collections::HashMap<String, String>,
}
