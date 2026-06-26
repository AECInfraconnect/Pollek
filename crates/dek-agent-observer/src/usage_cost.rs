use crate::usage_model::{CanonicalCostBreakdown, CanonicalTokenUsage, CostSource};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceCatalogV2 {
    pub schema_version: String,
    pub catalog_version: String,
    pub default_currency: String,
    pub models: Vec<ModelPriceRuleV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPriceRuleV2 {
    pub provider: String,
    #[serde(default)]
    pub provider_api: Option<String>,
    pub model_match: String,
    #[serde(default)]
    pub effective_from: Option<String>,
    #[serde(default)]
    pub effective_to: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub prices_per_1m: BTreeMap<String, f64>,
    #[serde(default)]
    pub tiers: Vec<PriceTierV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceTierV2 {
    pub tier_id: String,
    #[serde(default)]
    pub priority: i32,
    pub condition: PriceTierCondition,
    #[serde(default)]
    pub prices_per_1m: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceTierCondition {
    pub usage_key_regex: String,
    pub operator: String,
    pub value: i64,
}

#[derive(Debug, Clone)]
pub struct CostCalculationInput<'a> {
    pub provider: &'a str,
    pub provider_api: Option<&'a str>,
    pub model: &'a str,
    pub occurred_at: DateTime<Utc>,
    pub tokens: &'a CanonicalTokenUsage,
    pub provider_reported_cost: Option<f64>,
    pub provider_reported_currency: Option<&'a str>,
}

pub fn calculate_cost_v2(
    catalog: &PriceCatalogV2,
    input: CostCalculationInput<'_>,
) -> CanonicalCostBreakdown {
    if let Some(provider_cost) = input.provider_reported_cost {
        return CanonicalCostBreakdown {
            currency: input
                .provider_reported_currency
                .unwrap_or(&catalog.default_currency)
                .to_string(),
            total_cost: provider_cost,
            cost_source: CostSource::ProviderReported,
            estimated: false,
            ..CanonicalCostBreakdown::default()
        };
    }

    let Some(rule) = catalog
        .models
        .iter()
        .filter(|rule| rule_matches(rule, &input))
        .max_by_key(|rule| rule_specificity(rule))
    else {
        return CanonicalCostBreakdown {
            currency: catalog.default_currency.clone(),
            cost_source: CostSource::Unknown,
            estimated: true,
            ..CanonicalCostBreakdown::default()
        };
    };

    let tier = matching_tier(rule, input.tokens);
    let mut prices = rule.prices_per_1m.clone();
    let mut cost_source = CostSource::PriceCatalogExact;
    let mut pricing_tier_id = None;
    if let Some(tier) = tier {
        for (key, value) in &tier.prices_per_1m {
            prices.insert(key.clone(), *value);
        }
        cost_source = CostSource::PriceCatalogTiered;
        pricing_tier_id = Some(tier.tier_id.clone());
    }

    let mut breakdown = CanonicalCostBreakdown {
        currency: rule
            .currency
            .clone()
            .unwrap_or_else(|| catalog.default_currency.clone()),
        price_catalog_version: Some(catalog.catalog_version.clone()),
        pricing_tier_id,
        cost_source,
        estimated: true,
        ..CanonicalCostBreakdown::default()
    };
    breakdown.input_cost = price_for("input_tokens", input.tokens.input_tokens, &prices);
    breakdown.output_cost = price_for("output_tokens", input.tokens.output_tokens, &prices);
    breakdown.cached_input_cost = price_for(
        "cached_input_tokens",
        input.tokens.cached_input_tokens,
        &prices,
    );
    breakdown.cache_write_input_cost = price_for(
        "cache_write_input_tokens",
        input.tokens.cache_write_input_tokens,
        &prices,
    );
    breakdown.reasoning_output_cost = price_for(
        "reasoning_output_tokens",
        input.tokens.reasoning_output_tokens,
        &prices,
    );
    breakdown.tool_cost = price_for(
        "tool_prompt_tokens",
        input.tokens.tool_prompt_tokens,
        &prices,
    ) + price_for(
        "tool_result_tokens",
        input.tokens.tool_result_tokens,
        &prices,
    );
    breakdown.image_cost = price_for(
        "image_input_tokens",
        input.tokens.image_input_tokens,
        &prices,
    ) + price_for(
        "image_output_tokens",
        input.tokens.image_output_tokens,
        &prices,
    );
    breakdown.audio_cost = price_for(
        "audio_input_tokens",
        input.tokens.audio_input_tokens,
        &prices,
    ) + price_for(
        "audio_output_tokens",
        input.tokens.audio_output_tokens,
        &prices,
    );
    let mut ext_cost = 0.0;
    for (key, tokens) in &input.tokens.usage_details_ext {
        let cost = price_for(key, *tokens, &prices);
        if cost > 0.0 {
            breakdown.cost_details_ext.insert(key.clone(), cost);
            ext_cost += cost;
        }
    }
    breakdown.total_cost = breakdown.input_cost
        + breakdown.output_cost
        + breakdown.cached_input_cost
        + breakdown.cache_write_input_cost
        + breakdown.reasoning_output_cost
        + breakdown.tool_cost
        + breakdown.image_cost
        + breakdown.audio_cost
        + ext_cost;
    breakdown
}

fn rule_matches(rule: &ModelPriceRuleV2, input: &CostCalculationInput<'_>) -> bool {
    if rule.provider != input.provider {
        return false;
    }
    if let Some(rule_api) = &rule.provider_api {
        if Some(rule_api.as_str()) != input.provider_api {
            return false;
        }
    }
    if !effective_at(rule, input.occurred_at) {
        return false;
    }
    model_matches(&rule.model_match, input.model)
}

fn model_matches(pattern: &str, model: &str) -> bool {
    if pattern == model {
        return true;
    }
    Regex::new(pattern)
        .map(|regex| regex.is_match(model))
        .unwrap_or(false)
}

fn effective_at(rule: &ModelPriceRuleV2, occurred_at: DateTime<Utc>) -> bool {
    let after_from = match &rule.effective_from {
        Some(value) => DateTime::parse_from_rfc3339(value)
            .map(|dt| occurred_at >= dt.with_timezone(&Utc))
            .unwrap_or(true),
        None => true,
    };
    let before_to = match &rule.effective_to {
        Some(value) => DateTime::parse_from_rfc3339(value)
            .map(|dt| occurred_at < dt.with_timezone(&Utc))
            .unwrap_or(true),
        None => true,
    };
    after_from && before_to
}

fn rule_specificity(rule: &ModelPriceRuleV2) -> usize {
    rule.provider.len() + rule.model_match.len() + usize::from(rule.provider_api.is_some())
}

fn matching_tier<'a>(
    rule: &'a ModelPriceRuleV2,
    tokens: &CanonicalTokenUsage,
) -> Option<&'a PriceTierV2> {
    rule.tiers
        .iter()
        .filter(|tier| tier_condition_matches(&tier.condition, tokens))
        .max_by_key(|tier| tier.priority)
}

fn tier_condition_matches(condition: &PriceTierCondition, tokens: &CanonicalTokenUsage) -> bool {
    let Ok(regex) = Regex::new(&condition.usage_key_regex) else {
        return false;
    };
    let total: i64 = tokens
        .token_class_counts()
        .iter()
        .filter(|(key, _)| regex.is_match(key))
        .map(|(_, value)| *value)
        .sum();
    match condition.operator.as_str() {
        "gt" => total > condition.value,
        "gte" => total >= condition.value,
        "lt" => total < condition.value,
        "lte" => total <= condition.value,
        "eq" => total == condition.value,
        _ => false,
    }
}

fn price_for(key: &str, tokens: i64, prices: &BTreeMap<String, f64>) -> f64 {
    if tokens <= 0 {
        return 0.0;
    }
    let per_1m = prices.get(key).copied().unwrap_or(0.0);
    (tokens as f64 / 1_000_000.0) * per_1m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_catalog() -> PriceCatalogV2 {
        PriceCatalogV2 {
            schema_version: "price-catalog.v2".to_string(),
            catalog_version: "test-2026-06-26".to_string(),
            default_currency: "USD".to_string(),
            models: vec![ModelPriceRuleV2 {
                provider: "fixture".to_string(),
                provider_api: Some("responses".to_string()),
                model_match: "^fixture-model$".to_string(),
                effective_from: None,
                effective_to: None,
                source_url: None,
                currency: Some("USD".to_string()),
                prices_per_1m: BTreeMap::from([
                    ("input_tokens".to_string(), 2.0),
                    ("output_tokens".to_string(), 8.0),
                    ("cached_input_tokens".to_string(), 0.5),
                    ("reasoning_output_tokens".to_string(), 12.0),
                ]),
                tiers: Vec::new(),
            }],
        }
    }

    #[test]
    fn prices_multiple_token_classes() {
        let tokens = CanonicalTokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            cached_input_tokens: 1_000_000,
            reasoning_output_tokens: 250_000,
            ..CanonicalTokenUsage::default()
        };
        let cost = calculate_cost_v2(
            &fixture_catalog(),
            CostCalculationInput {
                provider: "fixture",
                provider_api: Some("responses"),
                model: "fixture-model",
                occurred_at: Utc::now(),
                tokens: &tokens,
                provider_reported_cost: None,
                provider_reported_currency: None,
            },
        );

        assert_eq!(cost.total_cost, 9.5);
        assert_eq!(
            cost.price_catalog_version.as_deref(),
            Some("test-2026-06-26")
        );
    }

    #[test]
    fn provider_reported_cost_takes_precedence() {
        let tokens = CanonicalTokenUsage::default();
        let cost = calculate_cost_v2(
            &fixture_catalog(),
            CostCalculationInput {
                provider: "fixture",
                provider_api: Some("responses"),
                model: "fixture-model",
                occurred_at: Utc::now(),
                tokens: &tokens,
                provider_reported_cost: Some(4.25),
                provider_reported_currency: Some("USD"),
            },
        );

        assert_eq!(cost.total_cost, 4.25);
        assert_eq!(cost.cost_source, CostSource::ProviderReported);
        assert!(!cost.estimated);
    }
}
