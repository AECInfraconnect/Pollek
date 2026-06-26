use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiBudgetLimit {
    pub schema_version: String,
    pub budget_id: String,
    pub tenant_id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub window: String,
    pub currency: String,
    pub soft_cost_limit: Option<f64>,
    pub hard_cost_limit: Option<f64>,
    pub soft_token_limit: Option<i64>,
    pub hard_token_limit: Option<i64>,
    pub action_on_soft: String,
    pub action_on_hard: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageWindowTotals {
    pub cost: f64,
    pub tokens: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetAction {
    Allow,
    Warn { threshold: f64 },
    RequireApproval { reason: String },
    Throttle { reason: String },
    Deny { reason: String },
    DegradeModel { preferred_model: String },
    DisableTool { tool_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetEvaluation {
    pub budget_id: String,
    pub action: BudgetAction,
    pub current_cost: f64,
    pub current_tokens: i64,
    pub projected_cost: f64,
    pub projected_tokens: i64,
}

pub fn evaluate_budget(
    policy: &AiBudgetLimit,
    current: UsageWindowTotals,
    projected: UsageWindowTotals,
) -> BudgetEvaluation {
    let projected_cost = current.cost + projected.cost;
    let projected_tokens = current.tokens + projected.tokens;
    let hard_exceeded = policy
        .hard_cost_limit
        .map(|limit| projected_cost >= limit)
        .unwrap_or(false)
        || policy
            .hard_token_limit
            .map(|limit| projected_tokens >= limit)
            .unwrap_or(false);
    let soft_exceeded = policy
        .soft_cost_limit
        .map(|limit| projected_cost >= limit)
        .unwrap_or(false)
        || policy
            .soft_token_limit
            .map(|limit| projected_tokens >= limit)
            .unwrap_or(false);
    let action = if !policy.enabled {
        BudgetAction::Allow
    } else if hard_exceeded {
        hard_action(policy)
    } else if soft_exceeded {
        soft_action(policy)
    } else {
        BudgetAction::Allow
    };

    BudgetEvaluation {
        budget_id: policy.budget_id.clone(),
        action,
        current_cost: current.cost,
        current_tokens: current.tokens,
        projected_cost,
        projected_tokens,
    }
}

fn soft_action(policy: &AiBudgetLimit) -> BudgetAction {
    match policy.action_on_soft.as_str() {
        "approval" | "require_approval" => BudgetAction::RequireApproval {
            reason: "soft budget threshold requires approval".to_string(),
        },
        "throttle" => BudgetAction::Throttle {
            reason: "soft budget threshold reached".to_string(),
        },
        _ => BudgetAction::Warn { threshold: 1.0 },
    }
}

fn hard_action(policy: &AiBudgetLimit) -> BudgetAction {
    match policy.action_on_hard.as_str() {
        "approval" | "require_approval" => BudgetAction::RequireApproval {
            reason: "hard budget threshold requires approval".to_string(),
        },
        "throttle" => BudgetAction::Throttle {
            reason: "hard budget threshold reached".to_string(),
        },
        "degrade_model" => BudgetAction::DegradeModel {
            preferred_model: "policy-selected-lower-cost-model".to_string(),
        },
        _ => BudgetAction::Deny {
            reason: "hard budget threshold exceeded".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> AiBudgetLimit {
        AiBudgetLimit {
            schema_version: "ai-budget-limit.v1".to_string(),
            budget_id: "budget_1".to_string(),
            tenant_id: "local".to_string(),
            scope_type: "agent".to_string(),
            scope_id: "agent_1".to_string(),
            window: "day".to_string(),
            currency: "USD".to_string(),
            soft_cost_limit: Some(5.0),
            hard_cost_limit: Some(10.0),
            soft_token_limit: Some(50_000),
            hard_token_limit: Some(100_000),
            action_on_soft: "warn".to_string(),
            action_on_hard: "deny".to_string(),
            enabled: true,
            created_at: "2026-06-26T00:00:00Z".to_string(),
            updated_at: "2026-06-26T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn warns_on_soft_limit() {
        let evaluation = evaluate_budget(
            &policy(),
            UsageWindowTotals {
                cost: 4.0,
                tokens: 10,
            },
            UsageWindowTotals {
                cost: 1.5,
                tokens: 0,
            },
        );

        assert!(matches!(evaluation.action, BudgetAction::Warn { .. }));
    }

    #[test]
    fn denies_on_hard_limit() {
        let evaluation = evaluate_budget(
            &policy(),
            UsageWindowTotals {
                cost: 9.0,
                tokens: 10,
            },
            UsageWindowTotals {
                cost: 1.5,
                tokens: 0,
            },
        );

        assert!(matches!(evaluation.action, BudgetAction::Deny { .. }));
    }
}
