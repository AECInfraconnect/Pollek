//! Provider-response -> canonical usage event conversion and price catalogs:
//! parse provider/generic usage objects into AiUsageEventV1, infer the
//! provider, and price tokens via the v2 / embedded / legacy-v1 catalogs.

use super::*;

pub(super) fn apply_cost_catalog(mut event: AiUsageEventV1) -> AiUsageEventV1 {
    if !matches!(event.cost.cost_source, CostSource::Unknown) {
        return event;
    }
    let Some(provider) = event.provider.clone() else {
        return event;
    };
    let Some(model) = event.model.clone() else {
        return event;
    };
    let Some(catalog) = load_price_catalog_v2() else {
        return event;
    };
    event.cost = calculate_cost_v2(
        &catalog,
        CostCalculationInput {
            provider: &provider,
            provider_api: event.provider_api.as_deref(),
            model: &model,
            occurred_at: event.occurred_at,
            tokens: &event.tokens,
            provider_reported_cost: None,
            provider_reported_currency: None,
        },
    );
    event
}

pub(super) fn usage_event_from_provider_response(
    state: &AppState,
    tenant: &str,
    req: ProviderResponseUsageRequest,
) -> Result<AiUsageEventV1, String> {
    let provider = req
        .provider
        .clone()
        .or_else(|| req.host.as_deref().and_then(provider_from_host))
        .or_else(|| infer_provider_from_response(&req.raw_response))
        .ok_or_else(|| {
            "provider or recognizable host is required for exact usage capture".to_string()
        })?;

    let trace_id = req
        .trace_id
        .clone()
        .unwrap_or_else(|| format!("trace_{}", uuid::Uuid::new_v4()));
    let span_id = req
        .span_id
        .clone()
        .unwrap_or_else(|| format!("span_{}", uuid::Uuid::new_v4()));
    let mut ctx = NormalizationContext::local(trace_id, span_id);
    ctx.tenant_id = tenant.to_string();
    ctx.workspace_id = Some(state.identity.workspace_id.clone());
    ctx.device_id = Some(local_device_id());
    ctx.agent_id = req.agent_id.clone();
    ctx.agent_type = req.agent_type.clone().unwrap_or(AgentType::Unknown);
    ctx.surface = req
        .surface
        .clone()
        .unwrap_or_else(|| "local_usage_capture".to_string());
    ctx.pep_type = req.pep_type.clone();
    ctx.control_mode = req.control_mode.clone();
    ctx.session_id = req.session_id.clone();
    ctx.task_id = req.task_id.clone();
    ctx.invocation_id = req.invocation_id.clone();

    let mut event = match provider.as_str() {
        "openai" | "azure-openai" | "deepseek" | "xai" | "groq" | "together" | "mistral"
        | "cohere" | "openrouter" | "perplexity" | "fireworks" | "cerebras" | "replicate"
        | "huggingface" => dek_agent_observer::providers::OpenAiUsageNormalizer
            .normalize(&req.raw_response, ctx.clone())
            .or_else(|_| usage_event_from_generic_usage_object(&provider, &req, ctx.clone())),
        "anthropic" => dek_agent_observer::providers::AnthropicUsageNormalizer
            .normalize(&req.raw_response, ctx.clone())
            .or_else(|_| usage_event_from_generic_usage_object(&provider, &req, ctx.clone())),
        "google" | "gemini" => dek_agent_observer::providers::GeminiUsageNormalizer
            .normalize(&req.raw_response, ctx.clone())
            .or_else(|_| usage_event_from_generic_usage_object("google", &req, ctx.clone())),
        "bedrock" | "aws-bedrock" => dek_agent_observer::providers::BedrockUsageNormalizer
            .normalize(&req.raw_response, ctx.clone())
            .or_else(|_| usage_event_from_generic_usage_object("bedrock", &req, ctx.clone())),
        "ollama" | "local" => usage_event_from_generic_usage_object(&provider, &req, ctx.clone()),
        _ => usage_event_from_generic_usage_object(&provider, &req, ctx.clone()),
    }
    .map_err(|err| err.to_string())?;

    event.provider = Some(provider);
    if event.provider_api.is_none() {
        event.provider_api = req.provider_api;
    }
    event.resource_id = req.resource_id;
    event.resource_type = req.resource_type;
    event.metadata = merge_usage_metadata(
        event.metadata,
        json!({
            "capture_quality": "exact_provider_response",
            "capture_source": req.source.unwrap_or_else(|| "local_provider_response".to_string()),
            "host": req.host,
            "plaintext_seen_by": event.pep_type.clone().unwrap_or_else(|| "approved_local_integration".to_string())
        }),
    );
    Ok(event.finalize())
}

pub(super) fn usage_event_from_generic_usage_object(
    provider: &str,
    req: &ProviderResponseUsageRequest,
    ctx: NormalizationContext,
) -> Result<AiUsageEventV1, dek_agent_observer::usage_normalizer::UsageNormalizeError> {
    let host = req
        .host
        .clone()
        .unwrap_or_else(|| host_for_provider(provider).to_string());
    let (_parsed_provider, usage) =
        dek_agent_observer::egress_parser::parse_llm_usage(&host, &req.raw_response).ok_or_else(
            || dek_agent_observer::usage_normalizer::UsageNormalizeError::MissingUsage {
                provider: provider.to_string(),
            },
        )?;
    let tokens = CanonicalTokenUsage {
        input_tokens: usage.input_tokens.unwrap_or(0),
        output_tokens: usage.output_tokens.unwrap_or(0),
        total_tokens: usage
            .total_tokens
            .unwrap_or_else(|| usage.input_tokens.unwrap_or(0) + usage.output_tokens.unwrap_or(0)),
        estimated: false,
        source: UsageSource::ProviderResponse,
        ..CanonicalTokenUsage::default()
    };

    Ok(AiUsageEventV1 {
        schema_version: AiUsageEventV1::SCHEMA_VERSION.to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
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
        provider: Some(provider.to_string()),
        provider_api: req.provider_api.clone(),
        provider_request_id: req
            .raw_response
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string),
        model: usage.model,
        model_version: req
            .raw_response
            .get("modelVersion")
            .and_then(Value::as_str)
            .map(str::to_string),
        service_tier: None,
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
        provider_usage_raw: usage_subtree(&req.raw_response),
        metadata: json!({}),
        local_sequence: None,
        cloud_sync_status: Some("pending".to_string()),
        idempotency_key: String::new(),
    }
    .finalize())
}

pub(super) fn local_device_id() -> String {
    let seed = format!(
        "{}:{}:{}",
        std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "local".into()),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest as _;
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();
    format!("dev_{}", hex::encode(&digest[..8]))
}

pub(super) fn provider_from_host(host: &str) -> Option<String> {
    let host = host.to_ascii_lowercase();
    if host.contains("openai.azure.com") || host.contains("azure.com/openai") {
        Some("azure-openai".into())
    } else if host.contains("openai.com") || host.contains("chatgpt.com") {
        Some("openai".into())
    } else if host.contains("anthropic.com") || host.contains("claude.ai") {
        Some("anthropic".into())
    } else if host.contains("googleapis.com")
        || host.contains("gemini.google.com")
        || host.contains("aistudio.google.com")
    {
        Some("google".into())
    } else if host.contains("deepseek.com") {
        Some("deepseek".into())
    } else if host.contains("api.x.ai") || host.contains("x.ai") {
        Some("xai".into())
    } else if host.contains("api.groq.com") || host.contains("groq.com") {
        Some("groq".into())
    } else if host.contains("api.together.xyz") || host.contains("together.ai") {
        Some("together".into())
    } else if host.contains("mistral.ai") {
        Some("mistral".into())
    } else if host.contains("cohere.com") {
        Some("cohere".into())
    } else if host.contains("openrouter.ai") {
        Some("openrouter".into())
    } else if host.contains("perplexity.ai") {
        Some("perplexity".into())
    } else if host.contains("fireworks.ai") {
        Some("fireworks".into())
    } else if host.contains("cerebras.ai") {
        Some("cerebras".into())
    } else if host.contains("replicate.com") {
        Some("replicate".into())
    } else if host.contains("huggingface.co") {
        Some("huggingface".into())
    } else if host.contains("11434") || host.contains("ollama") {
        Some("ollama".into())
    } else {
        None
    }
}

pub(super) fn infer_provider_from_response(value: &Value) -> Option<String> {
    let blob = value
        .get("model")
        .or_else(|| value.get("modelVersion"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if blob.contains("claude") {
        Some("anthropic".into())
    } else if blob.contains("gpt")
        || blob.contains("o1")
        || blob.contains("o3")
        || blob.contains("o4")
    {
        Some("openai".into())
    } else if blob.contains("gemini") {
        Some("google".into())
    } else if blob.contains("deepseek") {
        Some("deepseek".into())
    } else if blob.contains("grok") {
        Some("xai".into())
    } else if blob.contains("llama") && blob.contains("groq") {
        Some("groq".into())
    } else if blob.contains("sonar") {
        Some("perplexity".into())
    } else if blob.contains("mistral") || blob.contains("mixtral") {
        Some("mistral".into())
    } else if blob.contains("command") || blob.contains("embed-") {
        Some("cohere".into())
    } else {
        None
    }
}

pub(super) fn host_for_provider(provider: &str) -> &'static str {
    match provider {
        "openai" => "api.openai.com",
        "azure-openai" => "openai.azure.com",
        "anthropic" => "api.anthropic.com",
        "google" | "gemini" => "generativelanguage.googleapis.com",
        "deepseek" => "api.deepseek.com",
        "xai" => "api.x.ai",
        "groq" => "api.groq.com",
        "together" => "api.together.xyz",
        "mistral" => "api.mistral.ai",
        "cohere" => "api.cohere.com",
        "openrouter" => "openrouter.ai",
        "perplexity" => "api.perplexity.ai",
        "fireworks" => "api.fireworks.ai",
        "cerebras" => "api.cerebras.ai",
        "replicate" => "api.replicate.com",
        "huggingface" => "router.huggingface.co",
        "ollama" => "127.0.0.1:11434",
        _ => "local",
    }
}

pub(super) fn usage_subtree(value: &Value) -> Value {
    value
        .get("usage")
        .or_else(|| value.get("usageMetadata"))
        .or_else(|| value.get("message_delta").and_then(|m| m.get("usage")))
        .cloned()
        .unwrap_or_else(|| {
            let mut usage = Map::new();
            for key in ["prompt_eval_count", "eval_count", "total_duration"] {
                if let Some(v) = value.get(key) {
                    usage.insert(key.to_string(), v.clone());
                }
            }
            Value::Object(usage)
        })
}

pub(crate) fn merge_usage_metadata(existing: Value, extra: Value) -> Value {
    let mut map = match existing {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    if let Value::Object(extra) = extra {
        for (key, value) in extra {
            if !value.is_null() {
                map.insert(key, value);
            }
        }
    }
    Value::Object(map)
}

pub(super) fn load_price_catalog_v2() -> Option<PriceCatalogV2> {
    let path = std::path::PathBuf::from("pollek-local-data/price_catalog.v2.json");
    if let Some(catalog) = std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
    {
        return Some(catalog);
    }

    if let Some(catalog) = load_legacy_price_catalog_v1() {
        return Some(catalog);
    }

    embedded_price_catalog()
}

/// Built-in price catalog used when no `pollek-local-data/price_catalog.*`
/// file exists on the device (fresh installs, or the process running from a
/// different working directory). Without this fallback every usage event kept
/// cost = $0.00 even though exact tokens had been captured. Prices are
/// estimated list prices per 1M tokens; a shipped catalog file always wins.
pub(super) fn embedded_price_catalog() -> Option<PriceCatalogV2> {
    static CATALOG: std::sync::OnceLock<Option<PriceCatalogV2>> = std::sync::OnceLock::new();
    CATALOG
        .get_or_init(|| {
            serde_json::from_str(include_str!("../../data/price_catalog.default.json")).ok()
        })
        .clone()
}

pub(super) fn load_legacy_price_catalog_v1() -> Option<PriceCatalogV2> {
    let path = std::path::PathBuf::from("pollek-local-data/price_catalog.v1.json");
    let value: Value = std::fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())?;
    let providers = value.get("providers")?.as_object()?;
    let default_currency = value
        .get("currency")
        .and_then(Value::as_str)
        .unwrap_or("USD")
        .to_string();
    let catalog_version = value
        .get("catalog_version")
        .and_then(Value::as_str)
        .unwrap_or("legacy-v1")
        .to_string();
    let mut models = Vec::new();

    for (provider, provider_models) in providers {
        let Some(provider_models) = provider_models.as_object() else {
            continue;
        };
        for (model_match, price) in provider_models {
            let input = price
                .get("input_per_1m")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let output = price
                .get("output_per_1m")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let mut prices_per_1m = std::collections::BTreeMap::new();
            prices_per_1m.insert("input_tokens".to_string(), input);
            prices_per_1m.insert("output_tokens".to_string(), output);
            models.push(ModelPriceRuleV2 {
                provider: provider.clone(),
                provider_api: None,
                model_match: model_match.clone(),
                effective_from: None,
                effective_to: None,
                source_url: None,
                currency: Some(default_currency.clone()),
                prices_per_1m,
                tiers: Vec::new(),
            });
        }
    }

    Some(PriceCatalogV2 {
        schema_version: "price-catalog.v2".to_string(),
        catalog_version,
        default_currency,
        models,
    })
}
