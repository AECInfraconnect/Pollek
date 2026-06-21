#![warn(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::unwrap_used, clippy::expect_used)]
//! dek-openfga — OpenFGA remote PDP adapter (P3 hardened).
//!
//! Changes vs. previous version:
//!  - println! -> tracing (B5 / P3 #8): hot-path logging now structured + leveled
//!  - typed errors (P3 #7): connection failure => Err(PolicyError::Unavailable)
//!    so the router fails CLOSED explicitly instead of folding into the decision
//!  - decision cache (P3 #9): short-TTL moka cache keyed by (user,relation,object)
//!    cuts an HTTP round-trip on hot tuples; invalidated by `clear_cache()` on
//!    bundle reload.
//!
//! Cargo.toml additions:
//!   moka = { version = "0.12", features = ["future"] }
//!   # thiserror comes transitively via dek-policy-runtime::PolicyError

use async_trait::async_trait;
use dek_config::MtlsConfig;
use dek_policy_runtime::{PolicyDecision, PolicyError, PolicyResult, PolicyRuntime};
use moka::future::Cache;
use reqwest::Client;
use serde_json::json;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Maximum cached entries
const CACHE_CAPACITY: u64 = 10_000;

pub struct OpenFgaAdapter {
    endpoint: String,
    store_id: String,
    client: Client,
    /// (user|relation|object) -> (allow, timestamp)
    cache: Cache<String, (bool, Instant)>,
}

impl OpenFgaAdapter {
    pub fn new(endpoint: &str, store_id: &str, mtls: Option<&MtlsConfig>) -> anyhow::Result<Self> {
        let client = if let Some(m) = mtls {
            m.build_client(None)?
        } else {
            Client::new()
        };
        let cache = Cache::builder()
            .max_capacity(CACHE_CAPACITY)
            .build();
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
            status: "success".to_string(),
            decision: if allow { "allow" } else { "deny" }.to_string(),
            allow,
            reason: reason.to_string(),
            effects: json!({}),
            obligations: vec![],
            metadata: json!({ "store_id": self.store_id }),
        }
    }
}

#[async_trait]
impl PolicyRuntime for OpenFgaAdapter {
    async fn evaluate(&self, input: serde_json::Value) -> PolicyResult {
        let principal = input
            .get("principal")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let resource = input
            .get("resource")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let risk_tier = input
            .get("risk_tier")
            .and_then(|v| v.as_str())
            .unwrap_or("low");

        // Dynamic TTL based on risk_tier
        let ttl_secs = match risk_tier {
            "high" | "critical" => 1, // very short cache for high risk
            "medium" => 30,
            _ => 300, // low risk
        };
        let ttl = Duration::from_secs(ttl_secs);

        let cache_key = format!("{principal}|{action}|{resource}");

        // 1) Cache hit -> check TTL -> no network round-trip.
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
                // Expired
                self.cache.remove(&cache_key).await;
            }
        }

        // 2) Miss -> query OpenFGA.
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
            .map_err(|e| PolicyError::Unavailable(format!("OpenFGA connect failed: {e}")))?;

        if !res.status().is_success() {
            let status = res.status();
            warn!(%status, "openfga returned non-success status");
            return Err(PolicyError::Unavailable(format!("OpenFGA HTTP {status}")));
        }

        let body = res
            .json::<serde_json::Value>()
            .await
            .map_err(|e| PolicyError::Eval(format!("parse OpenFGA response: {e}")))?;

        let allow = body
            .get("allowed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // 3) Cache the verdict (both allow and deny).
        self.cache.insert(cache_key, (allow, Instant::now())).await;

        let reason = if allow {
            "OpenFGA remote check allowed"
        } else {
            "OpenFGA remote check denied"
        };
        Ok(self.decision(allow, reason))
    }

    fn version(&self) -> String {
        "openfga-v1.1.0".to_string()
    }

    async fn clear_cache(&self) {
        self.cache.invalidate_all();
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
        // Was previously folded into Ok(decision{status:"error"}); now a typed error.
        let a = OpenFgaAdapter::new("http://127.0.0.1:0", "store_1", None).unwrap();
        let input = json!({ "principal": "u", "action": "read", "resource": "doc" });
        let err = a.evaluate(input).await.unwrap_err();
        assert!(matches!(err, PolicyError::Unavailable(_)));
    }
}
