//! Bounded, read-only retrieval of MCP capability metadata over the
//! Streamable HTTP transport (loopback endpoints only).
//!
//! This module intentionally only ever sends `initialize`,
//! `notifications/initialized`, `tools/list`, `resources/list`, and
//! `prompts/list`. It must never send `tools/call`, `resources/read`,
//! `prompts/get`, or any other method that could execute a tool or read
//! resource/prompt content.

use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;

const PROBE_TIMEOUT: Duration = Duration::from_millis(1500);
const REQUEST_TIMEOUT: Duration = Duration::from_millis(600);
const MAX_RESPONSE_BYTES: usize = 65_536;
const MAX_ITEMS: usize = 50;
const MCP_SESSION_HEADER: &str = "mcp-session-id";

#[derive(Debug, Clone, Serialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpResource {
    pub uri: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpPrompt {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpCapabilitySnapshot {
    pub server_name: Option<String>,
    pub server_version: Option<String>,
    pub protocol_version: Option<String>,
    pub tools: Vec<McpTool>,
    pub tools_truncated: bool,
    pub resources: Vec<McpResource>,
    pub resources_truncated: bool,
    pub prompts: Vec<McpPrompt>,
    pub prompts_truncated: bool,
}

/// Attempt a bounded MCP capability listing against `url`. Returns `None`
/// on any failure (timeout, non-MCP endpoint, transport error) — callers
/// should treat this as "no live data available" rather than an error.
pub async fn probe_mcp_http_capabilities(
    client: &reqwest::Client,
    url: &str,
) -> Option<McpCapabilitySnapshot> {
    tokio::time::timeout(PROBE_TIMEOUT, probe_inner(client, url))
        .await
        .ok()
        .flatten()
}

async fn probe_inner(client: &reqwest::Client, url: &str) -> Option<McpCapabilitySnapshot> {
    let init_body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {
                "name": "pollek-auto-discovery",
                "version": env!("CARGO_PKG_VERSION"),
            },
        },
    });
    let (init_result, session_id) = send_rpc(client, url, &init_body, None).await?;

    let server_info = init_result.get("serverInfo");
    let server_name = server_info
        .and_then(|v| v.get("name"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let server_version = server_info
        .and_then(|v| v.get("version"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let protocol_version = init_result
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    // Best-effort: some servers require this notification before other
    // calls succeed. It has no response and its failure is not fatal.
    send_notification(
        client,
        url,
        session_id.as_deref(),
        "notifications/initialized",
    )
    .await;

    let (tool_items, tools_truncated) =
        list_items(client, url, session_id.as_deref(), "tools/list", "tools").await;
    let (resource_items, resources_truncated) = list_items(
        client,
        url,
        session_id.as_deref(),
        "resources/list",
        "resources",
    )
    .await;
    let (prompt_items, prompts_truncated) = list_items(
        client,
        url,
        session_id.as_deref(),
        "prompts/list",
        "prompts",
    )
    .await;

    let tools = tool_items
        .into_iter()
        .filter_map(|v| {
            let name = v.get("name")?.as_str()?.to_string();
            Some(McpTool {
                name,
                description: v
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(str::to_string),
                input_schema: v.get("inputSchema").cloned(),
            })
        })
        .collect();

    let resources = resource_items
        .into_iter()
        .filter_map(|v| {
            let uri = v.get("uri")?.as_str()?.to_string();
            Some(McpResource {
                uri,
                name: v.get("name").and_then(|n| n.as_str()).map(str::to_string),
                description: v
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(str::to_string),
                mime_type: v
                    .get("mimeType")
                    .and_then(|m| m.as_str())
                    .map(str::to_string),
            })
        })
        .collect();

    let prompts = prompt_items
        .into_iter()
        .filter_map(|v| {
            let name = v.get("name")?.as_str()?.to_string();
            Some(McpPrompt {
                name,
                description: v
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(str::to_string),
            })
        })
        .collect();

    Some(McpCapabilitySnapshot {
        server_name,
        server_version,
        protocol_version,
        tools,
        tools_truncated,
        resources,
        resources_truncated,
        prompts,
        prompts_truncated,
    })
}

async fn list_items(
    client: &reqwest::Client,
    url: &str,
    session_id: Option<&str>,
    method: &str,
    result_key: &str,
) -> (Vec<Value>, bool) {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": method,
        "params": {},
    });
    let Some((result, _)) = send_rpc(client, url, &body, session_id).await else {
        return (Vec::new(), false);
    };
    let items = result
        .get(result_key)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let truncated = items.len() > MAX_ITEMS;
    (items.into_iter().take(MAX_ITEMS).collect(), truncated)
}

/// Sends one JSON-RPC request and returns its `result` value along with the
/// session id to use for subsequent calls (either newly issued or carried
/// forward from `session_id`).
async fn send_rpc(
    client: &reqwest::Client,
    url: &str,
    body: &Value,
    session_id: Option<&str>,
) -> Option<(Value, Option<String>)> {
    let mut req = client
        .post(url)
        .timeout(REQUEST_TIMEOUT)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(
            reqwest::header::ACCEPT,
            "application/json, text/event-stream",
        )
        .json(body);
    if let Some(sid) = session_id {
        req = req.header(MCP_SESSION_HEADER, sid);
    }

    let res = req.send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    let new_session_id = res
        .headers()
        .get(MCP_SESSION_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .or_else(|| session_id.map(str::to_string));
    let content_type = res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let text = res.text().await.ok()?;
    if text.len() > MAX_RESPONSE_BYTES {
        return None;
    }

    let payload = if content_type.contains("text/event-stream") {
        extract_sse_json(&text)?
    } else {
        serde_json::from_str::<Value>(&text).ok()?
    };

    if payload.get("error").is_some() {
        return None;
    }
    let result = payload.get("result")?.clone();
    Some((result, new_session_id))
}

/// Best-effort JSON-RPC notification. No response is expected or awaited
/// beyond the request itself, and failures are ignored.
async fn send_notification(
    client: &reqwest::Client,
    url: &str,
    session_id: Option<&str>,
    method: &str,
) {
    let body = json!({ "jsonrpc": "2.0", "method": method });
    let mut req = client
        .post(url)
        .timeout(REQUEST_TIMEOUT)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(
            reqwest::header::ACCEPT,
            "application/json, text/event-stream",
        )
        .json(&body);
    if let Some(sid) = session_id {
        req = req.header(MCP_SESSION_HEADER, sid);
    }
    let _ = req.send().await;
}

fn extract_sse_json(body: &str) -> Option<Value> {
    for line in body.lines() {
        if let Some(data) = line.strip_prefix("data:") {
            if let Ok(v) = serde_json::from_str::<Value>(data.trim()) {
                return Some(v);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    fn rpc_response(id: i64, result: Value) -> ResponseTemplate {
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/json")
            .set_body_json(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
    }

    fn method_of(req: &Request) -> String {
        serde_json::from_slice::<Value>(&req.body)
            .ok()
            .and_then(|v| v.get("method").and_then(|m| m.as_str()).map(str::to_string))
            .unwrap_or_default()
    }

    #[tokio::test]
    async fn retrieves_tools_resources_and_prompts() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/mcp"))
            .respond_with(move |req: &Request| match method_of(req).as_str() {
                "initialize" => rpc_response(
                    1,
                    json!({
                        "protocolVersion": "2025-03-26",
                        "serverInfo": {"name": "demo-mcp", "version": "1.2.3"},
                        "capabilities": {},
                    }),
                )
                .insert_header("mcp-session-id", "sess-abc"),
                "notifications/initialized" => ResponseTemplate::new(202),
                "tools/list" => rpc_response(
                    2,
                    json!({"tools": [
                        {"name": "search", "description": "Search things", "inputSchema": {"type": "object"}}
                    ]}),
                ),
                "resources/list" => rpc_response(
                    2,
                    json!({"resources": [
                        {"uri": "file:///demo.txt", "name": "demo", "mimeType": "text/plain"}
                    ]}),
                ),
                "prompts/list" => rpc_response(2, json!({"prompts": [{"name": "greet"}]})),
                _ => ResponseTemplate::new(400),
            })
            .expect(5) // wiremock expected-call count, not Option::expect
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/mcp", server.uri());
        let snapshot = probe_mcp_http_capabilities(&client, &url).await;
        assert!(snapshot.is_some(), "expected a capability snapshot");
        let Some(snapshot) = snapshot else {
            return;
        };

        assert_eq!(snapshot.server_name.as_deref(), Some("demo-mcp"));
        assert_eq!(snapshot.server_version.as_deref(), Some("1.2.3"));
        assert_eq!(snapshot.tools.len(), 1);
        assert_eq!(snapshot.tools[0].name, "search");
        assert!(snapshot.tools[0].input_schema.is_some());
        assert_eq!(snapshot.resources.len(), 1);
        assert_eq!(snapshot.resources[0].uri, "file:///demo.txt");
        assert_eq!(snapshot.prompts.len(), 1);
        assert_eq!(snapshot.prompts[0].name, "greet");
        assert!(!snapshot.tools_truncated);
    }

    #[tokio::test]
    async fn never_sends_tool_invocation_or_resource_read_methods() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(move |req: &Request| {
                let m = method_of(req);
                assert_ne!(m, "tools/call", "must never invoke a tool");
                assert_ne!(m, "resources/read", "must never read resource content");
                assert_ne!(m, "prompts/get", "must never fetch prompt content");
                match m.as_str() {
                    "initialize" => rpc_response(
                        1,
                        json!({"serverInfo": {"name": "demo"}, "protocolVersion": "2025-03-26"}),
                    ),
                    "notifications/initialized" => ResponseTemplate::new(202),
                    "tools/list" => rpc_response(2, json!({"tools": []})),
                    "resources/list" => rpc_response(2, json!({"resources": []})),
                    "prompts/list" => rpc_response(2, json!({"prompts": []})),
                    _ => ResponseTemplate::new(400),
                }
            })
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/mcp", server.uri());
        let snapshot = probe_mcp_http_capabilities(&client, &url).await;
        assert!(snapshot.is_some());
    }

    #[tokio::test]
    async fn returns_none_when_server_errors_on_initialize() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/mcp", server.uri());
        assert!(probe_mcp_http_capabilities(&client, &url).await.is_none());
    }

    #[tokio::test]
    async fn caps_item_count_and_flags_truncation() {
        let server = MockServer::start().await;
        let many_tools: Vec<Value> = (0..75)
            .map(|i| json!({"name": format!("tool_{i}")}))
            .collect();

        Mock::given(method("POST"))
            .respond_with(move |req: &Request| match method_of(req).as_str() {
                "initialize" => rpc_response(
                    1,
                    json!({"serverInfo": {"name": "demo"}, "protocolVersion": "2025-03-26"}),
                ),
                "notifications/initialized" => ResponseTemplate::new(202),
                "tools/list" => rpc_response(2, json!({"tools": many_tools})),
                "resources/list" => rpc_response(2, json!({"resources": []})),
                "prompts/list" => rpc_response(2, json!({"prompts": []})),
                _ => ResponseTemplate::new(400),
            })
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let url = format!("{}/mcp", server.uri());
        let snapshot = probe_mcp_http_capabilities(&client, &url).await;
        assert!(snapshot.is_some(), "expected a capability snapshot");
        let Some(snapshot) = snapshot else {
            return;
        };
        assert_eq!(snapshot.tools.len(), MAX_ITEMS);
        assert!(snapshot.tools_truncated);
    }
}
