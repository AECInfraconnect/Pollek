use crate::usage_model::{
    AiUsageEventKind, AiUsageEventV1, CanonicalCostBreakdown, CanonicalTokenUsage, UsageSource,
};
use crate::usage_normalizer::{
    as_i64, nested_i64, NormalizationContext, UsageNormalizeError, UsageNormalizer,
};
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct OpenAiUsageNormalizer;

impl UsageNormalizer for OpenAiUsageNormalizer {
    fn provider(&self) -> &'static str {
        "openai"
    }

    fn normalize(
        &self,
        raw_response: &Value,
        ctx: NormalizationContext,
    ) -> Result<AiUsageEventV1, UsageNormalizeError> {
        let usage = raw_response
            .get("usage")
            .ok_or_else(|| UsageNormalizeError::MissingUsage {
                provider: self.provider().to_string(),
            })?;
        let provider_total = usage.get("total_tokens").and_then(Value::as_i64);
        let tokens = CanonicalTokenUsage {
            input_tokens: as_i64(usage, "input_tokens").max(as_i64(usage, "prompt_tokens")),
            output_tokens: as_i64(usage, "output_tokens").max(as_i64(usage, "completion_tokens")),
            cached_input_tokens: nested_i64(usage, "input_tokens_details", "cached_tokens"),
            reasoning_output_tokens: nested_i64(usage, "output_tokens_details", "reasoning_tokens"),
            source: UsageSource::ProviderResponse,
            ..CanonicalTokenUsage::default()
        }
        .with_provider_total(provider_total);
        let provider_api = if usage.get("prompt_tokens").is_some() {
            "chat_completions"
        } else {
            "responses"
        };

        Ok(AiUsageEventV1 {
            schema_version: AiUsageEventV1::SCHEMA_VERSION.to_string(),
            event_id: Uuid::new_v4().to_string(),
            event_kind: AiUsageEventKind::ModelCallCompleted,
            occurred_at: Utc::now(),
            received_at: Utc::now(),
            tenant_id: ctx.tenant_id,
            workspace_id: ctx.workspace_id,
            device_id: ctx.device_id,
            actor_id_hash: ctx.actor_id_hash,
            actor_kind: ctx.actor_kind,
            trace_id: ctx.trace_id,
            span_id: ctx.span_id,
            parent_span_id: ctx.parent_span_id,
            session_id: ctx.session_id,
            task_id: ctx.task_id,
            agent_run_id: ctx.agent_run_id,
            agent_step_id: ctx.agent_step_id,
            invocation_id: ctx.invocation_id,
            agent_id: ctx.agent_id,
            agent_instance_id: None,
            agent_type: ctx.agent_type,
            parent_agent_id: ctx.parent_agent_id,
            subagent_id: ctx.subagent_id,
            shadow_candidate_id: ctx.shadow_candidate_id,
            provider: Some(self.provider().to_string()),
            provider_api: Some(provider_api.to_string()),
            provider_request_id: raw_response
                .get("id")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            model: raw_response
                .get("model")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            model_version: None,
            service_tier: raw_response
                .get("service_tier")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            inference_region: None,
            surface: ctx.surface,
            pep_type: ctx.pep_type,
            control_mode: ctx.control_mode,
            policy_ids: ctx.policy_ids,
            tokens,
            cost: CanonicalCostBreakdown::default(),
            tool_id: None,
            tool_name: None,
            mcp_server_id: None,
            resource_id: None,
            resource_type: None,
            latency_ms: None,
            status: "ok".to_string(),
            error_code: None,
            provider_usage_raw: usage.clone(),
            metadata: json!({}),
            local_sequence: None,
            cloud_sync_status: Some("pending".to_string()),
            idempotency_key: String::new(),
        }
        .finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_openai_cached_and_reasoning_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let normalizer = OpenAiUsageNormalizer;
        let event = normalizer.normalize(
            &json!({
                "id": "resp_1",
                "model": "gpt-test",
                "usage": {
                    "input_tokens": 100,
                    "input_tokens_details": { "cached_tokens": 40 },
                    "output_tokens": 25,
                    "output_tokens_details": { "reasoning_tokens": 7 },
                    "total_tokens": 125
                }
            }),
            NormalizationContext::local("trace", "span"),
        )?;

        assert_eq!(event.provider.as_deref(), Some("openai"));
        assert_eq!(event.tokens.input_tokens, 100);
        assert_eq!(event.tokens.cached_input_tokens, 40);
        assert_eq!(event.tokens.reasoning_output_tokens, 7);
        assert_eq!(event.tokens.total_tokens, 125);
        Ok(())
    }
}
