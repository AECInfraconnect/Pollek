// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEnforcementRequest {
    pub request_id: String,
    pub trace_id: String,
    pub tenant_id: String,
    pub workspace_id: String,
    pub environment_id: String,
    pub agent_id: Option<String>,
    pub principal: PrincipalRef,
    pub action: ActionRef,
    pub resource: ResourceRef,
    pub tool: Option<ToolRef>,
    pub input: serde_json::Value,
    pub context: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrincipalRef {
    pub entity_type: String,
    pub entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRef {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRef {
    pub resource_type: String,
    pub resource_id: String,
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRef {
    pub tool_id: String,
    pub mcp_server_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecisionEffect {
    Allow,
    Deny,
    Redact,
    Mask,
    Warn,
    RequireApproval,
    BreakGlassAllow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterDecisionResult {
    pub adapter_id: String,
    pub decision: DecisionEffect,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionResult {
    pub request_id: String,
    pub trace_id: String,
    pub decision: DecisionEffect,
    pub reason: String,
    pub matched_policy_ids: Vec<String>,
    pub matched_route_id: String,
    pub adapter_results: Vec<AdapterDecisionResult>,
    pub obligations: Vec<serde_json::Value>,
    pub latency_ms: u64,
    pub selected_engine: Option<String>,
    pub enforcement_plane: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRoute {
    pub route_id: String,
    pub pdp_required: Vec<String>,
}

#[async_trait::async_trait]
pub trait PolicyAdapter: Send + Sync {
    fn adapter_id(&self) -> &'static str;
    async fn evaluate(
        &self,
        request: &NormalizedEnforcementRequest,
    ) -> anyhow::Result<AdapterDecisionResult>;
}

pub struct PolicyRouter {
    adapters: HashMap<String, Box<dyn PolicyAdapter>>,
    _routes: Vec<PolicyRoute>,
}

impl PolicyRouter {
    pub fn new(
        adapters: HashMap<String, Box<dyn PolicyAdapter>>,
        routes: Vec<PolicyRoute>,
    ) -> Self {
        Self {
            adapters,
            _routes: routes,
        }
    }

    pub async fn evaluate(&self, request: &NormalizedEnforcementRequest) -> DecisionResult {
        let mut adapter_results = Vec::new();
        let mut final_decision = DecisionEffect::Allow;
        let mut final_reason = String::new();
        let matched_route_id = String::from("default_allow");

        // Simple evaluation logic: AND across all required adapters in the first matched route.
        // For demonstration, we just evaluate all adapters.
        for adapter in self.adapters.values() {
            if let Ok(res) = adapter.evaluate(request).await {
                if matches!(res.decision, DecisionEffect::Deny) {
                    final_decision = DecisionEffect::Deny;
                    final_reason = res.reason.clone();
                }
                adapter_results.push(res);
            }
        }

        DecisionResult {
            request_id: request.request_id.clone(),
            trace_id: request.trace_id.clone(),
            decision: final_decision,
            reason: final_reason,
            matched_policy_ids: adapter_results
                .iter()
                .map(|r| r.adapter_id.clone())
                .collect(),
            matched_route_id,
            adapter_results,
            obligations: vec![],
            latency_ms: 1,
            selected_engine: None,
            enforcement_plane: None,
        }
    }
}

pub struct PolicyRouterHandle {
    current: arc_swap::ArcSwap<PolicyRouter>,
}

impl PolicyRouterHandle {
    pub fn new(router: PolicyRouter) -> Self {
        Self {
            current: arc_swap::ArcSwap::from_pointee(router),
        }
    }

    pub fn get(&self) -> arc_swap::Guard<std::sync::Arc<PolicyRouter>> {
        self.current.load()
    }

    pub fn swap(&self, router: PolicyRouter) {
        self.current.store(std::sync::Arc::new(router));
    }
}

// -----------------------------------------------------------------------------
// Legacy structures
// -----------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Principal {
    pub id: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRequest {
    pub request_id: String,
    pub trace_id: Option<String>,
    pub tenant_id: String,
    pub device_id: String,
    pub principal: Principal,
    pub agent: Option<AgentIdentity>,
    pub action: String,
    pub resource: ResourceRef,
    pub context: serde_json::Value,
    pub input_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Obligation {
    pub kind: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorResult {
    pub evaluator_id: String,
    pub allow: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionResponse {
    pub decision_id: String,
    pub allow: bool,
    pub reason_code: String,
    pub reason: String,
    pub obligations: Vec<Obligation>,
    pub effects: serde_json::Value,
    pub policy_bundle_id: String,
    pub policy_bundle_version: String,
    pub evaluator_results: Vec<EvaluatorResult>,
    pub latency_ms: u64,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
