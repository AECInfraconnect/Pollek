use crate::usage_model::{
    AiUsageEventKind, AiUsageEventV1, CanonicalCostBreakdown, CanonicalTokenUsage, UsageSource,
};
use crate::usage_normalizer::{as_i64, NormalizationContext, UsageNormalizeError, UsageNormalizer};
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct GeminiUsageNormalizer;

impl UsageNormalizer for GeminiUsageNormalizer {
    fn provider(&self) -> &'static str {
        "google"
    }

    fn normalize(
        &self,
        raw_response: &Value,
        ctx: NormalizationContext,
    ) -> Result<AiUsageEventV1, UsageNormalizeError> {
        let usage =
            raw_response
                .get("usageMetadata")
                .ok_or_else(|| UsageNormalizeError::MissingUsage {
                    provider: self.provider().to_string(),
                })?;
        let mut by_modality = BTreeMap::new();
        collect_modalities(usage, "promptTokensDetails", "input", &mut by_modality);
        collect_modalities(usage, "candidatesTokensDetails", "output", &mut by_modality);
        let tokens = CanonicalTokenUsage {
            input_tokens: as_i64(usage, "promptTokenCount"),
            cached_input_tokens: as_i64(usage, "cachedContentTokenCount"),
            output_tokens: as_i64(usage, "candidatesTokenCount"),
            tool_prompt_tokens: as_i64(usage, "toolUsePromptTokenCount"),
            reasoning_output_tokens: as_i64(usage, "thoughtsTokenCount"),
            by_modality,
            source: UsageSource::ProviderResponse,
            ..CanonicalTokenUsage::default()
        }
        .with_provider_total(usage.get("totalTokenCount").and_then(Value::as_i64));

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
            provider_api: Some("generate_content".to_string()),
            provider_request_id: raw_response
                .get("responseId")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            model: raw_response
                .get("modelVersion")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            model_version: raw_response
                .get("modelVersion")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            service_tier: usage
                .get("serviceTier")
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

fn collect_modalities(
    usage: &Value,
    key: &str,
    direction: &str,
    by_modality: &mut BTreeMap<String, i64>,
) {
    if let Some(items) = usage.get(key).and_then(Value::as_array) {
        for item in items {
            let modality = item
                .get("modality")
                .or_else(|| item.get("modalityType"))
                .and_then(Value::as_str);
            let token_count = item
                .get("tokenCount")
                .or_else(|| item.get("token_count"))
                .and_then(Value::as_i64);
            if let (Some(modality), Some(token_count)) = (modality, token_count) {
                by_modality.insert(
                    format!("{}_{}", direction, modality.to_lowercase()),
                    token_count,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_gemini_usage_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let normalizer = GeminiUsageNormalizer;
        let event = normalizer.normalize(
            &json!({
                "responseId": "gem_1",
                "modelVersion": "gemini-test",
                "usageMetadata": {
                    "promptTokenCount": 80,
                    "cachedContentTokenCount": 10,
                    "candidatesTokenCount": 30,
                    "toolUsePromptTokenCount": 6,
                    "thoughtsTokenCount": 8,
                    "totalTokenCount": 134,
                    "promptTokensDetails": [
                        { "modality": "TEXT", "tokenCount": 70 },
                        { "modality": "IMAGE", "tokenCount": 10 }
                    ]
                }
            }),
            NormalizationContext::local("trace", "span"),
        )?;

        assert_eq!(event.tokens.input_tokens, 80);
        assert_eq!(event.tokens.cached_input_tokens, 10);
        assert_eq!(event.tokens.output_tokens, 30);
        assert_eq!(event.tokens.tool_prompt_tokens, 6);
        assert_eq!(event.tokens.reasoning_output_tokens, 8);
        assert_eq!(event.tokens.total_tokens, 134);
        assert_eq!(event.tokens.by_modality.get("input_image"), Some(&10));
        Ok(())
    }
}
