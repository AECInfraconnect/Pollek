use crate::model::{AgentObservationEvent, CostLedgerEntry, EventKind};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AiUsageEventKind {
    AgentRunStarted,
    AgentRunCompleted,
    AgentStepStarted,
    AgentStepCompleted,
    ModelCallStarted,
    ModelCallChunk,
    #[default]
    ModelCallCompleted,
    ToolCallStarted,
    ToolCallCompleted,
    ResourceAccess,
    BudgetPreflight,
    BudgetAlert,
    BudgetBlocked,
    UsageReconciled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    LocalAgent,
    BrowserAi,
    CodingAgent,
    ClaudeCode,
    CodexCli,
    Cursor,
    McpClient,
    McpServerAgent,
    A2aAgent,
    ManagedAgent,
    GatewayAgent,
    ShadowAi,
    // Tolerate unrecognized agent-type labels (e.g. a new/local runtime the
    // caller names itself) by mapping them to `Unknown` rather than rejecting
    // the whole usage event and losing its exact token/cost data.
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum UsageSource {
    ProviderResponse,
    ProviderStreamingFinal,
    ProviderBillingExport,
    SdkInstrumentation,
    ProxyObservation,
    BrowserEstimate,
    LocalTokenizerEstimate,
    ManualImport,
    Reconciled,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CostSource {
    ProviderReported,
    PriceCatalogExact,
    PriceCatalogTiered,
    EstimatedTokenizer,
    BillingReconciled,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalTokenUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub cached_input_tokens: i64,
    pub cache_write_input_tokens: i64,
    pub cache_write_input_tokens_5m: Option<i64>,
    pub cache_write_input_tokens_1h: Option<i64>,
    pub reasoning_output_tokens: i64,
    pub reasoning_input_tokens: Option<i64>,
    pub tool_prompt_tokens: i64,
    pub tool_result_tokens: i64,
    pub image_input_tokens: i64,
    pub image_output_tokens: i64,
    pub audio_input_tokens: i64,
    pub audio_output_tokens: i64,
    pub video_input_tokens: i64,
    #[serde(default)]
    pub by_modality: BTreeMap<String, i64>,
    #[serde(default)]
    pub usage_details_ext: BTreeMap<String, i64>,
    pub estimated: bool,
    pub source: UsageSource,
}

impl Default for CanonicalTokenUsage {
    fn default() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            cached_input_tokens: 0,
            cache_write_input_tokens: 0,
            cache_write_input_tokens_5m: None,
            cache_write_input_tokens_1h: None,
            reasoning_output_tokens: 0,
            reasoning_input_tokens: None,
            tool_prompt_tokens: 0,
            tool_result_tokens: 0,
            image_input_tokens: 0,
            image_output_tokens: 0,
            audio_input_tokens: 0,
            audio_output_tokens: 0,
            video_input_tokens: 0,
            by_modality: BTreeMap::new(),
            usage_details_ext: BTreeMap::new(),
            estimated: false,
            source: UsageSource::Unknown,
        }
    }
}

impl CanonicalTokenUsage {
    pub fn computed_total_tokens(&self) -> i64 {
        let mapped_total = self.input_tokens
            + self.output_tokens
            + self.cached_input_tokens
            + self.cache_write_input_tokens
            + self.reasoning_input_tokens.unwrap_or(0)
            + self.tool_prompt_tokens
            + self.tool_result_tokens
            + self.image_input_tokens
            + self.image_output_tokens
            + self.audio_input_tokens
            + self.audio_output_tokens
            + self.video_input_tokens;
        mapped_total + self.usage_details_ext.values().copied().sum::<i64>()
    }

    pub fn with_provider_total(mut self, provider_total: Option<i64>) -> Self {
        self.total_tokens = provider_total.unwrap_or_else(|| self.computed_total_tokens());
        self
    }

    pub fn token_class_counts(&self) -> BTreeMap<&'static str, i64> {
        let mut counts = BTreeMap::new();
        counts.insert("input_tokens", self.input_tokens);
        counts.insert("output_tokens", self.output_tokens);
        counts.insert("cached_input_tokens", self.cached_input_tokens);
        counts.insert("cache_write_input_tokens", self.cache_write_input_tokens);
        counts.insert("reasoning_output_tokens", self.reasoning_output_tokens);
        counts.insert("tool_prompt_tokens", self.tool_prompt_tokens);
        counts.insert("tool_result_tokens", self.tool_result_tokens);
        counts.insert("image_input_tokens", self.image_input_tokens);
        counts.insert("image_output_tokens", self.image_output_tokens);
        counts.insert("audio_input_tokens", self.audio_input_tokens);
        counts.insert("audio_output_tokens", self.audio_output_tokens);
        counts.insert("video_input_tokens", self.video_input_tokens);
        counts
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalCostBreakdown {
    pub currency: String,
    pub input_cost: f64,
    pub output_cost: f64,
    pub cached_input_cost: f64,
    pub cache_write_input_cost: f64,
    pub reasoning_output_cost: f64,
    pub tool_cost: f64,
    pub image_cost: f64,
    pub audio_cost: f64,
    pub total_cost: f64,
    pub price_catalog_version: Option<String>,
    pub pricing_tier_id: Option<String>,
    pub cost_source: CostSource,
    pub estimated: bool,
    #[serde(default)]
    pub cost_details_ext: BTreeMap<String, f64>,
}

impl Default for CanonicalCostBreakdown {
    fn default() -> Self {
        Self {
            currency: "USD".to_string(),
            input_cost: 0.0,
            output_cost: 0.0,
            cached_input_cost: 0.0,
            cache_write_input_cost: 0.0,
            reasoning_output_cost: 0.0,
            tool_cost: 0.0,
            image_cost: 0.0,
            audio_cost: 0.0,
            total_cost: 0.0,
            price_catalog_version: None,
            pricing_tier_id: None,
            cost_source: CostSource::Unknown,
            estimated: true,
            cost_details_ext: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiUsageEventV1 {
    pub schema_version: String,
    pub event_id: String,
    pub event_kind: AiUsageEventKind,
    pub occurred_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub tenant_id: String,
    pub workspace_id: Option<String>,
    pub device_id: Option<String>,
    pub actor_id_hash: Option<String>,
    pub actor_kind: Option<String>,
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    pub agent_run_id: Option<String>,
    pub agent_step_id: Option<String>,
    pub invocation_id: Option<String>,
    pub agent_id: Option<String>,
    pub agent_instance_id: Option<String>,
    pub agent_type: AgentType,
    pub parent_agent_id: Option<String>,
    pub subagent_id: Option<String>,
    pub shadow_candidate_id: Option<String>,
    pub provider: Option<String>,
    pub provider_api: Option<String>,
    pub provider_request_id: Option<String>,
    pub model: Option<String>,
    pub model_version: Option<String>,
    pub service_tier: Option<String>,
    pub inference_region: Option<String>,
    pub surface: String,
    pub pep_type: Option<String>,
    pub control_mode: Option<String>,
    pub policy_ids: Vec<String>,
    pub tokens: CanonicalTokenUsage,
    pub cost: CanonicalCostBreakdown,
    pub tool_id: Option<String>,
    pub tool_name: Option<String>,
    pub mcp_server_id: Option<String>,
    pub resource_id: Option<String>,
    pub resource_type: Option<String>,
    pub latency_ms: Option<i64>,
    pub status: String,
    pub error_code: Option<String>,
    #[serde(default)]
    pub provider_usage_raw: Value,
    #[serde(default)]
    pub metadata: Value,
    pub local_sequence: Option<i64>,
    pub cloud_sync_status: Option<String>,
    pub idempotency_key: String,
}

impl AiUsageEventV1 {
    pub const SCHEMA_VERSION: &'static str = "ai-usage-event.v1";

    pub fn finalize(mut self) -> Self {
        if self.tokens.total_tokens == 0 {
            self.tokens.total_tokens = self.tokens.computed_total_tokens();
        }
        if self.idempotency_key.is_empty() {
            self.idempotency_key = self.compute_idempotency_key();
        }
        self
    }

    pub fn compute_idempotency_key(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.tenant_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.device_id.as_deref().unwrap_or_default().as_bytes());
        hasher.update(b"|");
        hasher.update(self.event_id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.occurred_at.to_rfc3339().as_bytes());
        hasher.update(b"|");
        hasher.update(
            self.provider_request_id
                .as_deref()
                .unwrap_or_default()
                .as_bytes(),
        );
        hex_lower(&hasher.finalize())
    }

    pub fn from_legacy_observation(
        event: &AgentObservationEvent,
        provider: Option<String>,
    ) -> Self {
        let occurred_at = parse_timestamp_or_now(&event.timestamp);
        let tokens = event
            .token_usage
            .as_ref()
            .map(|usage| CanonicalTokenUsage {
                input_tokens: usage.input_tokens.unwrap_or(0),
                output_tokens: usage.output_tokens.unwrap_or(0),
                total_tokens: usage.total_tokens.unwrap_or_else(|| {
                    usage.input_tokens.unwrap_or(0) + usage.output_tokens.unwrap_or(0)
                }),
                estimated: true,
                source: UsageSource::ProxyObservation,
                ..CanonicalTokenUsage::default()
            })
            .unwrap_or_else(|| CanonicalTokenUsage {
                estimated: true,
                source: UsageSource::ProxyObservation,
                ..CanonicalTokenUsage::default()
            });
        let event_kind = match &event.event_kind {
            EventKind::LlmCall => AiUsageEventKind::ModelCallCompleted,
            EventKind::ToolCall => AiUsageEventKind::ToolCallCompleted,
            EventKind::ResourceAccess => AiUsageEventKind::ResourceAccess,
            EventKind::Decision => AiUsageEventKind::BudgetPreflight,
            EventKind::Generic => AiUsageEventKind::ModelCallCompleted,
        };
        let model = event
            .token_usage
            .as_ref()
            .and_then(|usage| usage.model.clone());
        let resource_type = event
            .resource_access
            .as_ref()
            .map(|resource| resource.resource_type.clone());
        let tool_name = event.tool_call.as_ref().map(|tool| tool.tool_name.clone());
        let mcp_server_id = event
            .tool_call
            .as_ref()
            .and_then(|tool| tool.server.clone());
        let mut policy_ids = Vec::new();
        if let Some(decision) = &event.decision {
            policy_ids = decision.matched_policy_ids.clone();
        }
        let metadata = json!({
            "legacy_observation_event_id": event.event_id,
            "legacy_event_kind": &event.event_kind,
            "risk_level": event.risk_level,
            "browser_scope": event.browser_scope,
        });

        Self {
            schema_version: Self::SCHEMA_VERSION.to_string(),
            event_id: event.event_id.clone(),
            event_kind,
            occurred_at,
            received_at: Utc::now(),
            tenant_id: event.tenant_id.clone(),
            workspace_id: None,
            device_id: None,
            actor_id_hash: None,
            actor_kind: None,
            trace_id: event.trace_id.clone(),
            span_id: format!("span_{}", event.event_id),
            parent_span_id: None,
            session_id: None,
            task_id: None,
            agent_run_id: None,
            agent_step_id: None,
            invocation_id: Some(event.event_id.clone()),
            agent_id: event.agent_id.clone(),
            agent_instance_id: None,
            agent_type: infer_agent_type(event),
            parent_agent_id: None,
            subagent_id: None,
            shadow_candidate_id: event.shadow_candidate_id.clone(),
            provider,
            provider_api: None,
            provider_request_id: None,
            model,
            model_version: None,
            service_tier: None,
            inference_region: None,
            surface: event.surface.clone(),
            pep_type: event.pep_type.clone(),
            control_mode: None,
            policy_ids,
            tokens,
            cost: CanonicalCostBreakdown::default(),
            tool_id: event.tool_id.clone(),
            tool_name,
            mcp_server_id,
            resource_id: event.resource_id.clone(),
            resource_type,
            latency_ms: event.latency_ms,
            status: "ok".to_string(),
            error_code: None,
            provider_usage_raw: json!({}),
            metadata,
            local_sequence: None,
            cloud_sync_status: Some("pending".to_string()),
            idempotency_key: String::new(),
        }
        .finalize()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCostLedgerEntryV2 {
    pub schema_version: String,
    pub event_id: String,
    pub tenant_id: String,
    pub agent_id: String,
    pub provider: String,
    pub model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_input_tokens: i64,
    pub reasoning_output_tokens: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
    pub currency: String,
    pub cost_source: CostSource,
    pub estimated: bool,
    pub timestamp: DateTime<Utc>,
}

impl From<CostLedgerEntry> for AiCostLedgerEntryV2 {
    fn from(entry: CostLedgerEntry) -> Self {
        Self {
            schema_version: "ai-cost-ledger-entry.v2".to_string(),
            event_id: entry.event_id,
            tenant_id: "local".to_string(),
            agent_id: entry.agent_id,
            provider: entry.provider,
            model: entry.model,
            input_tokens: entry.input_tokens,
            output_tokens: entry.output_tokens,
            cached_input_tokens: 0,
            reasoning_output_tokens: 0,
            total_tokens: entry.total_tokens,
            total_cost: entry.total_cost,
            currency: entry.currency,
            cost_source: CostSource::PriceCatalogExact,
            estimated: entry.estimated,
            timestamp: parse_timestamp_or_now(&entry.timestamp),
        }
    }
}

impl From<AgentObservationEvent> for AiUsageEventV1 {
    fn from(event: AgentObservationEvent) -> Self {
        let provider = event.provider.clone();
        Self::from_legacy_observation(&event, provider)
    }
}

fn infer_agent_type(event: &AgentObservationEvent) -> AgentType {
    if event.browser_scope.is_some() || event.surface.contains("browser") {
        return AgentType::BrowserAi;
    }
    if event.shadow_candidate_id.is_some() {
        return AgentType::ShadowAi;
    }
    AgentType::Unknown
}

fn parse_timestamp_or_now(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TokenUsage;

    #[test]
    fn legacy_observation_maps_to_canonical_usage_event() {
        let event = AgentObservationEvent {
            event_id: "evt_1".to_string(),
            tenant_id: "local".to_string(),
            trace_id: "trace_1".to_string(),
            agent_id: Some("agent_codex".to_string()),
            shadow_candidate_id: None,
            tool_id: None,
            resource_id: None,
            surface: "cli".to_string(),
            action: "model_call".to_string(),
            pep_type: Some("sdk".to_string()),
            risk_level: None,
            timestamp: "2026-06-26T00:00:00Z".to_string(),
            payload_json: "{}".to_string(),
            token_usage: Some(TokenUsage {
                input_tokens: Some(10),
                output_tokens: Some(5),
                total_tokens: None,
                model: Some("test-model".to_string()),
            }),
            browser_scope: None,
            event_kind: EventKind::LlmCall,
            decision: None,
            tool_call: None,
            resource_access: None,
            latency_ms: Some(25),
            provider: Some("local".to_string()),
        };

        let canonical = AiUsageEventV1::from(event);

        assert_eq!(canonical.schema_version, "ai-usage-event.v1");
        assert_eq!(canonical.tokens.total_tokens, 15);
        assert_eq!(canonical.provider.as_deref(), Some("local"));
        assert!(!canonical.idempotency_key.is_empty());
    }
}
