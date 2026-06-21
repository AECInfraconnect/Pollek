pub use pollen_contract::PollenDecisionResultV1 as DecisionResult;
pub use pollen_contract::PollenDecisionResultV1Decision as DecisionEffect;
pub use pollen_contract::PollenTelemetryEnvelopeV1 as TelemetryEventEnvelope;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
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
