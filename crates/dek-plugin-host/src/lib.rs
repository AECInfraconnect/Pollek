// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

pub mod manifest;

use dek_plugin_sdk::{EvalRequest, PluginError, PolicyEvaluator, TransformPlugin};
use dek_policy_router::{PolicyRouter, Route};
use dek_policy_runtime::{
    PolicyDecision as OldPolicyDecision, PolicyError, PolicyResult, PolicyRuntime,
};
use std::collections::HashMap;
use std::sync::Arc;

pub struct PluginHost {
    router: PolicyRouter,
    policy_evaluators: HashMap<String, Arc<dyn PolicyEvaluator>>,
    transform_plugins: HashMap<String, Arc<dyn TransformPlugin>>,
}

pub struct EvaluatorAdapter {
    evaluator: Arc<dyn PolicyEvaluator>,
}

impl EvaluatorAdapter {
    pub fn new(evaluator: Arc<dyn PolicyEvaluator>) -> Self {
        Self { evaluator }
    }
}

#[async_trait::async_trait]
impl PolicyRuntime for EvaluatorAdapter {
    async fn evaluate(&self, input: Arc<serde_json::Value>) -> PolicyResult {
        let request_id = input
            .get("request_id")
            .and_then(|v| v.as_str())
            .unwrap_or("auto-req")
            .to_string();
        let tenant_id = input
            .get("tenant_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let subject = input
            .get("principal")
            .and_then(|v| v.get("id").or(Some(v)))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let resource = input.get("resource").and_then(|v| {
            if v.is_object() {
                v.get("id")
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
            } else {
                v.as_str().map(|s| s.to_string())
            }
        });

        let req = EvalRequest {
            request_id,
            tenant_id,
            subject,
            action,
            resource,
            payload: (*input).clone(),
            context: Default::default(),
        };

        match self.evaluator.evaluate(req).await {
            Ok(decision) => {
                let is_allow = decision.is_allow();
                Ok(OldPolicyDecision {
                    evaluator_id: decision.evaluator_id,
                    evaluator_type: decision.evaluator_type,
                    required: decision.required,
                    status: "success".into(),
                    decision: if is_allow {
                        "allow".into()
                    } else {
                        "deny".into()
                    },
                    allow: is_allow,
                    reason: decision.reason,
                    effects: decision.effects,
                    obligations: decision.obligations,
                    metadata: decision.metadata,
                    explanation: None,
                    user_action_required: false,
                    user_action_th: None,
                })
            }
            Err(PluginError::Unavailable(msg)) => Err(PolicyError::Unavailable(msg)),
            Err(e) => Err(PolicyError::Eval(e.to_string())),
        }
    }

    fn version(&self) -> String {
        self.evaluator.identity().version.clone()
    }
}

impl PluginHost {
    pub fn new() -> Self {
        Self {
            router: PolicyRouter::new(),
            policy_evaluators: HashMap::new(),
            transform_plugins: HashMap::new(),
        }
    }

    pub fn set_routes(&mut self, routes: Vec<Route>) {
        self.router.set_routes(routes);
    }

    pub fn register_evaluator(&mut self, evaluator: Arc<dyn PolicyEvaluator>) {
        let id = evaluator.identity().id.clone();
        self.policy_evaluators.insert(id.clone(), evaluator.clone());

        let adapter = EvaluatorAdapter::new(evaluator);
        self.router.register_evaluator(&id, Arc::new(adapter));
    }

    pub fn register_transform(&mut self, transform: Arc<dyn TransformPlugin>) {
        let id = transform.identity().id.clone();
        self.transform_plugins.insert(id, transform);
    }

    pub async fn authorize(&self, payload: serde_json::Value) -> anyhow::Result<OldPolicyDecision> {
        self.router.authorize(payload).await
    }
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
pub trait PollekPlugin: Send + Sync {
    fn id(&self) -> &str;
    fn permissions(&self) -> &[String];
    async fn call(
        &self,
        operation: &str,
        input: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value>;
}

pub async fn call_plugin_checked(
    plugin: &dyn PollekPlugin,
    operation: &str,
    input: serde_json::Value,
    policy: &dek_plugin_sdk::PluginPolicy,
) -> anyhow::Result<serde_json::Value> {
    policy.ensure_allowed(plugin.id(), operation, plugin.permissions())?;

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(policy.timeout_ms(plugin.id())),
        plugin.call(operation, input),
    )
    .await
    .map_err(|_| anyhow::anyhow!("plugin call timeout"))??;

    policy.validate_output(plugin.id(), operation, &result)?;
    Ok(result)
}
