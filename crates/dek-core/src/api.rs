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
use tokio::net::TcpListener;
use tracing::{error, info};

use dek_policy_syncer::EnforcementState;

#[derive(Clone)]
struct ApiState {
    snapshot: Arc<ArcSwap<RuntimeSnapshot>>,
    enforcement: Arc<ArcSwap<EnforcementState>>,
}

pub async fn start_sidecar_api(
    snapshot: Arc<ArcSwap<RuntimeSnapshot>>,
    enforcement: Arc<ArcSwap<EnforcementState>>,
    port: u16,
) -> anyhow::Result<()> {
    let state = ApiState { snapshot, enforcement };

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
    if let EnforcementState::StrictDeny { ref reason, .. } = **enf {
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
        return (StatusCode::OK, Json(serde_json::to_value(response).unwrap()));
    }

    let snap = state.snapshot.load();
    let val = serde_json::to_value(&req).unwrap_or(serde_json::json!({}));

    let start = std::time::Instant::now();
    let res = snap.router.authorize(val).await.unwrap_or_else(|_| dek_policy_runtime::PolicyDecision {
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
        obligations: res.obligations.into_iter().map(|o| dek_decision::Obligation {
            kind: o,
            parameters: serde_json::json!({}),
        }).collect(),
        effects: res.effects,
        policy_bundle_id: "active".into(),
        policy_bundle_version: "v1".into(),
        evaluator_results: vec![],
        latency_ms: latency,
    };

    let mut json_resp = serde_json::to_value(response).unwrap();
    if has_require_approval {
        if let Some(obj) = json_resp.as_object_mut() {
            obj.insert("error".to_string(), serde_json::json!({
                "code": -32002,
                "message": "Access Denied: pending_approval",
                "data": {
                    "status": "denied",
                    "reason": res.reason
                }
            }));
        }
    }

    (StatusCode::OK, Json(json_resp))
}

async fn batch_check(
    State(state): State<ApiState>,
    Json(reqs): Json<Vec<DecisionRequest>>,
) -> impl IntoResponse {
    let mut responses = Vec::with_capacity(reqs.len());
    
    let enf = state.enforcement.load();
    let is_strict_deny = matches!(**enf, EnforcementState::StrictDeny { .. });
    let deny_reason = if let EnforcementState::StrictDeny { ref reason, .. } = **enf {
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

        responses.push(DecisionResponse {
            decision_id: uuid::Uuid::new_v4().to_string(),
            allow: res.allow,
            reason_code: if res.allow {
                "OK".into()
            } else {
                "DENIED_BY_POLICY".into()
            },
            reason: res.reason,
            obligations: vec![],
            effects: res.effects,
            policy_bundle_id: "active".into(),
            policy_bundle_version: "v1".into(),
            evaluator_results: vec![],
            latency_ms: latency,
        });
    }

    (StatusCode::OK, Json(responses))
}
