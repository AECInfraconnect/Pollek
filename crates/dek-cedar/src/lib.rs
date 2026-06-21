// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::unwrap_used, clippy::expect_used)]
use anyhow::Result;
use async_trait::async_trait;
use cedar_policy::{
    Authorizer, Context, Decision, Entities, EntityId, EntityTypeName, EntityUid, PolicySet,
    Request,
};
use dek_plugin_sdk::{
    DecisionEffect, DecisionStatus, EvalRequest, PluginError, PluginIdentity, PluginResult,
    PluginType, PolicyDecision, PolicyEvaluator, DEK_PLUGIN_API_VERSION,
};
use moka::sync::Cache;
use std::str::FromStr;
use std::time::Duration;

pub struct CedarAdapter {
    policy_src: String,
    policy_set: PolicySet,
    cache: Cache<String, (PolicyDecision, std::time::Instant)>,
}

impl CedarAdapter {
    pub fn new(policy_src: &str) -> Result<Self> {
        let policy_set = PolicySet::from_str(policy_src)
            .map_err(|e| anyhow::anyhow!("Cedar Parse Error: {}", e))?;
        let cache = Cache::builder().max_capacity(10_000).build();
        Ok(Self {
            policy_src: policy_src.to_string(),
            policy_set,
            cache,
        })
    }
}

#[async_trait]
impl PolicyEvaluator for CedarAdapter {
    fn identity(&self) -> PluginIdentity {
        PluginIdentity {
            id: "cedar_native".into(),
            name: "Cedar Policy Evaluator".into(),
            version: "1.0.0".into(),
            vendor: "AEC Infraconnect".into(),
            plugin_type: PluginType::PolicyEvaluator,
            api_version: DEK_PLUGIN_API_VERSION.into(),
        }
    }

    async fn evaluate(&self, input: EvalRequest) -> PluginResult<PolicyDecision> {
        let risk = input
            .payload
            .get("risk_tier")
            .and_then(|v| v.as_str())
            .unwrap_or("low");
        let ttl = match risk {
            "high" | "critical" => Duration::from_secs(0),
            "medium" => Duration::from_secs(15),
            _ => Duration::from_secs(300),
        };

        let cache_key = serde_json::to_string(&input.payload).unwrap_or_default();
        if !ttl.is_zero() && !cache_key.is_empty() {
            if let Some((decision, ts)) = self.cache.get(&cache_key) {
                if ts.elapsed() <= ttl {
                    return Ok(decision);
                }
            }
        }

        let get_field = |key: &str| -> Option<String> {
            input.payload.get(key).and_then(|v| {
                if v.is_object() {
                    v.get("kind")
                        .or_else(|| v.get("id"))
                        .and_then(|val| val.as_str().map(|s| s.to_string()))
                } else if v.is_string() {
                    v.as_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
        };

        let principal = get_field("principal").unwrap_or_else(|| "User::\"unknown\"".to_string());
        let action = get_field("action").unwrap_or_else(|| "Action::\"unknown\"".to_string());
        let resource = get_field("resource").unwrap_or_else(|| "Resource::\"unknown\"".to_string());

        tracing::info!(
            "Evaluating Cedar Policy:\n{}\nInput: principal={}, action={}, resource={}",
            self.policy_src,
            principal,
            action,
            resource
        );

        let context = match input.payload.get("context") {
            Some(ctx_val) => Context::from_json_value(ctx_val.clone(), None)
                .map_err(|e| PluginError::Invalid(format!("Context parse error: {}", e)))?,
            None => Context::empty(),
        };

        let entities = match input.payload.get("entities") {
            Some(ent_val) => Entities::from_json_value(ent_val.clone(), None)
                .map_err(|e| PluginError::Invalid(format!("Entities parse error: {}", e)))?,
            None => Entities::empty(),
        };

        let make_uid = |type_name: &str, id: &str| -> std::result::Result<EntityUid, PluginError> {
            if id.contains("::") {
                EntityUid::from_str(id)
                    .map_err(|e| PluginError::Invalid(format!("EntityUid parse error: {}", e)))
            } else {
                Ok(EntityUid::from_type_name_and_id(
                    EntityTypeName::from_str(type_name).map_err(|e| {
                        PluginError::Invalid(format!("EntityTypeName parse error: {}", e))
                    })?,
                    EntityId::from_str(id).map_err(|e| {
                        PluginError::Invalid(format!("EntityId parse error: {}", e))
                    })?,
                ))
            }
        };

        let principal_uid = make_uid("User", &principal)?;
        let action_uid = make_uid("Action", &action)?;
        let resource_uid = make_uid("Resource", &resource)?;

        let request = Request::new(principal_uid, action_uid, resource_uid, context, None)
            .map_err(|e| PluginError::Execution(format!("Cedar Request Error: {}", e)))?;

        let authorizer = Authorizer::new();
        let answer = authorizer.is_authorized(&request, &self.policy_set, &entities);

        let allowed = answer.decision() == Decision::Allow;

        let mut obligations = vec![];
        for reason in answer.diagnostics().reason() {
            if let Some(policy) = self.policy_set.policy(reason) {
                if let Some(obs) = policy.annotation("obligations") {
                    let text = obs.to_string();
                    let text = text.trim_matches('"');
                    obligations.push(text.to_string());
                }
            }
        }

        let decision_res = PolicyDecision {
            evaluator_id: "cedar_native".to_string(),
            evaluator_type: "local_pdp".to_string(),
            required: true,
            status: DecisionStatus::Success,
            decision: if allowed {
                DecisionEffect::Allow
            } else {
                DecisionEffect::Deny
            },
            reason: if allowed {
                "Allowed by Cedar policy".to_string()
            } else {
                "Denied by Cedar policy".to_string()
            },
            effects: serde_json::json!({}),
            obligations,
            metadata: serde_json::json!({ "policy_version": "1.0", "diagnostics": format!("{:?}", answer.diagnostics()) }),
        };

        if !ttl.is_zero() && !cache_key.is_empty() {
            self.cache
                .insert(cache_key, (decision_res.clone(), std::time::Instant::now()));
        }

        Ok(decision_res)
    }

    async fn clear_cache(&self) -> PluginResult<()> {
        self.cache.invalidate_all();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use serde_json::json;

    #[test]
    fn test_cedar_json_parse() -> anyhow::Result<()> {
        let ctx_val = json!({ "ip": "127.0.0.1" });
        let _ctx = Context::from_json_value(ctx_val, None)?;

        let ent_val = serde_json::json!([]);
        let _ents = Entities::from_json_value(ent_val, None)?;

        Ok(())
    }

    #[tokio::test]
    async fn test_cache_ttl_logic() {
        let policy = "permit(principal, action, resource);";
        let adapter = CedarAdapter::new(policy).unwrap();

        let high_risk = json!({ "principal": "User::\"u\"", "action": "Action::\"a\"", "resource": "Resource::\"r\"", "risk_tier": "high" });
        let low_risk = json!({ "principal": "User::\"u\"", "action": "Action::\"a\"", "resource": "Resource::\"r\"", "risk_tier": "low" });

        let req_high = EvalRequest {
            request_id: "1".into(),
            tenant_id: None,
            subject: None,
            action: None,
            resource: None,
            payload: high_risk.clone(),
            context: std::collections::BTreeMap::new(),
        };
        let req_low = EvalRequest {
            request_id: "2".into(),
            tenant_id: None,
            subject: None,
            action: None,
            resource: None,
            payload: low_risk.clone(),
            context: std::collections::BTreeMap::new(),
        };

        // Evaluate high risk: should not cache
        let _ = adapter.evaluate(req_high).await.unwrap();
        let cache_key_high = serde_json::to_string(&high_risk).unwrap();
        assert!(
            adapter.cache.get(&cache_key_high).is_none(),
            "High risk should not be cached"
        );

        // Evaluate low risk: should cache
        let _ = adapter.evaluate(req_low).await.unwrap();
        let cache_key_low = serde_json::to_string(&low_risk).unwrap();
        assert!(
            adapter.cache.get(&cache_key_low).is_some(),
            "Low risk should be cached"
        );

        // Clear cache
        adapter.clear_cache().await.unwrap();
        assert!(
            adapter.cache.get(&cache_key_low).is_none(),
            "Cache should be cleared"
        );
    }
}
