use crate::usage_model::{AgentType, AiUsageEventV1};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub trait UsageNormalizer: Send + Sync {
    fn provider(&self) -> &'static str;

    fn normalize(
        &self,
        raw_response: &Value,
        ctx: NormalizationContext,
    ) -> Result<AiUsageEventV1, UsageNormalizeError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationContext {
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
    pub agent_type: AgentType,
    pub parent_agent_id: Option<String>,
    pub subagent_id: Option<String>,
    pub shadow_candidate_id: Option<String>,
    pub surface: String,
    pub pep_type: Option<String>,
    pub control_mode: Option<String>,
    pub policy_ids: Vec<String>,
}

impl NormalizationContext {
    pub fn local(trace_id: impl Into<String>, span_id: impl Into<String>) -> Self {
        Self {
            tenant_id: "local".to_string(),
            workspace_id: None,
            device_id: None,
            actor_id_hash: None,
            actor_kind: None,
            trace_id: trace_id.into(),
            span_id: span_id.into(),
            parent_span_id: None,
            session_id: None,
            task_id: None,
            agent_run_id: None,
            agent_step_id: None,
            invocation_id: None,
            agent_id: None,
            agent_type: AgentType::Unknown,
            parent_agent_id: None,
            subagent_id: None,
            shadow_candidate_id: None,
            surface: "sdk".to_string(),
            pep_type: None,
            control_mode: None,
            policy_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum UsageNormalizeError {
    #[error("missing provider usage object for {provider}")]
    MissingUsage { provider: String },
    #[error("unsupported provider response shape for {provider}: {message}")]
    UnsupportedShape { provider: String, message: String },
}

pub(crate) fn as_i64(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0)
}

pub(crate) fn nested_i64(value: &Value, parent: &str, key: &str) -> i64 {
    value
        .get(parent)
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_i64)
        .unwrap_or(0)
}

pub(crate) fn optional_nested_i64(value: &Value, parent: &str, key: &str) -> Option<i64> {
    value
        .get(parent)
        .and_then(|obj| obj.get(key))
        .and_then(Value::as_i64)
}
