// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use clap::Parser;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long)]
    tenant: String,

    #[arg(long)]
    agent_id: String,

    #[arg(long, default_value = "http://127.0.0.1:43890")]
    lcp_endpoint: String,

    #[arg(long)]
    target_cmd: String,

    #[arg(trailing_var_arg = true)]
    target_args: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    tracing::info!(
        "Starting wrapper for agent {} (tenant {}). Target: {} {:?}",
        args.agent_id,
        args.tenant,
        args.target_cmd,
        args.target_args
    );

    let mut child = Command::new(&args.target_cmd)
        .args(&args.target_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let mut child_stdin = child.stdin.take().expect("Failed to open child stdin");
    let child_stdout = child.stdout.take().expect("Failed to open child stdout");

    // Thread to read from wrapper stdin and write to child stdin (intercepting)
    let tenant = args.tenant.clone();
    let agent_id = args.agent_id.clone();
    let lcp_endpoint = args.lcp_endpoint.clone();

    let stdin_task = tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        let client = reqwest::Client::new();

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await.unwrap_or(0);
            if bytes_read == 0 {
                break; // EOF
            }

            // Attempt to parse JSON-RPC
            let mut allow = true;
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                // Send to LCP for evaluation
                let req_payload = serde_json::json!({
                    "agent_id": agent_id,
                    "protocol": "mcp-stdio",
                    "payload": json
                });

                let url = format!("{}/v1/tenants/{}/pdp/routes/execute", lcp_endpoint, tenant);
                match client.post(&url).json(&req_payload).send().await {
                    Ok(resp) => {
                        if let Ok(decision) = resp.json::<serde_json::Value>().await {
                            if decision["decision"] == "Deny" {
                                allow = false;

                                // Inject JSON-RPC error back to caller
                                let msg_id = json["id"].clone();
                                let err_resp = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": msg_id,
                                    "error": {
                                        "code": -32000,
                                        "message": format!("Access Denied by DEK: {}", decision["reason"].as_str().unwrap_or("Policy Violation"))
                                    }
                                });
                                let mut err_str = serde_json::to_string(&err_resp).unwrap();
                                err_str.push('\n');
                                let mut stdout = tokio::io::stdout();
                                let _ = stdout.write_all(err_str.as_bytes()).await;
                                let _ = stdout.flush().await;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to reach LCP PDP Router: {}", e);
                        // Default deny on failure to reach control plane?
                        // Actually, the route itself has failure_behavior, but if LCP is down...
                        // We will allow for resilience, but log a warning.
                    }
                }
            }

            if allow {
                if let Err(e) = child_stdin.write_all(line.as_bytes()).await {
                    tracing::error!("Failed to write to child stdin: {}", e);
                    break;
                }
                let _ = child_stdin.flush().await;
            }
        }
    });

    // Thread to read from child stdout and write to wrapper stdout (bypass for now, but could intercept to redact)
    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(child_stdout);
        let mut stdout = tokio::io::stdout();
        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await.unwrap_or(0);
            if bytes_read == 0 {
                break; // EOF
            }

            if let Err(e) = stdout.write_all(line.as_bytes()).await {
                tracing::error!("Failed to write to stdout: {}", e);
                break;
            }
            let _ = stdout.flush().await;
        }
    });

    let _ = tokio::try_join!(stdin_task, stdout_task);
    let _ = child.wait().await;

    Ok(())
}
