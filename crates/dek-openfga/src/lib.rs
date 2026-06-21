// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

#![warn(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::unwrap_used, clippy::expect_used)]

use async_trait::async_trait;
use dek_config::MtlsConfig;
use dek_plugin_sdk::{
    DecisionEffect, DecisionStatus, EvalRequest, PluginError, PluginIdentity, PluginResult,
    PluginType, PolicyDecision, PolicyEvaluator, DEK_PLUGIN_API_VERSION,
};
use moka::future::Cache;
use reqwest::Client;
use serde_json::json;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

const CACHE_CAPACITY: u64 = 10_000;

#[derive(Debug, thiserror::Error)]
pub enum OpenFgaError {
    #[error("connection failed: {0}")]
    Connection(String),
    #[error("invalid model: {0}")]
    Model(String),
    #[error("evaluation failed: {0}")]
    Eval(String),
}

pub struct OpenFgaAdapter {
    endpoint: String,
    store_id: String,
    client: Client,
    cache: Cache<String, (bool, Instant)>,
}

impl OpenFgaAdapter {
    pub fn new(
        endpoint: &str,
        store_id: &str,
        mtls: Option<&MtlsConfig>,
    ) -> Result<Self, OpenFgaError> {
        let client = if let Some(m) = mtls {
            m.build_client(None)
                .map_err(|e| OpenFgaError::Connection(e.to_string()))?
        } else {
            Client::new()
        };
        let cache = Cache::builder().max_capacity(CACHE_CAPACITY).build();
        Ok(Self {
            endpoint: endpoint.to_string(),
            store_id: store_id.to_string(),
            client,
            cache,
        })
    }

    fn decision(&self, allow: bool, reason: &str) -> PolicyDecision {
        PolicyDecision {
            evaluator_id: "openfga_remote".to_string(),
            evaluator_type: "remote_pdp".to_string(),
            required: true,
            status: DecisionStatus::Success,
            decision: if allow {
                DecisionEffect::Allow
            } else {
                DecisionEffect::Deny
            },
            reason: reason.to_string(),
            effects: json!({}),
            obligations: vec![],
            metadata: json!({ "store_id": self.store_id }),
        }
    }
}

#[async_trait]
impl PolicyEvaluator for OpenFgaAdapter {
    fn identity(&self) -> PluginIdentity {
        PluginIdentity {
            id: "openfga_remote".into(),
            name: "OpenFGA Policy Evaluator".into(),
            version: "1.1.0".into(),
            vendor: "AEC Infraconnect".into(),
            plugin_type: PluginType::PolicyEvaluator,
            api_version: DEK_PLUGIN_API_VERSION.into(),
        }
    }

    async fn evaluate(&self, input: EvalRequest) -> PluginResult<PolicyDecision> {
        let principal = input
            .payload
            .get("principal")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let action = input
            .payload
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let resource = input
            .payload
            .get("resource")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let risk_tier = input
            .payload
            .get("risk_tier")
            .and_then(|v| v.as_str())
            .unwrap_or("low");

        let ttl_secs = match risk_tier {
            "high" | "critical" => 1,
            "medium" => 30,
            _ => 300,
        };
        let ttl = Duration::from_secs(ttl_secs);

        let cache_key = format!("{principal}|{action}|{resource}");

        if let Some((allow, ts)) = self.cache.get(&cache_key).await {
            if ts.elapsed() <= ttl {
                debug!(%principal, %action, %resource, allow, "openfga cache hit");
                let reason = if allow {
                    "OpenFGA (cached) allowed"
                } else {
                    "OpenFGA (cached) denied"
                };
                return Ok(self.decision(allow, reason));
            } else {
                self.cache.remove(&cache_key).await;
            }
        }

        let url = format!("{}/stores/{}/check", self.endpoint, self.store_id);
        let payload = json!({
            "tuple_key": { "user": principal, "relation": action, "object": resource }
        });
        debug!(%url, %principal, %action, %resource, "openfga check (cache miss)");

        let res = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| PluginError::Unavailable(format!("OpenFGA connect failed: {e}")))?;

        if !res.status().is_success() {
            let status = res.status();
            warn!(%status, "openfga returned non-success status");
            return Err(PluginError::Unavailable(format!("OpenFGA HTTP {status}")));
        }

        let body = res
            .json::<serde_json::Value>()
            .await
            .map_err(|e| PluginError::Execution(format!("parse OpenFGA response: {e}")))?;

        let allow = body
            .get("allowed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        self.cache.insert(cache_key, (allow, Instant::now())).await;

        let reason = if allow {
            "OpenFGA remote check allowed"
        } else {
            "OpenFGA remote check denied"
        };
        Ok(self.decision(allow, reason))
    }

    async fn clear_cache(&self) -> PluginResult<()> {
        self.cache.invalidate_all();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn test_openfga_new_no_mtls() {
        let a = OpenFgaAdapter::new("http://localhost:8080", "store_1", None).unwrap();
        assert_eq!(a.endpoint, "http://localhost:8080");
        assert_eq!(a.store_id, "store_1");
    }

    #[tokio::test]
    async fn test_unreachable_is_typed_unavailable() {
        let a = OpenFgaAdapter::new("http://127.0.0.1:0", "store_1", None).unwrap();
        let input = json!({ "principal": "u", "action": "read", "resource": "doc" });
        let req = EvalRequest {
            request_id: "req1".into(),
            tenant_id: None,
            subject: None,
            action: None,
            resource: None,
            payload: input,
            context: std::collections::BTreeMap::new(),
        };
        let err = a.evaluate(req).await.unwrap_err();
        assert!(matches!(err, PluginError::Unavailable(_)));
    }
}
