use serde::{Deserialize, Serialize};
use crate::capability::CapabilityDescriptor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnforcementHooks {
    pub agent_signature_id: String,
    pub default_policy_refs: Vec<String>,
    pub tool_guards: Vec<ToolGuard>,
    pub resource_limits: ResourceLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolGuard {
    pub tool_name: String,
    pub guard: GuardKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardKind {
    Allow,
    Deny,
    RequireApproval,
    RedactParams { fields: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_calls_per_min: Option<u32>,
    pub daily_cost_cap_usd: Option<f64>,
    pub daily_token_cap: Option<i64>,
}

pub fn derive_enforcement(cap: &CapabilityDescriptor) -> EnforcementHooks {
    let tool_guards = cap.tool_capabilities.iter().map(|t| ToolGuard {
        tool_name: t.tool_name.clone(),
        guard: match t.risk_class.as_str() {
            "delete" | "exec" | "write" => GuardKind::RequireApproval,
            _ => GuardKind::Allow,
        },
    }).collect();

    let has_critical = cap.data_reach.iter().any(|d| d.sensitivity == "critical");
    
    EnforcementHooks {
        agent_signature_id: cap.agent_signature_id.clone(),
        default_policy_refs: vec!["pollen.baseline.agent".into()],
        tool_guards,
        resource_limits: ResourceLimits {
            max_calls_per_min: Some(if has_critical { 30 } else { 120 }),
            daily_cost_cap_usd: Some(50.0),
            daily_token_cap: Some(2_000_000),
        },
    }
}
