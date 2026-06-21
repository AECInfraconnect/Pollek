#![allow(clippy::panic)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::Result;
use clap::Parser;
use dek_mcp_normalizer::{MessageDirection, NormalizedMcpEvent, TransportType};
use dek_policy_router::PolicyRouter;
use serde_json::{json, Value};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    server_id: String,

    #[arg(long)]
    agent_id: String,

    #[arg(long)]
    transport: Option<String>,

    #[arg(last = true)]
    command_args: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();

    if args.command_args.is_empty() {
        error!("Error: No command provided to wrap");
        std::process::exit(1);
    }

    info!(
        "dek-stdio-wrapper starting. Server ID: {}, Agent ID: {}",
        args.server_id, args.agent_id
    );

    // Load Bootstrap and Config
    use dek_config::{BootstrapConfig, MtlsConfig};
    let bootstrap =
        BootstrapConfig::load_or_default("bootstrap.json").unwrap_or_else(|_| BootstrapConfig {
            device_id: "local-device".into(),
            mtls: MtlsConfig {
                client_cert_path: "certs/client.crt".to_string(),
                client_key_path: "certs/client.key".to_string(),
                root_ca_path: "certs/root_ca.crt".to_string(),
            },
            pinned_bundle_public_key: "".to_string(),
            cloud_url: String::new(),
            spiffe_id: None,
            tenant_id: None,
            local_api_token: None,
        });

    let mut tenant_id = "default-tenant".to_string();
    let mut spiffe_id: Option<String> = None;

    // Setup Adaptive Policy Pipeline
    let mut router = PolicyRouter::new();
    let bundle_path_buf = dek_config::paths::get_active_bundle_path();
    let staged_path = std::path::Path::new(&bundle_path_buf);
    if staged_path.exists() {
        if let Ok(content) = std::fs::read_to_string(staged_path) {
            if let Ok(payload) = serde_json::from_str::<Value>(&content) {
                info!("Loading dynamic policy evaluator configuration from active_bundle.json");
                dek_router_builder::load_router_config(&mut router, &payload);

                if let Some(t) = payload.get("tenant_id").and_then(|v| v.as_str()) {
                    tenant_id = t.to_string();
                }
                if let Some(s) = payload.get("spiffe_id").and_then(|v| v.as_str()) {
                    spiffe_id = Some(s.to_string());
                }
            }
        }
    }

    let router = Arc::new(RwLock::new(router));

    // Init telemetry
    let telemetry_db = dek_config::paths::get_data_dir().join("telemetry-stdio.db");
    let telemetry_sink = dek_telemetry::CloudTelemetrySink::new(
        "https://telemetry.pollen-cloud.internal",
        &bootstrap.mtls,
        None,
        &telemetry_db.to_string_lossy(),
        None,
    )
    .ok();

    let mut cmd = Command::new(&args.command_args[0]);
    cmd.args(&args.command_args[1..]);

    // Inject opt-in proxy redirect environment variables
    cmd.env("HTTP_PROXY", "http://127.0.0.1:43890");
    cmd.env("HTTPS_PROXY", "http://127.0.0.1:43890");

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn()?;

    let mut child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to open child stdin"))?;
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to open child stdout"))?;
    let child_stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to open child stderr"))?;

    // Parent streams
    let mut parent_stdin = BufReader::new(tokio::io::stdin()).lines();
    let mut parent_stdout = tokio::io::stdout();

    let (tx_out, mut rx_out) = mpsc::channel::<String>(100);

    // Task 1: Read child stderr and pipe to our stderr
    let mut child_stderr_reader = BufReader::new(child_stderr).lines();
    tokio::spawn(async move {
        while let Ok(Some(line)) = child_stderr_reader.next_line().await {
            info!("[child stderr] {}", line);
        }
    });

    // Initialize PluginHost for redaction
    let mut plugin_paths = std::collections::HashMap::new();
    let base_dir = dek_config::paths::get_plugin_dir()
        .to_string_lossy()
        .into_owned();
    let paths_to_try = vec![
        format!("{}/pii_redactor.wasm", base_dir),
        "target/wasm32-wasip1/release/pii_redactor.wasm".to_string(),
        "target/wasm32-wasip1/debug/pii_redactor.wasm".to_string(),
    ];
    for p in paths_to_try {
        if std::path::Path::new(&p).exists() {
            plugin_paths.insert("pii-redactor".to_string(), p.to_string());
            break;
        }
    }

    let plugin_host = Arc::new(dek_wasm_host::WasmPluginHost::new(dek_wasm_host::WasmHostConfig::default())
        .unwrap_or_else(|_| panic!("Failed to create WasmPluginHost")));

    for (name, p) in plugin_paths {
        if let Ok(bytes) = std::fs::read(&p) {
            let key = dek_wasm_host::plugin_key::PluginKey {
                tenant_id: "system".into(),
                plugin_id: name.clone(),
                version: "1.0.0".into(),
                abi_version: "1".into(),
                wasm_sha256: "dummy".into(),
            };
            if let Err(e) = plugin_host.load_plugin(key, &bytes).await {
                warn!("Failed to load plugin {}: {}", p, e);
            }
        }
    }

    // Task 2: Read child stdout and pipe to our stdout
    let mut child_stdout_reader = BufReader::new(child_stdout).lines();
    let tx_out_clone = tx_out.clone();
    let plugin_host_clone = plugin_host.clone();
    tokio::spawn(async move {
        while let Ok(Some(line)) = child_stdout_reader.next_line().await {
            if let Ok(mut payload) = serde_json::from_str::<Value>(&line) {
                // Determine if we need to redact. For Phase 4, we assume redaction is an obligation.
                // In a full impl, we'd check `decision.obligations`. We will just run it if loaded.
                let pool_key = "system:pii-redactor:1.0.0:dummy";
                let input_bytes = serde_json::to_vec(&payload).unwrap_or_default();
                if let Ok(redacted_bytes) = plugin_host_clone.invoke(pool_key, "auto".into(), &input_bytes).await {
                    if let Ok(redacted) = serde_json::from_slice(&redacted_bytes) {
                        payload = redacted;
                    }
                }
                let _ = tx_out_clone.send(payload.to_string()).await;
            } else {
                // Forward unmodified if not JSON
                let _ = tx_out_clone.send(line).await;
            }
        }
    });

    // Task 3: Read parent stdin, intercept, and optionally pipe to child stdin
    let agent_id = args.agent_id.clone();
    let server_id = args.server_id.clone();
    tokio::spawn(async move {
        while let Ok(Some(line)) = parent_stdin.next_line().await {
            info!("[wrapper] Intercepted Request: {}", line);

            if let Ok(payload) = serde_json::from_str::<Value>(&line) {
                // Determine method for policy router
                let method = payload
                    .get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                // Create normalized event shape
                let normalized = NormalizedMcpEvent {
                    event_id: Uuid::new_v4().to_string(),
                    transport: TransportType::Stdio,
                    direction: MessageDirection::Request,
                    request_type: method.to_string(),
                    jsonrpc_id: payload.get("id").cloned(),
                    tenant_id: tenant_id.clone(),
                    device_id: bootstrap.device_id.clone(),
                    spiffe_id: spiffe_id.clone(),
                    user_id: Some(agent_id.clone()),
                    agent_id: Some(agent_id.clone()),
                    server_id: Some(server_id.clone()),
                    tool_name: payload
                        .get("params")
                        .and_then(|p| p.get("name"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    resource_uri: None,
                    prompt_name: None,
                    payload: payload.clone(),
                    session: json!({}),
                    runtime: json!({ "os": std::env::consts::OS }),
                };

                let mut policy_input = serde_json::to_value(&normalized).unwrap_or(json!({}));
                // Mock legacy fields
                policy_input["action"] = json!(normalized
                    .tool_name
                    .clone()
                    .unwrap_or(normalized.request_type.clone()));
                policy_input["principal"] = json!(agent_id.clone());
                policy_input["resource"] = json!(server_id.clone());

                let decision_req = dek_decision::DecisionRequest {
                    request_id: Uuid::new_v4().to_string(),
                    trace_id: None,
                    tenant_id: tenant_id.clone(),
                    device_id: bootstrap.device_id.clone(),
                    principal: dek_decision::Principal {
                        id: agent_id.clone(),
                        roles: vec![],
                    },
                    agent: None,
                    action: normalized
                        .tool_name
                        .clone()
                        .unwrap_or(normalized.request_type.clone()),
                    resource: dek_decision::ResourceRef {
                        resource_type: "mcp_tool".into(),
                        resource_id: server_id.clone(),
                        uri: None,
                    },
                    context: policy_input.clone(),
                    input_hash: "mock_hash".into(),
                };

                let decision_input = serde_json::to_value(&decision_req).unwrap_or(policy_input);

                let _start_time = std::time::Instant::now();
                let decision = router
                    .read()
                    .await
                    .authorize(decision_input)
                    .await
                    .unwrap_or_else(|_| dek_policy_runtime::PolicyDecision {
                        evaluator_id: "wrapper".into(),
                        evaluator_type: "wrapper".into(),
                        required: true,
                        status: "error".into(),
                        decision: "deny".into(),
                        allow: false,
                        reason: "Policy evaluation failed".into(),
                        effects: json!({}),
                        obligations: vec![],
                        metadata: json!({}),
                    });

                if let Some(telemetry) = &telemetry_sink {
                    let event = json!({
                        "schema_version": "1.0",
                        "event_type": "decision_log",
                        "device_id": bootstrap.device_id.clone(),
                        "tenant_id": tenant_id.clone(),
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                        "mcp": {
                            "principal": agent_id.clone(),
                            "tool": normalized.tool_name.clone().unwrap_or_default(),
                            "method": normalized.request_type.clone(),
                            "verdict": if decision.allow { "allow" } else { "deny" },
                            "reason": decision.reason.clone(),
                            "request_id": decision_req.request_id.clone(),
                        }
                    });
                    telemetry.emit_async(event, dek_telemetry::spooler::Priority::Normal);
                }

                if !decision.allow {
                    warn!("[wrapper] Denied: {}", decision.reason);

                    let err_res = json!({
                        "jsonrpc": "2.0",
                        "id": payload.get("id").unwrap_or(&json!(null)),
                        "error": {
                            "code": -32001,
                            "message": "Pollen policy denied MCP request",
                            "data": {
                                "reason": decision.reason
                            }
                        }
                    });

                    let _ = tx_out.send(err_res.to_string()).await;
                    continue; // Skip writing to child
                }
            }

            // Allowed or unparseable JSON (let child handle errors)
            let mut l = line;
            l.push('\n');
            if child_stdin.write_all(l.as_bytes()).await.is_err() {
                break;
            }
        }
    });

    // Task 4: Write all output to parent stdout
    while let Some(mut output) = rx_out.recv().await {
        output.push('\n');
        if parent_stdout.write_all(output.as_bytes()).await.is_err() {
            break;
        }
    }

    let status = child.wait().await?;
    info!("dek-stdio-wrapper exiting with status: {}", status);

    Ok(())
}
