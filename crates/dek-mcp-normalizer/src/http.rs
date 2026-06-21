use crate::{MessageDirection, NormalizedMcpEvent, TransportAdapter, TransportType};
use anyhow::Result;
use serde_json::json;
use uuid::Uuid;

pub struct HttpTransportAdapter;

impl TransportAdapter for HttpTransportAdapter {
    fn transport_name(&self) -> &'static str {
        "http"
    }

    fn normalize_request(
        &self,
        raw: serde_json::Value,
        tenant_id: &str,
        device_id: &str,
        spiffe_id: Option<&str>,
        user_id: Option<&str>,
    ) -> Result<NormalizedMcpEvent> {
        let method = raw.get("method").and_then(|v| v.as_str()).unwrap_or("unknown");
        
        let mut tool_name = None;
        let mut server_id = None;

        // If it's tools/call, the tool name is in params.name
        if method == "tools/call" {
            if let Some(params) = raw.get("params") {
                tool_name = params.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
                // For proxy scenario, server_id might be provided in headers or query, 
                // but if it's in params, extract it (though MCP spec doesn't natively have server_id in tools/call).
            }
        }

        let jsonrpc_id = raw.get("id").cloned();

        Ok(NormalizedMcpEvent {
            event_id: Uuid::new_v4().to_string(),
            transport: TransportType::Http,
            direction: MessageDirection::Request,
            request_type: method.to_string(),
            jsonrpc_id,
            tenant_id: tenant_id.to_string(),
            device_id: device_id.to_string(),
            spiffe_id: spiffe_id.map(|s| s.to_string()),
            user_id: user_id.map(|s| s.to_string()),
            agent_id: Some("unknown-agent".to_string()),
            server_id: Some("unknown-server".to_string()),
            tool_name,
            resource_uri: None,
            prompt_name: None,
            payload: raw.clone(),
            session: json!({}),
            runtime: json!({ "os": std::env::consts::OS }),
        })
    }

    fn normalize_response(
        &self,
        raw: serde_json::Value,
        tenant_id: &str,
        device_id: &str,
        spiffe_id: Option<&str>,
        user_id: Option<&str>,
    ) -> Result<NormalizedMcpEvent> {
        let jsonrpc_id = raw.get("id").cloned();
        Ok(NormalizedMcpEvent {
            event_id: Uuid::new_v4().to_string(),
            transport: TransportType::Http,
            direction: MessageDirection::Response,
            request_type: "unknown".to_string(), // Inferred by correlating jsonrpc_id in reality
            jsonrpc_id,
            tenant_id: tenant_id.to_string(),
            device_id: device_id.to_string(),
            spiffe_id: spiffe_id.map(|s| s.to_string()),
            user_id: user_id.map(|s| s.to_string()),
            agent_id: None,
            server_id: None,
            tool_name: None,
            resource_uri: None,
            prompt_name: None,
            payload: raw.clone(),
            session: json!({}),
            runtime: json!({ "os": std::env::consts::OS }),
        })
    }
}
