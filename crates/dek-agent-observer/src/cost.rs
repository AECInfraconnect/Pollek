use crate::model::{CostObservation, TokenUsageObservation};
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

pub fn calculate_cost(tokens: &TokenUsageObservation, catalog: &PriceCatalog) -> CostObservation {
    let provider = match tokens.provider.as_ref() {
        Some(p) => p,
        None => return unknown_cost(catalog, tokens.estimated),
    };
    let model = match tokens.model.as_ref() {
        Some(m) => m,
        None => return unknown_cost(catalog, tokens.estimated),
    };

    let price = match catalog.providers.get(provider).and_then(|m| m.get(model)) {
        Some(p) => p,
        None => return unknown_cost(catalog, tokens.estimated),
    };

    let input_cost = tokens
        .input_tokens
        .map(|t| (t as f64 / 1_000_000.0) * price.input_per_1m);
    let output_cost = tokens
        .output_tokens
        .map(|t| (t as f64 / 1_000_000.0) * price.output_per_1m);

    CostObservation {
        currency: catalog.currency.clone(),
        input_cost,
        output_cost,
        total_cost: Some(input_cost.unwrap_or(0.0) + output_cost.unwrap_or(0.0)),
        price_catalog_version: Some(catalog.catalog_version.clone()),
        estimated: tokens.estimated,
    }
}

fn unknown_cost(catalog: &PriceCatalog, estimated: bool) -> CostObservation {
    CostObservation {
        currency: catalog.currency.clone(),
        input_cost: None,
        output_cost: None,
        total_cost: None,
        price_catalog_version: Some(catalog.catalog_version.clone()),
        estimated,
    }
}
