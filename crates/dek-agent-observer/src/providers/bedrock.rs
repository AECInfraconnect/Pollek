use crate::usage_model::{
    AiUsageEventKind, AiUsageEventV1, CanonicalCostBreakdown, CanonicalTokenUsage, UsageSource,
};
use crate::usage_normalizer::{as_i64, NormalizationContext, UsageNormalizeError, UsageNormalizer};
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct BedrockUsageNormalizer;

impl UsageNormalizer for BedrockUsageNormalizer {
    fn provider(&self) -> &'static str {
        "bedrock"
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
        let tokens = CanonicalTokenUsage {
            input_tokens: as_i64(usage, "inputTokens"),
            output_tokens: as_i64(usage, "outputTokens"),
            cached_input_tokens: as_i64(usage, "cacheReadInputTokens"),
            cache_write_input_tokens: as_i64(usage, "cacheWriteInputTokens"),
            source: UsageSource::ProviderResponse,
            ..CanonicalTokenUsage::default()
        }
        .with_provider_total(usage.get("totalTokens").and_then(Value::as_i64));
        let invoked_model = raw_response
            .pointer("/trace/promptRouter/invokedModelId")
            .and_then(Value::as_str)
            .map(ToString::to_string);

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
            provider_api: Some("converse".to_string()),
            provider_request_id: raw_response
                .get("requestId")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            model: invoked_model.or_else(|| {
                raw_response
                    .get("modelId")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            }),
            model_version: None,
            service_tier: None,
            inference_region: raw_response
                .get("region")
                .and_then(Value::as_str)
                .map(ToString::to_string),
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
            latency_ms: raw_response
                .get("metrics")
                .and_then(|metrics| metrics.get("latencyMs"))
                .and_then(Value::as_i64),
            status: raw_response
                .get("stopReason")
                .and_then(Value::as_str)
                .map(|reason| format!("ok:{}", reason))
                .unwrap_or_else(|| "ok".to_string()),
            error_code: None,
            provider_usage_raw: usage.clone(),
            metadata: json!({
                "bedrock_request_metadata": raw_response.get("requestMetadata"),
            }),
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
    fn maps_bedrock_cache_read_and_write_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let normalizer = BedrockUsageNormalizer;
        let event = normalizer.normalize(
            &json!({
                "requestId": "bed_1",
                "modelId": "anthropic.claude-test",
                "usage": {
                    "inputTokens": 70,
                    "outputTokens": 22,
                    "totalTokens": 102,
                    "cacheReadInputTokens": 8,
                    "cacheWriteInputTokens": 2
                },
                "metrics": { "latencyMs": 321 }
            }),
            NormalizationContext::local("trace", "span"),
        )?;

        assert_eq!(event.tokens.input_tokens, 70);
        assert_eq!(event.tokens.cached_input_tokens, 8);
        assert_eq!(event.tokens.cache_write_input_tokens, 2);
        assert_eq!(event.tokens.total_tokens, 102);
        assert_eq!(event.latency_ms, Some(321));
        Ok(())
    }
}
