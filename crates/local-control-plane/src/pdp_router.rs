use crate::error::{ApiError, ApiResult};
use crate::pdp_models::{PdpFailureBehavior, PdpRouteRule, PdpRuntime};
use crate::state::AppState;

pub struct PdpRouterService {
    state: AppState,
}

impl PdpRouterService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn evaluate_route(
        &self,
        tenant: &str,
        agent_id: Option<&str>,
        resource_id: Option<&str>,
        protocol: Option<&str>,
    ) -> ApiResult<Option<PdpRouteRule>> {
        let list = self
            .state
            .pdp_store
            .list_routes(tenant)
            .await
            .map_err(ApiError::Internal)?;

        let mut routes = vec![];
        for val in list {
            if let Ok(r) = serde_json::from_value::<PdpRouteRule>(val) {
                if r.enabled {
                    routes.push(r);
                }
            }
        }

        routes.sort_by_key(|r| r.priority);

        for route in routes {
            if self.matches_route(&route, agent_id, resource_id, protocol) {
                return Ok(Some(route));
            }
        }

        Ok(None)
    }

    fn matches_route(
        &self,
        route: &PdpRouteRule,
        agent_id: Option<&str>,
        resource_id: Option<&str>,
        protocol: Option<&str>,
    ) -> bool {
        let cond = &route.match_cond;

        if let Some(a_id) = agent_id {
            if let Some(agents) = &cond.agent_ids {
                if !agents.is_empty() && !agents.contains(&a_id.to_string()) {
                    return false;
                }
            }
        }

        if let Some(r_id) = resource_id {
            if let Some(resources) = &cond.resource_ids {
                if !resources.is_empty() && !resources.contains(&r_id.to_string()) {
                    return false;
                }
            }
        }

        if let Some(p) = protocol {
            if let Some(protocols) = &cond.protocols {
                if !protocols.is_empty() && !protocols.contains(&p.to_string()) {
                    return false;
                }
            }
        }

        true
    }

    pub async fn simulate_route(
        &self,
        tenant: &str,
        agent_id: Option<&str>,
        resource_id: Option<&str>,
        protocol: Option<&str>,
    ) -> ApiResult<serde_json::Value> {
        let start = std::time::Instant::now();
        let rule = self
            .evaluate_route(tenant, agent_id, resource_id, protocol)
            .await?;

        let latency = start.elapsed().as_millis() as u64;

        let (pdp_used, fallback, reason, decision) = match &rule {
            Some(r) => (
                r.primary_pdp_id.clone(),
                false,
                "primary selected",
                "Simulated Allow",
            ),
            None => ("none".to_string(), false, "no matching route", "Deny"),
        };

        let audit = serde_json::json!({
            "event_type": "route_evaluation",
            "tenant": tenant,
            "agent_id": agent_id,
            "resource_id": resource_id,
            "selected_pdp": pdp_used,
            "fallback_used": fallback,
            "reason": reason,
            "latency_ms": latency,
            "final_decision": decision,
            "route_rule_id": rule.map(|r| r.id),
        });

        tracing::info!("Audit Event: {}", audit);

        Ok(audit)
    }

    pub async fn execute_route(
        &self,
        tenant: &str,
        agent_id: Option<&str>,
        resource_id: Option<&str>,
        protocol: Option<&str>,
        payload: &serde_json::Value,
    ) -> ApiResult<serde_json::Value> {
        let start = std::time::Instant::now();
        let rule_opt = self
            .evaluate_route(tenant, agent_id, resource_id, protocol)
            .await?;

        let rule = match rule_opt {
            Some(r) => r,
            None => {
                let audit = serde_json::json!({
                    "event_type": "route_execution",
                    "tenant": tenant,
                    "agent_id": agent_id,
                    "resource_id": resource_id,
                    "selected_pdp": "none",
                    "fallback_used": false,
                    "reason": "no matching route",
                    "latency_ms": start.elapsed().as_millis() as u64,
                    "final_decision": "Deny",
                });
                tracing::info!("Audit Event: {}", audit);
                return Ok(serde_json::json!({"decision": "Deny", "reason": "No matching route"}));
            }
        };

        let mut errors = vec![];
        let mut final_decision = None;
        let mut pdp_used = "none".to_string();
        let mut fallback_used = false;

        match self
            .call_pdp(
                tenant,
                &rule.primary_pdp_id,
                payload,
                rule.timeout_ms,
                rule.max_retries,
            )
            .await
        {
            Ok(decision) => {
                final_decision = Some(decision);
                pdp_used = rule.primary_pdp_id.clone();
            }
            Err(e) => {
                errors.push(format!("Primary {}: {}", rule.primary_pdp_id, e));
            }
        }

        if final_decision.is_none() {
            fallback_used = true;
            for fb_id in &rule.fallback_pdp_ids {
                match self
                    .call_pdp(tenant, fb_id, payload, rule.timeout_ms, rule.max_retries)
                    .await
                {
                    Ok(decision) => {
                        final_decision = Some(decision);
                        pdp_used = fb_id.clone();
                        break;
                    }
                    Err(e) => {
                        errors.push(format!("Fallback {}: {}", fb_id, e));
                    }
                }
            }
        }

        let decision_val = match final_decision {
            Some(d) => d,
            None => {
                let fallback_decision = match rule.failure_behavior {
                    PdpFailureBehavior::Allow => "Allow",
                    PdpFailureBehavior::Deny => "Deny",
                    _ => "Deny",
                };
                serde_json::json!({
                    "decision": fallback_decision,
                    "reason": format!("All PDPs failed. Errors: {:?}", errors)
                })
            }
        };

        if !rule.shadow_pdp_ids.is_empty() {
            let shadow_ids = rule.shadow_pdp_ids.clone();
            let tenant_clone = tenant.to_string();
            let payload_clone = payload.clone();
            let state_clone = self.state.clone();
            let primary_decision = decision_val.clone();
            let t_ms = rule.timeout_ms;
            let m_retries = rule.max_retries;

            tokio::spawn(async move {
                for sid in shadow_ids {
                    let svc = PdpRouterService::new(state_clone.clone());
                    match svc
                        .call_pdp(&tenant_clone, &sid, &payload_clone, t_ms, m_retries)
                        .await
                    {
                        Ok(shadow_res) => {
                            if shadow_res != primary_decision {
                                tracing::warn!(
                                    "Shadow mismatch! Primary: {}, Shadow ({}): {}",
                                    primary_decision,
                                    sid,
                                    shadow_res
                                );
                                let audit = serde_json::json!({
                                    "event_type": "shadow_mismatch",
                                    "tenant": tenant_clone,
                                    "shadow_pdp_id": sid,
                                    "primary_decision": primary_decision,
                                    "shadow_decision": shadow_res,
                                });
                                tracing::info!("Audit Event: {}", audit);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Shadow PDP {} failed: {}", sid, e);
                        }
                    }
                }
            });
        }

        let latency = start.elapsed().as_millis() as u64;

        let audit = serde_json::json!({
            "event_type": "route_execution",
            "tenant": tenant,
            "agent_id": agent_id,
            "resource_id": resource_id,
            "selected_pdp": pdp_used,
            "fallback_used": fallback_used,
            "reason": "evaluated",
            "latency_ms": latency,
            "final_decision": decision_val,
            "route_rule_id": rule.id,
            "errors": errors,
        });
        tracing::info!("Audit Event: {}", audit);

        Ok(decision_val)
    }

    async fn call_pdp(
        &self,
        tenant: &str,
        pdp_id: &str,
        payload: &serde_json::Value,
        timeout_ms: u64,
        max_retries: u32,
    ) -> Result<serde_json::Value, String> {
        let rt_val = self
            .state
            .pdp_store
            .get_runtime(tenant, pdp_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "PDP Runtime not found".to_string())?;

        let rt: PdpRuntime = serde_json::from_value(rt_val).map_err(|e| e.to_string())?;

        if let Some(endpoint) = rt.endpoint {
            let client = reqwest::Client::new();

            let mut bearer_token = None;
            if let Some(token) = self
                .state
                .pdp_credentials
                .retrieve_credential(&rt.id)
                .await
                .unwrap_or(None)
            {
                bearer_token = Some(token);
            } else if let Some(auth_ref) = rt.auth_ref {
                bearer_token = Some(auth_ref);
            }

            let mut attempts = 0;
            loop {
                attempts += 1;
                let mut req = client
                    .post(&endpoint)
                    .timeout(std::time::Duration::from_millis(timeout_ms))
                    .json(payload);

                if let Some(ref token) = bearer_token {
                    req = req.bearer_auth(token);
                }

                match req.send().await {
                    Ok(resp) if resp.status().is_success() => {
                        let json = resp.json::<serde_json::Value>().await.unwrap_or_else(|_| serde_json::json!({"decision": "Deny", "reason": "invalid response body"}));
                        return Ok(json);
                    }
                    Ok(resp) => {
                        if attempts > max_retries {
                            return Err(format!("HTTP {}", resp.status()));
                        }
                    }
                    Err(e) => {
                        if attempts > max_retries {
                            return Err(e.to_string());
                        }
                    }
                }
            }
        } else {
            // Built in engines
            Ok(serde_json::json!({"decision": "Allow", "reason": "mock local engine"}))
        }
    }
}
