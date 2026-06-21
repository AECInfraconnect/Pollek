use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TraceSpan {
    pub span_id: String,
    pub name: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event_type")]
pub enum TelemetryEvent {
    #[serde(rename = "decision")]
    Decision {
        schema_version: String,
        event_id: String,
        trace_id: String,
        span_id: String,
        tenant_id: String,
        device_id: String,
        spiffe_id: String,
        pep_type: String,
        agent_id: String,
        principal_id: String,
        mcp_server_id: String,
        tool_id: String,
        tool_name: String,
        action: String,
        resource_id: String,
        resource_uri: String,
        decision: String,
        reason: String,
        policy_ids: Vec<String>,
        bundle_id: String,
        bundle_version: String,
        latency_ms: u64,
        cached: bool,
        timestamp: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        compliance_tags: Option<Vec<String>>,
    },
    #[serde(rename = "trace")]
    Trace {
        schema_version: String,
        trace_id: String,
        tenant_id: String,
        device_id: String,
        spans: Vec<TraceSpan>,
    },
    #[serde(rename = "security")]
    Security {
        schema_version: String,
        severity: String,
        category: String,
        tenant_id: String,
        device_id: String,
        details: HashMap<String, String>,
        timestamp: String,
    },
    #[serde(rename = "metric")]
    Metric {
        schema_version: String,
        tenant_id: String,
        device_id: String,
        metrics: HashMap<String, f64>,
        timestamp: String,
    },
    #[serde(rename = "ebpf_guardrail")]
    EbpfGuardrail {
        schema_version: String,
        tenant_id: String,
        device_id: String,
        pid: u32,
        process_name: String,
        dest_ip: String,
        dest_port: u16,
        fqdn: String,
        verdict: String,
        map_name: String,
        rule_id: String,
        timestamp: String,
    },
    #[serde(rename = "os_guardrail")]
    OsGuardrail {
        schema_version: String,
        tenant_id: String,
        device_id: String,
        os_platform: String, // "windows" or "macos"
        pid: Option<u32>,
        process_name: Option<String>,
        dest_ip: Option<String>,
        dest_port: Option<u16>,
        fqdn: Option<String>,
        protocol: Option<String>,
        verdict: String, // "allow" or "deny"
        rule_id: Option<String>,
        timestamp: String,
    },
    #[serde(rename = "os_lifecycle")]
    OsLifecycle {
        schema_version: String,
        tenant_id: String,
        device_id: String,
        os_platform: String, // "windows" or "macos"
        component: String,   // "wfp_filter", "nefilter", "wfp_callout"
        event: String,       // "started", "stopped", "install.completed", "install.failed", etc.
        details: Option<String>,
        timestamp: String,
    },
}
