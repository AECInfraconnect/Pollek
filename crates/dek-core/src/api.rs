// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use arc_swap::ArcSwap;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dek_activation::snapshot::RuntimeSnapshot;
use dek_decision::{DecisionRequest, DecisionResponse};
use std::sync::Arc;
use tracing::{error, info};

use dek_policy_syncer::EnforcementState;
use dek_telemetry::CloudTelemetrySink;

#[derive(Clone)]
struct ApiState {
    snapshot: Arc<ArcSwap<RuntimeSnapshot>>,
    enforcement: Arc<ArcSwap<EnforcementState>>,
    telemetry: Option<Arc<CloudTelemetrySink>>,
    identity_health: tokio::sync::watch::Receiver<crate::svid_renewal_failclosed::IdentityHealth>,
}

pub async fn start_sidecar_api(
    snapshot: Arc<ArcSwap<RuntimeSnapshot>>,
    enforcement: Arc<ArcSwap<EnforcementState>>,
    telemetry: Option<Arc<CloudTelemetrySink>>,
    identity_health: tokio::sync::watch::Receiver<crate::svid_renewal_failclosed::IdentityHealth>,
    port: u16,
) -> anyhow::Result<()> {
    let state = ApiState {
        snapshot,
        enforcement,
        telemetry,
        identity_health,
    };

    let app = Router::new()
        .route("/v1/healthz", get(healthz))
        .route("/v1/readyz", get(readyz))
        .route("/v1/capabilities", get(capabilities))
        .route("/v1/decision/check", post(check))
        .route("/v1/decision/batch-check", post(batch_check))
        .with_state(state);

    let addr = format!("127.0.0.1:{}", port);
    info!("Starting Sidecar API on {}", addr);
    let socket_addr: std::net::SocketAddr = addr.parse()?;
    let socket = if socket_addr.is_ipv6() {
        tokio::net::TcpSocket::new_v6()?
    } else {
        tokio::net::TcpSocket::new_v4()?
    };
    let _ = socket.set_reuseaddr(true);
    socket.bind(socket_addr)?;
    let listener = socket.listen(1024)?;

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            error!("Sidecar API server failed: {}", e);
        }
    });

    Ok(())
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

async fn readyz() -> impl IntoResponse {
    (StatusCode::OK, "READY")
}

async fn capabilities() -> impl IntoResponse {
    Json(dek_domain_schema::EnforcementCapabilities::detect())
}

async fn check(
    State(state): State<ApiState>,
    Json(req): Json<DecisionRequest>,
) -> impl IntoResponse {
    let enf = state.enforcement.load();
    let health = *state.identity_health.borrow();

    let is_strict_deny = matches!(**enf, EnforcementState::StrictDeny { .. })
        || health == crate::svid_renewal_failclosed::IdentityHealth::Expired;

    if is_strict_deny {
        let reason = if health == crate::svid_renewal_failclosed::IdentityHealth::Expired {
            "SVID expired".to_string()
        } else if let EnforcementState::StrictDeny { ref reason, .. } = **enf {
            reason.clone()
        } else {
            "Unknown".to_string()
        };

        let response = DecisionResponse {
            decision_id: uuid::Uuid::new_v4().to_string(),
            allow: false,
            reason_code: "DENIED_BY_FAIL_CLOSED".into(),
            reason: format!("fail-closed: {}", reason),
            obligations: vec![],
            effects: serde_json::json!({}),
            policy_bundle_id: "active".into(),
            policy_bundle_version: "v1".into(),
            evaluator_results: vec![],
            latency_ms: 0,
        };
        return (
            StatusCode::OK,
            Json(serde_json::to_value(response).unwrap_or_else(|_| serde_json::json!({}))),
        );
    }

    let snap = state.snapshot.load();
    let val = serde_json::to_value(&req).unwrap_or(serde_json::json!({}));

    let start = std::time::Instant::now();
    let res =
        snap.router
            .authorize(val)
            .await
            .unwrap_or_else(|_| dek_policy_runtime::PolicyDecision {
                evaluator_id: "core_api".into(),
                evaluator_type: "router".into(),
                required: true,
                status: "error".into(),
                decision: "deny".into(),
                allow: false,
                reason: "Policy evaluation failed".into(),
                effects: serde_json::json!({}),
                obligations: vec![],
                metadata: serde_json::json!({}),
            });

    let latency = start.elapsed().as_millis() as u64;

    let has_require_approval = res.obligations.iter().any(|o| o == "require_approval");
    let allow = res.allow && !has_require_approval;

    let response = DecisionResponse {
        decision_id: uuid::Uuid::new_v4().to_string(),
        allow,
        reason_code: if has_require_approval {
            "PENDING_APPROVAL".into()
        } else if allow {
            "OK".into()
        } else {
            "DENIED_BY_POLICY".into()
        },
        reason: res.reason.clone(),
        obligations: res
            .obligations
            .into_iter()
            .map(|o| dek_decision::Obligation {
                kind: o,
                parameters: serde_json::json!({}),
            })
            .collect(),
        effects: res.effects,
        policy_bundle_id: "active".into(),
        policy_bundle_version: "v1".into(),
        evaluator_results: vec![],
        latency_ms: latency,
    };

    let mut json_resp = serde_json::to_value(&response).unwrap_or_else(|_| serde_json::json!({}));
    if has_require_approval {
        if let Some(obj) = json_resp.as_object_mut() {
            obj.insert(
                "error".to_string(),
                serde_json::json!({
                    "code": -32002,
                    "message": "Access Denied: pending_approval",
                    "data": {
                        "status": "denied",
                        "reason": res.reason
                    }
                }),
            );
        }
    }

    if let Some(telemetry) = &state.telemetry {
        let event = serde_json::json!({
            "schema_version": "1.0",
            "event_id": response.decision_id,
            "event_type": "decision",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "tenant_id": req.context.get("tenant_id").and_then(|v| v.as_str()).unwrap_or("local").to_string(),
            "device_id": "api-sidecar",
            "redaction_applied": true,
            "payload": {
                "trace_id": "",
                "span_id": "",
                "spiffe_id": "",
                "pep_type": "api",
                "agent_id": "dek",
                "principal_id": req.principal,
                "action": req.action,
                "resource_id": req.resource,
                "decision": if res.allow { "allow" } else { "deny" },
                "reason": res.reason,
                "latency_ms": latency,
                "policy_bundle_id": "active",
                "policy_bundle_version": "v1",
                "evaluator_type": res.evaluator_type,
                "effects": response.effects,
            }
        });
        telemetry.emit_async(event, dek_telemetry::spooler::Priority::Normal);
    }

    (StatusCode::OK, Json(json_resp))
}

async fn batch_check(
    State(state): State<ApiState>,
    Json(reqs): Json<Vec<DecisionRequest>>,
) -> impl IntoResponse {
    let mut responses = Vec::with_capacity(reqs.len());

    let enf = state.enforcement.load();
    let health = *state.identity_health.borrow();
    let is_strict_deny = matches!(**enf, EnforcementState::StrictDeny { .. })
        || health == crate::svid_renewal_failclosed::IdentityHealth::Expired;

    let deny_reason = if health == crate::svid_renewal_failclosed::IdentityHealth::Expired {
        "fail-closed: SVID expired".to_string()
    } else if let EnforcementState::StrictDeny { ref reason, .. } = **enf {
        format!("fail-closed: {}", reason)
    } else {
        String::new()
    };

    // Simple serial evaluation for now. Could be parallelized.
    for req in reqs {
        if is_strict_deny {
            responses.push(DecisionResponse {
                decision_id: uuid::Uuid::new_v4().to_string(),
                allow: false,
                reason_code: "DENIED_BY_FAIL_CLOSED".into(),
                reason: deny_reason.clone(),
                obligations: vec![],
                effects: serde_json::json!({}),
                policy_bundle_id: "active".into(),
                policy_bundle_version: "v1".into(),
                evaluator_results: vec![],
                latency_ms: 0,
            });
            continue;
        }

        let snap = state.snapshot.load();
        let val = serde_json::to_value(&req).unwrap_or(serde_json::json!({}));

        let start = std::time::Instant::now();
        let res = snap.router.authorize(val).await.unwrap_or_else(|_| {
            dek_policy_runtime::PolicyDecision {
                evaluator_id: "core_api".into(),
                evaluator_type: "router".into(),
                required: true,
                status: "error".into(),
                decision: "deny".into(),
                allow: false,
                reason: "Policy evaluation failed".into(),
                effects: serde_json::json!({}),
                obligations: vec![],
                metadata: serde_json::json!({}),
            }
        });
        let latency = start.elapsed().as_millis() as u64;

        let response = DecisionResponse {
            decision_id: uuid::Uuid::new_v4().to_string(),
            allow: res.allow,
            reason_code: if res.allow {
                "OK".into()
            } else {
                "DENIED_BY_POLICY".into()
            },
            reason: res.reason.clone(),
            obligations: vec![],
            effects: res.effects.clone(),
            policy_bundle_id: "active".into(),
            policy_bundle_version: "v1".into(),
            evaluator_results: vec![],
            latency_ms: latency,
        };

        if let Some(telemetry) = &state.telemetry {
            let event = serde_json::json!({
                "schema_version": "1.0",
                "event_id": response.decision_id.clone(),
                "event_type": "decision",
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "tenant_id": req.context.get("tenant_id").and_then(|v| v.as_str()).unwrap_or("local").to_string(),
                "device_id": "api-sidecar",
                "redaction_applied": true,
                "payload": {
                    "trace_id": "",
                    "span_id": "",
                    "spiffe_id": "",
                    "pep_type": "api",
                    "agent_id": "dek",
                    "principal_id": req.principal.clone(),
                    "action": req.action.clone(),
                    "resource_id": req.resource.clone(),
                    "decision": if res.allow { "allow" } else { "deny" },
                    "reason": res.reason.clone(),
                    "latency_ms": latency,
                    "policy_bundle_id": "active",
                    "policy_bundle_version": "v1",
                    "evaluator_type": res.evaluator_type.clone(),
                    "effects": res.effects.clone(),
                }
            });
            telemetry.emit_async(event, dek_telemetry::spooler::Priority::Normal);
        }

        responses.push(response);
    }

    (StatusCode::OK, Json(responses))
}
