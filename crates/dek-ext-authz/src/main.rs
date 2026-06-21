pub mod envoy {
    pub mod service {
        pub mod auth {
            pub mod v3 {
                tonic::include_proto!("envoy.service.auth.v3");
            }
        }
    }
}

use envoy::service::auth::v3::authorization_server::{Authorization, AuthorizationServer};
use envoy::service::auth::v3::{CheckRequest, CheckResponse, check_response::HttpResponse, DeniedHttpResponse, OkHttpResponse, HttpStatus};
use envoy::service::auth::v3::Status as EnvoyStatus;
use tonic::{transport::Server, Request, Response, Status};
use serde_json::{json, Value};
use dek_policy_router::PolicyRouter;
use dek_config::BootstrapConfig;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

#[derive(Clone)]
pub struct ExtAuthzService {
    router: Arc<RwLock<PolicyRouter>>,
    tenant_id: String,
    device_id: String,
}

#[tonic::async_trait]
impl Authorization for ExtAuthzService {
    async fn check(
        &self,
        request: Request<CheckRequest>,
    ) -> Result<Response<CheckResponse>, Status> {
        let req = request.into_inner();
        
        // Extract HTTP attributes from envoy request
        let mut policy_input = json!({
            "pep_mode": "envoy_ext_authz",
            "context": {
                "tenant_id": self.tenant_id,
                "device_id": self.device_id,
            }
        });

        if let Some(attrs) = req.attributes {
            if let Some(http) = attrs.request {
                policy_input["action"] = json!(http.method);
                policy_input["resource"] = json!({
                    "type": "url",
                    "id": format!("{}://{}{}", http.scheme, http.host, http.path),
                    "attributes": {
                        "headers": http.headers
                    }
                });
            }
            if let Some(source) = attrs.source {
                policy_input["principal"] = json!({
                    "id": source.principal
                });
            }
        }

        // Evaluate Policy
        let decision = self.router.read().await.authorize(policy_input).await.unwrap_or_else(|e| {
            error!("Policy routing failed: {}", e);
            dek_policy_runtime::PolicyDecision {
                evaluator_id: "ext_authz".into(),
                evaluator_type: "router".into(),
                required: true,
                status: "error".into(),
                decision: "deny".into(),
                allow: false,
                reason: "Internal policy router error".into(),
                effects: json!({}),
                obligations: vec![],
                metadata: json!({}),
            }
        });

        if decision.allow {
            info!("ExtAuthz: Request Allowed");
            let ok_response = OkHttpResponse {
                headers: vec![],
                headers_to_remove: vec![],
            };

            let check_res = CheckResponse {
                status: Some(EnvoyStatus {
                    code: 0, // OK
                    message: "Allowed by Pollen DEK".to_string(),
                    details: vec![]
                }),
                http_response: Some(HttpResponse::OkResponse(ok_response)),
            };
            Ok(Response::new(check_res))
        } else {
            warn!("ExtAuthz: Request Denied: {}", decision.reason);
            let denied_response = DeniedHttpResponse {
                status: Some(HttpStatus {
                    code: 403, // Forbidden
                }),
                headers: vec![],
                body: format!("Access Denied by Pollen DEK: {}", decision.reason),
            };

            let check_res = CheckResponse {
                status: Some(EnvoyStatus {
                    code: 7, // PermissionDenied
                    message: decision.reason.clone(),
                    details: vec![]
                }),
                http_response: Some(HttpResponse::DeniedResponse(denied_response)),
            };
            Ok(Response::new(check_res))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();
    info!("Starting Pollen DEK Envoy ext_authz Server...");

    let bootstrap = BootstrapConfig::load_or_default("bootstrap.json").unwrap_or_else(|_| BootstrapConfig {
        device_id: "local-device".into(),
        mtls: dek_config::MtlsConfig {
            client_cert_path: "".into(),
            client_key_path: "".into(),
            root_ca_path: "".into(),
        },
        pinned_bundle_public_key: "".into(),
        cloud_url: "".into(),
        spiffe_id: None,
        tenant_id: None,
    });

    let mut router = PolicyRouter::new();
    let bundle_path_buf = dek_config::paths::get_active_bundle_path();
    let staged_path = std::path::Path::new(&bundle_path_buf);
    if staged_path.exists()
        && let Ok(content) = std::fs::read_to_string(staged_path)
            && let Ok(payload) = serde_json::from_str::<Value>(&content) {
                info!("Loading dynamic policy evaluator configuration from active_bundle.json");
                dek_router_builder::load_router_config(&mut router, &payload);
            }

    let addr = "[::1]:50051".parse()?;
    let service = ExtAuthzService {
        router: Arc::new(RwLock::new(router)),
        tenant_id: bootstrap.tenant_id.unwrap_or_else(|| "default".into()),
        device_id: bootstrap.device_id,
    };

    info!("Envoy ext_authz gRPC listening on {}", addr);
    Server::builder()
        .add_service(AuthorizationServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
