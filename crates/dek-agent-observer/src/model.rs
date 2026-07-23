use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentObservationEvent {
    pub event_id: String,
    pub tenant_id: String,
    pub trace_id: String,
    pub agent_id: Option<String>,
    pub shadow_candidate_id: Option<String>,
    pub tool_id: Option<String>,
    pub resource_id: Option<String>,
    pub surface: String,
    pub action: String,
    pub pep_type: Option<String>,
    pub risk_level: Option<String>,
    pub timestamp: String,
    pub payload_json: String,
    pub token_usage: Option<TokenUsage>,
    #[serde(default)]
    pub browser_scope: Option<BrowserAiObservationScope>,

    // Unified Event fields
    #[serde(default)]
    pub event_kind: EventKind,
    #[serde(default)]
    pub decision: Option<DecisionInfo>,
    #[serde(default)]
    pub tool_call: Option<ToolCall>,
    #[serde(default)]
    pub resource_access: Option<ResourceAccess>,
    #[serde(default)]
    pub latency_ms: Option<i64>,
    #[serde(default)]
    pub provider: Option<String>,

    /// Raw runtime signal from a kernel/ETW/EndpointSecurity sensor, carried so
    /// the [`crate::agent_correlator`] can attribute an agent-less event to a
    /// discovered agent before it reaches the telemetry spool.
    #[serde(default)]
    pub process_signal: Option<ProcessSignal>,
}

/// Runtime process/flow signal captured by a low-level sensor (eBPF ring
/// buffer, Windows ETW, macOS EndpointSecurity) *before* agent attribution.
/// All fields are optional because different sensors expose different keys.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ProcessSignal {
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    /// sha256 of the (normalized) executable path — the stable identity key.
    pub exe_path_hash: Option<String>,
    /// cgroup v2 id (Linux) or equivalent scope id.
    pub cgroup_id: Option<u64>,
    /// Network 5-tuple peer, when the signal came from a flow event.
    pub remote_addr: Option<String>,
    pub remote_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrowserAiObservationScope {
    pub base_name: Option<String>,
    pub display_name: Option<String>,
    pub browser_id: Option<String>,
    pub browser_name: Option<String>,
    pub candidate_id: Option<String>,
    pub discovery_candidate_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    #[default]
    Generic,
    LlmCall,
    ToolCall,
    ResourceAccess,
    Decision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool_name: String,
    pub server: Option<String>,
    pub args_summary: Option<String>,
    pub result_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAccess {
    pub resource_type: String,
    pub target_redacted: String,
    pub bytes: Option<i64>,
    pub verb: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionInfo {
    pub allow: bool,
    pub reason_code: String,
    pub obligations: Vec<String>,
    pub matched_policy_ids: Vec<String>,
    pub compliance_tags: Vec<String>,

    #[serde(default)]
    pub pep_plane: Option<String>,
    #[serde(default)]
    pub enforced_for_real: Option<bool>,
    #[serde(default)]
    pub status_badge: Option<String>,
    #[serde(default)]
    pub message_th: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub total_tokens: Option<i64>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostLedgerEntry {
    pub event_id: String,
    pub agent_id: String,
    pub provider: String,
    pub model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
    pub currency: String,
    pub estimated: bool,
    pub timestamp: String,
}
