use crate::model::{AgentObservationEvent, CostLedgerEntry};
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

    let price = catalog.providers.get(provider).and_then(|m| m.get(model))?;

    let input_cost = (tokens.input_tokens.unwrap_or(0) as f64 / 1_000_000.0) * price.input_per_1m;
    let output_cost =
        (tokens.output_tokens.unwrap_or(0) as f64 / 1_000_000.0) * price.output_per_1m;

    Some(CostLedgerEntry {
        event_id: event.event_id.clone(),
        agent_id: event
            .agent_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
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
#[derive(Debug, Clone)]
pub struct BudgetPolicy {
    pub agent_id: String,
    pub daily_cost_cap_usd: f64,
    pub daily_token_cap: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BudgetDecision {
    WithinBudget,
    CostExceeded,
    TokenExceeded,
}

/// รวมยอดวันนี้จาก ledger แล้วเทียบ cap
pub fn check_budget(policy: &BudgetPolicy, todays_entries: &[CostLedgerEntry]) -> BudgetDecision {
    let spent: f64 = todays_entries
        .iter()
        .filter(|e| e.agent_id == policy.agent_id)
        .map(|e| e.total_cost)
        .sum();
    let tokens: i64 = todays_entries
        .iter()
        .filter(|e| e.agent_id == policy.agent_id)
        .map(|e| e.total_tokens)
        .sum();

    if spent >= policy.daily_cost_cap_usd {
        BudgetDecision::CostExceeded
    } else if tokens >= policy.daily_token_cap {
        BudgetDecision::TokenExceeded
    } else {
        BudgetDecision::WithinBudget
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EventKind, TokenUsage};

    #[test]
    fn test_calculate_cost() -> Result<(), Box<dyn std::error::Error>> {
        let mut providers = HashMap::new();
        let mut models = HashMap::new();
        models.insert(
            "gpt-4o".to_string(),
            ModelPrice {
                input_per_1m: 5.0,
                output_per_1m: 15.0,
            },
        );
        providers.insert("openai".to_string(), models);

        let catalog = PriceCatalog {
            catalog_version: "1.0".to_string(),
            currency: "USD".to_string(),
            providers,
        };

        let event = AgentObservationEvent {
            process_signal: None,
            event_id: "evt1".into(),
            tenant_id: "t1".into(),
            trace_id: "tr1".into(),
            agent_id: Some("agent1".into()),
            shadow_candidate_id: None,
            tool_id: None,
            resource_id: None,
            surface: "test".into(),
            action: "test".into(),
            pep_type: None,
            risk_level: None,
            timestamp: "2026-06-20T12:00:00Z".into(),
            payload_json: "{}".into(),
            token_usage: Some(TokenUsage {
                model: Some("gpt-4o".into()),
                input_tokens: Some(1_000_000),
                output_tokens: Some(2_000_000),
                total_tokens: Some(3_000_000),
            }),
            browser_scope: None,
            event_kind: EventKind::Generic,
            decision: None,
            tool_call: None,
            resource_access: None,
            latency_ms: None,
            provider: None,
        };

        let cost = calculate_cost(&event, "openai", &catalog).ok_or("should calculate cost")?;
        assert_eq!(cost.input_cost, 5.0);
        assert_eq!(cost.output_cost, 30.0);
        assert_eq!(cost.total_cost, 35.0);
        Ok(())
    }

    #[test]
    fn budget_checks_are_agent_scoped_for_multiple_browsers() {
        let entries = vec![
            CostLedgerEntry {
                event_id: "evt_chrome".into(),
                agent_id: "cand_chatgpt_chrome".into(),
                provider: "openai".into(),
                model: Some("gpt-4o".into()),
                input_tokens: 400,
                output_tokens: 200,
                total_tokens: 600,
                input_cost: 0.01,
                output_cost: 0.02,
                total_cost: 0.03,
                currency: "USD".into(),
                estimated: true,
                timestamp: "2026-06-26T00:00:00Z".into(),
            },
            CostLedgerEntry {
                event_id: "evt_edge".into(),
                agent_id: "cand_chatgpt_edge".into(),
                provider: "openai".into(),
                model: Some("gpt-4o".into()),
                input_tokens: 10_000,
                output_tokens: 10_000,
                total_tokens: 20_000,
                input_cost: 1.0,
                output_cost: 1.0,
                total_cost: 2.0,
                currency: "USD".into(),
                estimated: true,
                timestamp: "2026-06-26T00:00:00Z".into(),
            },
        ];

        let chrome_policy = BudgetPolicy {
            agent_id: "cand_chatgpt_chrome".into(),
            daily_cost_cap_usd: 1.0,
            daily_token_cap: 1_000,
        };
        let edge_policy = BudgetPolicy {
            agent_id: "cand_chatgpt_edge".into(),
            daily_cost_cap_usd: 1.0,
            daily_token_cap: 30_000,
        };

        assert_eq!(
            check_budget(&chrome_policy, &entries),
            BudgetDecision::WithinBudget
        );
        assert_eq!(
            check_budget(&edge_policy, &entries),
            BudgetDecision::CostExceeded
        );
    }
}
