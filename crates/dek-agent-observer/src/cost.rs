use crate::model::{AgentObservationEvent, CostLedgerEntry, TokenUsage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceCatalog {
    pub catalog_version: String,
    pub currency: String,
    // provider -> model -> Price
    pub providers: HashMap<String, HashMap<String, ModelPrice>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPrice {
    pub input_per_1m: f64,
    pub output_per_1m: f64,
}

pub fn calculate_cost(
    event: &AgentObservationEvent,
    provider: &str,
    catalog: &PriceCatalog,
) -> Option<CostLedgerEntry> {
    let tokens = event.token_usage.as_ref()?;
    let model = tokens.model.as_deref().unwrap_or("unknown");

    let price = catalog
        .providers
        .get(provider)
        .and_then(|m| m.get(model))?;

    let input_cost = (tokens.input_tokens.unwrap_or(0) as f64 / 1_000_000.0) * price.input_per_1m;
    let output_cost = (tokens.output_tokens.unwrap_or(0) as f64 / 1_000_000.0) * price.output_per_1m;

    Some(CostLedgerEntry {
        event_id: event.event_id.clone(),
        agent_id: event.agent_id.clone().unwrap_or_else(|| "unknown".to_string()),
        provider: provider.to_string(),
        model: Some(model.to_string()),
        input_tokens: tokens.input_tokens.unwrap_or(0),
        output_tokens: tokens.output_tokens.unwrap_or(0),
        total_tokens: tokens.input_tokens.unwrap_or(0) + tokens.output_tokens.unwrap_or(0),
        input_cost,
        output_cost,
        total_cost: input_cost + output_cost,
        currency: catalog.currency.clone(),
        estimated: true,
        timestamp: event.timestamp.clone(),
    })
}
