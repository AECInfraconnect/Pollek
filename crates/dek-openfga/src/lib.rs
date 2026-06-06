use anyhow::Result;
use async_trait::async_trait;
use dek_policy_runtime::{PolicyDecision, PolicyRuntime};
use reqwest::Client;
use serde_json::json;

use dek_config::MtlsConfig;

pub struct OpenFgaAdapter {
    endpoint: String,
    store_id: String,
    client: Client,
}

impl OpenFgaAdapter {
    pub fn new(endpoint: &str, store_id: &str, mtls: Option<&MtlsConfig>) -> Result<Self> {
        let client = if let Some(m) = mtls {
            m.build_client(None)?
        } else {
            Client::new()
        };
        Ok(Self {
            endpoint: endpoint.to_string(),
            store_id: store_id.to_string(),
            client,
        })
    }
}

#[async_trait]
impl PolicyRuntime for OpenFgaAdapter {
    async fn evaluate(&self, input: serde_json::Value) -> Result<PolicyDecision> {
        let principal = input
            .get("principal")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let resource = input
            .get("resource")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        println!(
            "Checking OpenFGA at {}/stores/{}/check",
            self.endpoint, self.store_id
        );
        println!(
            "Tuple: user={}, relation={}, object={}",
            principal, action, resource
        );

        let url = format!("{}/stores/{}/check", self.endpoint, self.store_id);
        let payload = json!({
            "tuple_key": {
                "user": principal,
                "relation": action,
                "object": resource
            }
        });

        // Make the real HTTP request to OpenFGA
        let mut allowed = false;
        let reason;

        let mut status = "success".to_string();

        match self.client.post(&url).json(&payload).send().await {
            Ok(res) => {
                if res.status().is_success() {
                    if let Ok(resp_json) = res.json::<serde_json::Value>().await {
                        allowed = resp_json
                            .get("allowed")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        reason = if allowed {
                            "OpenFGA remote check allowed".to_string()
                        } else {
                            "OpenFGA remote check denied".to_string()
                        };
                    } else {
                        status = "error".to_string();
                        reason = "Failed to parse OpenFGA JSON response".to_string();
                    }
                } else {
                    status = "error".to_string();
                    reason = format!("OpenFGA returned status: {}", res.status());
                }
            }
            Err(e) => {
                status = "error".to_string();
                reason = format!("Failed to connect to OpenFGA: {}", e);
            }
        }

        Ok(PolicyDecision {
            evaluator_id: "openfga_remote".to_string(),
            evaluator_type: "remote_pdp".to_string(),
            required: true,
            status,
            decision: if allowed {
                "allow".to_string()
            } else {
                "deny".to_string()
            },
            allow: allowed,
            reason,
            effects: serde_json::json!({}),
            obligations: vec![],
            metadata: serde_json::json!({ "store_id": self.store_id }),
        })
    }

    fn version(&self) -> String {
        "openfga-v1.0.0".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openfga_new_no_mtls() {
        let adapter = OpenFgaAdapter::new("http://localhost:8080", "store_1", None).unwrap();
        assert_eq!(adapter.endpoint, "http://localhost:8080");
        assert_eq!(adapter.store_id, "store_1");
    }

    #[tokio::test]
    async fn test_openfga_evaluate_network_error() {
        // Points to an invalid endpoint
        let adapter = OpenFgaAdapter::new("http://127.0.0.1:0", "store_1", None).unwrap();
        let payload = json!({ "principal": "user_1", "action": "read", "resource": "doc_1" });
        let result = adapter.evaluate(payload).await.unwrap();

        assert_eq!(result.allow, false);
        assert_eq!(result.decision, "deny");
        assert_eq!(result.status, "error");
        assert!(result.reason.contains("Failed to connect"));
    }
}
