use serde::{Deserialize, Serialize};
use crate::capability::CapabilityDescriptor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySpec {
    pub agent_signature_id: String,
    pub layers: TelemetryLayers,
    pub otel_attributes: Vec<String>,
    pub redaction: RedactionSpec,
    pub export: ExportSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryLayers {
    pub transport: bool,
    pub tool_exec: bool,
    pub agentic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionSpec {
    pub redact_keys: Vec<String>,
    pub redact_tool_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSpec {
    pub events_endpoint: String,
    pub metrics_endpoint: String,
    pub otlp_endpoint: Option<String>,
    pub batch_via_spool: bool,
}

pub fn derive_telemetry(cap: &CapabilityDescriptor) -> TelemetrySpec {
    let redact_keys: Vec<String> = cap.data_reach.iter()
        .filter(|d| d.sensitivity == "high" || d.sensitivity == "critical")
        .filter_map(|d| d.path_pattern.clone())
        .collect();

    TelemetrySpec {
        agent_signature_id: cap.agent_signature_id.clone(),
        layers: TelemetryLayers { transport: true, tool_exec: true, agentic: true },
        otel_attributes: vec![
            "gen_ai.system".into(),
            "gen_ai.request.model".into(),
            "gen_ai.usage.input_tokens".into(),
            "gen_ai.usage.output_tokens".into(),
            "mcp.tool.name".into(),
            "mcp.server.name".into(),
        ],
        redaction: RedactionSpec {
            redact_keys,
            redact_tool_args: cap.tool_capabilities.iter()
                .filter(|t| t.risk_class == "write" || t.risk_class == "exec")
                .map(|t| t.tool_name.clone())
                .collect(),
        },
        export: ExportSpec {
            events_endpoint: "/v1/telemetry/events".into(),
            metrics_endpoint: "/v1/telemetry/metrics".into(),
            otlp_endpoint: Some("http://127.0.0.1:4318".into()),
            batch_via_spool: true,
        },
    }
}
