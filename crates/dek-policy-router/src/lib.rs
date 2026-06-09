#![warn(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::unwrap_used, clippy::expect_used)]
use anyhow::Result;
use dek_policy_runtime::{PolicyDecision, PolicyRuntime};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EnforcementMode {
    #[default]
    Standard, // Standard evaluation
    FailClosed, // If evaluator error or missing, deny
    BreakGlass, // Bypass evaluation for emergency, always allow
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FailoverStrategy {
    #[default]
    Priority,
    HealthBased,
    RoundRobin,
    LeastLatency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub id: String,
    pub priority: i32,
    #[serde(default)]
    pub enforcement_mode: EnforcementMode,
    pub match_rule: EnterpriseMatchRule,
    #[serde(default)]
    pub pdp_required: Vec<String>,
    #[serde(default)]
    pub pdp_pool: Vec<String>,
    #[serde(default)]
    pub failover_strategy: FailoverStrategy,
    #[serde(default)]
    pub pdp_conditional: Vec<ConditionalPdp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnterpriseMatchRule {
    pub method: Option<String>,
    pub tool_category: Option<String>,
    pub resource_type: Option<String>,
    pub severity_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalPdp {
    pub evaluator: String,
    pub required_payload_key: String, // Mock condition evaluation
}

use dek_resilience::breaker::{CircuitBreaker, CircuitConfig, Admit};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use dek_errors::lock_ext::LockExt;

pub struct PdpStats {
    pub ewma_latency: f64,
    pub successes: u64,
    pub failures: u64,
}

impl Default for PdpStats {
    fn default() -> Self {
        Self::new()
    }
}

impl PdpStats {
    pub fn new() -> Self {
        Self {
            ewma_latency: 0.0,
            successes: 0,
            failures: 0,
        }
    }
    
    pub fn record_latency(&mut self, latency: f64, alpha: f64) {
        if self.ewma_latency == 0.0 {
            self.ewma_latency = latency;
        } else {
            self.ewma_latency = alpha * latency + (1.0 - alpha) * self.ewma_latency;
        }
    }

    pub fn health_score(&self) -> f64 {
        let total = self.successes + self.failures;
        if total == 0 {
            return 1.0;
        }
        self.successes as f64 / total as f64
    }
}

pub struct PolicyRouter {
    routes: Vec<Route>,
    evaluators: HashMap<String, Box<dyn PolicyRuntime>>,
    breakers: HashMap<String, Arc<CircuitBreaker>>,
    stats: HashMap<String, Arc<Mutex<PdpStats>>>,
    round_robin_counter: AtomicUsize,
    pdp_timeout_ms: u64,
    circuit_config: CircuitConfig,
}

impl PolicyRouter {
    pub fn new() -> Self {
        Self {
            routes: vec![],
            evaluators: HashMap::new(),
            breakers: HashMap::new(),
            stats: HashMap::new(),
            round_robin_counter: AtomicUsize::new(0),
            pdp_timeout_ms: 200,
            circuit_config: CircuitConfig::default(),
        }
    }

    pub fn set_scale_config(&mut self, pdp_timeout_ms: u64, failure_threshold: u32, cooldown_secs: u64) {
        self.pdp_timeout_ms = pdp_timeout_ms;
        self.circuit_config = CircuitConfig {
            failure_threshold,
            cooldown: std::time::Duration::from_secs(cooldown_secs),
            half_open_required_successes: 2,
        };
    }

    pub fn register_evaluator(&mut self, id: &str, evaluator: Box<dyn PolicyRuntime>) {
        self.evaluators.insert(id.to_string(), evaluator);
        self.breakers.insert(
            id.to_string(),
            Arc::new(CircuitBreaker::new(id, self.circuit_config.clone())),
        );
        self.stats.insert(id.to_string(), Arc::new(Mutex::new(PdpStats::new())));
    }

    pub fn set_routes(&mut self, mut routes: Vec<Route>) {
        routes.sort_by_key(|b| std::cmp::Reverse(b.priority)); // Highest priority first
        self.routes = routes;
    }

    pub async fn clear_caches(&self) {
        for evaluator in self.evaluators.values() {
            evaluator.clear_cache().await;
        }
    }

    fn select_pdp_from_pool(&self, pool: &[String], strategy: &FailoverStrategy) -> Option<String> {
        if pool.is_empty() {
            return None;
        }
        let available: Vec<&String> = pool.iter()
            .filter(|p| {
                if let Some(b) = self.breakers.get(*p) {
                    matches!(b.permitted(), Admit::Allow)
                } else {
                    false
                }
            }).collect();

        if available.is_empty() {
            return Some(pool[0].clone());
        }

        match strategy {
            FailoverStrategy::Priority => Some(available[0].clone()),
            FailoverStrategy::RoundRobin => {
                let idx = self.round_robin_counter.fetch_add(1, Ordering::Relaxed);
                Some(available[idx % available.len()].clone())
            }
            FailoverStrategy::HealthBased => {
                let mut best = available[0];
                let mut best_score = -1.0;
                for p in &available {
                    if let Some(stats) = self.stats.get(*p) {
                        let score = stats.lock_safe().health_score();
                        if score > best_score {
                            best_score = score;
                            best = p;
                        }
                    }
                }
                Some(best.clone())
            }
            FailoverStrategy::LeastLatency => {
                let mut best = available[0];
                let mut min_lat = f64::MAX;
                for p in &available {
                    if let Some(stats) = self.stats.get(*p) {
                        let lat = stats.lock_safe().ewma_latency;
                        if lat == 0.0 {
                            return Some((*p).clone());
                        }
                        if lat < min_lat {
                            min_lat = lat;
                            best = p;
                        }
                    }
                }
                Some(best.clone())
            }
        }
    }

    pub async fn authorize(&self, payload: serde_json::Value) -> Result<PolicyDecision> {
        // Support both old nested schema and new NormalizedMcpEvent schema
        let method = payload
            .get("request_type")
            .and_then(|v| v.as_str())
            .or_else(|| {
                payload
                    .get("mcp")
                    .and_then(|mcp| mcp.get("method"))
                    .and_then(|v| v.as_str())
            })
            .or_else(|| {
                payload
                    .get("action")
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("");

        // Extract optional matching context from payload
        let tool_category = payload
            .get("mcp")
            .and_then(|mcp| mcp.get("category"))
            .and_then(|v| v.as_str());
        let resource_type = payload.get("resource").and_then(|v| {
            if v.is_object() {
                v.get("kind").and_then(|k| k.as_str())
            } else {
                v.as_str()
            }
        });
        let severity_level = payload.get("severity").and_then(|v| v.as_str());

        let mut matched_route = None;
        for route in &self.routes {
            let mut matches = true;

            if let Some(ref m) = route.match_rule.method {
                if m != "*" && m != method {
                    matches = false;
                }
            }
            if let Some(ref cat) = route.match_rule.tool_category {
                if Some(cat.as_str()) != tool_category && cat != "*" {
                    matches = false;
                }
            }
            if let Some(ref res) = route.match_rule.resource_type {
                if Some(res.as_str()) != resource_type && res != "*" {
                    matches = false;
                }
            }
            if let Some(ref sev) = route.match_rule.severity_level {
                if Some(sev.as_str()) != severity_level && sev != "*" {
                    matches = false;
                }
            }

            if matches {
                matched_route = Some(route);
                break;
            }
        }

        let route = match matched_route {
            Some(r) => r,
            None => {
                return Ok(PolicyDecision {
                    evaluator_id: "router_default".into(),
                    evaluator_type: "router".into(),
                    required: true,
                    status: "success".into(),
                    decision: "deny".into(),
                    allow: false,
                    reason: "no matching route".into(),
                    effects: serde_json::json!({}),
                    obligations: vec![],
                    metadata: serde_json::json!({}),
                })
            }
        };

        tracing::info!(
            "== Adaptive Routing: Matched Route '{}' (Mode: {:?}) ==",
            route.id,
            route.enforcement_mode
        );

        if route.enforcement_mode == EnforcementMode::BreakGlass {
            tracing::warn!(
                "BREAK-GLASS MODE ACTIVATED for route {}: bypassing all evaluations",
                route.id
            );
            return Ok(PolicyDecision {
                evaluator_id: "router_breakglass".into(),
                evaluator_type: "router".into(),
                required: false,
                status: "success".into(),
                decision: "allow".into(),
                allow: true,
                reason: "Break-glass mode activated".into(),
                effects: serde_json::json!({}),
                obligations: vec![],
                metadata: serde_json::json!({}),
            });
        }

        let mut combined_decision = PolicyDecision {
            evaluator_id: "router_combiner".into(),
            evaluator_type: "router".into(),
            required: true,
            status: "success".into(),
            decision: "allow".into(),
            allow: true,
            reason: "All evaluators passed".into(),
            effects: serde_json::json!({}),
            obligations: vec![],
            metadata: serde_json::json!({}),
        };

        let mut to_evaluate = route.pdp_required.clone();
        for cond in &route.pdp_conditional {
            if payload.get(&cond.required_payload_key).is_some() || cond.required_payload_key == "*"
            {
                to_evaluate.push(cond.evaluator.clone());
            }
        }
        if !route.pdp_pool.is_empty() {
            if let Some(pdp) = self.select_pdp_from_pool(&route.pdp_pool, &route.failover_strategy) {
                to_evaluate.push(pdp);
            }
        }

        for ev_id in to_evaluate {
            if let Some(evaluator) = self.evaluators.get(&ev_id) {
                let breaker = self.breakers.get(&ev_id).cloned();
                
                // Check circuit breaker before hitting PDP
                if let Some(ref b) = breaker {
                    if let Admit::Reject = b.permitted() {
                        metrics::counter!("dek_proxy_requests_total", "decision" => "deny", "reason" => "circuit_open", "evaluator" => ev_id.clone()).increment(1);
                        tracing::warn!(%ev_id, "request rejected (circuit breaker open)");
                        combined_decision.allow = false;
                        combined_decision.decision = "deny".into();
                        combined_decision.reason = format!("Blocked by Circuit Breaker for {}", ev_id);
                        break;
                    }
                }

                let start_time = std::time::Instant::now();
                let eval_fut = evaluator.evaluate(payload.clone());
                let timeout_dur = std::time::Duration::from_millis(self.pdp_timeout_ms);
                
                match tokio::time::timeout(timeout_dur, eval_fut).await {
                    Ok(Ok(res)) => {
                        let latency = start_time.elapsed().as_millis() as f64;
                        metrics::histogram!("dek_policy_eval_latency_ms", "evaluator" => ev_id.clone()).record(latency);

                        tracing::info!("Evaluator {} returned: {}", ev_id, res.decision);
                        
                        if let Some(ref b) = breaker {
                            b.on_success();
                        }
                        if let Some(stats) = self.stats.get(&ev_id) {
                            let mut s = stats.lock_safe();
                            s.successes += 1;
                            s.record_latency(latency, 0.2);
                        }

                        // Combine obligations
                        combined_decision
                            .obligations
                            .extend(res.obligations.clone());

                        // Merge effects (simple mock merge)
                        if let serde_json::Value::Object(mut combined_map) =
                            combined_decision.effects.clone()
                        {
                            if let serde_json::Value::Object(res_map) = res.effects.clone() {
                                for (k, v) in res_map {
                                    combined_map.insert(k, v);
                                }
                            }
                            combined_decision.effects = serde_json::Value::Object(combined_map);
                        }

                        if !res.allow {
                            // Deny overrides
                            combined_decision.allow = false;
                            combined_decision.decision = "deny".into();
                            combined_decision.reason = format!("Blocked by {}", ev_id);
                            // Short-circuit on deny
                            break;
                        }
                    }
                    Ok(Err(dek_policy_runtime::PolicyError::Unavailable(msg))) => {
                        metrics::counter!("dek_pdp_unavailable_total", "evaluator" => ev_id.clone()).increment(1);
                        tracing::warn!("required PDP unavailable: {msg}; mode: {:?}", route.enforcement_mode);
                        if let Some(ref b) = breaker { b.on_failure(); }
                        if let Some(stats) = self.stats.get(&ev_id) { stats.lock_safe().failures += 1; }
                        combined_decision.allow = false;
                        combined_decision.decision = "deny".into();
                        combined_decision.reason = format!("required PDP unavailable: {} (Mode: {:?})", msg, route.enforcement_mode);
                        break;
                    }
                    Ok(Err(e)) => {
                        metrics::counter!("dek_pdp_error_total", "evaluator" => ev_id.clone()).increment(1);
                        tracing::warn!("PDP error: {e}; mode: {:?}", route.enforcement_mode);
                        if let Some(ref b) = breaker { b.on_failure(); }
                        if let Some(stats) = self.stats.get(&ev_id) { stats.lock_safe().failures += 1; }
                        combined_decision.allow = false;
                        combined_decision.decision = "deny".into();
                        combined_decision.reason = format!("PDP error: {} (Mode: {:?})", e, route.enforcement_mode);
                        break;
                    }
                    Err(_) => { // Timeout elapsed
                        metrics::counter!("dek_pdp_timeout_total", "evaluator" => ev_id.clone()).increment(1);
                        tracing::warn!("PDP timeout for evaluator: {}", ev_id);
                        if let Some(ref b) = breaker { b.on_failure(); }
                        if let Some(stats) = self.stats.get(&ev_id) { stats.lock_safe().failures += 1; }
                        combined_decision.allow = false;
                        combined_decision.decision = "deny".into();
                        combined_decision.reason = format!("PDP timeout ({} ms) for {}", self.pdp_timeout_ms, ev_id);
                        break;
                    }
                }
            } else {
                tracing::warn!(
                    "Error: Required evaluator {} not found. Mode: {:?}",
                    ev_id,
                    route.enforcement_mode
                );
                combined_decision.allow = false;
                combined_decision.decision = "deny".into();
                combined_decision.reason = format!(
                    "Required evaluator {} not configured or failed to load (Mode: {:?})",
                    ev_id, route.enforcement_mode
                );
                break;
            }
        }

        Ok(combined_decision)
    }
}

impl Default for PolicyRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use async_trait::async_trait;
    use dek_policy_runtime::PolicyDecision;

    struct DummyRuntime;
    #[async_trait]
    impl PolicyRuntime for DummyRuntime {
        async fn evaluate(
            &self,
            _input: serde_json::Value,
        ) -> std::result::Result<PolicyDecision, dek_policy_runtime::PolicyError> {
            Ok(PolicyDecision {
                evaluator_id: "dummy".into(),
                evaluator_type: "dummy".into(),
                required: true,
                status: "success".into(),
                decision: "allow".into(),
                allow: true,
                reason: "mocked".into(),
                effects: serde_json::json!({}),
                obligations: vec![],
                metadata: serde_json::json!({}),
            })
        }
        fn version(&self) -> String {
            "1.0".into()
        }
    }

    #[tokio::test]
    async fn test_empty_router_denies_all() {
        let router = PolicyRouter::new();
        let payload = serde_json::json!({ "request_type": "tools/call" });
        let res = router.authorize(payload).await.unwrap();
        assert_eq!(res.decision, "deny");
        assert_eq!(res.reason, "no matching route");
    }

    #[tokio::test]
    async fn test_route_matches_and_allows() {
        let mut router = PolicyRouter::new();
        router.register_evaluator("dummy", Box::new(DummyRuntime));
        router.set_routes(vec![Route {
            id: "route1".into(),
            priority: 10,
            enforcement_mode: EnforcementMode::Standard,
            match_rule: EnterpriseMatchRule {
                method: Some("test".into()),
                tool_category: None,
                resource_type: None,
                severity_level: None,
            },
            pdp_required: vec!["dummy".into()],
            pdp_conditional: vec![],
            pdp_pool: vec![],
            failover_strategy: FailoverStrategy::Priority,
        }]);

        let payload = serde_json::json!({ "request_type": "test" });
        let res = router.authorize(payload).await.unwrap();
        assert_eq!(res.decision, "allow");
    }

    #[tokio::test]
    async fn test_breakglass_mode() {
        let mut router = PolicyRouter::new();
        router.set_routes(vec![Route {
            id: "route_emergency".into(),
            priority: 100,
            enforcement_mode: EnforcementMode::BreakGlass,
            match_rule: EnterpriseMatchRule {
                method: Some("*".into()),
                tool_category: None,
                resource_type: None,
                severity_level: None,
            },
            pdp_required: vec!["missing_pdp".into()], // would normally fail
            pdp_conditional: vec![],
            pdp_pool: vec![],
            failover_strategy: FailoverStrategy::Priority,
        }]);

        let payload = serde_json::json!({ "request_type": "emergency_action" });
        let res = router.authorize(payload).await.unwrap();
        assert_eq!(res.decision, "allow");
        assert_eq!(res.reason, "Break-glass mode activated");
    }
}
