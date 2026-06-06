use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dek_agent_connector::{AgentConfigRewriter, ClaudeDesktopRewriter};
use dek_ipc::{IpcRequest, IpcResponse};
use std::path::PathBuf;

mod service;
use service::{ServiceManager, OsServiceManager};

mod proxy;

use tokio::net::TcpStream;
use tracing::{error, info};

#[derive(Parser)]
#[command(name = "dek-cli", about = "Pollen DEK Command Line Interface")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// IPC Server Host
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// IPC Server Port
    #[arg(long, default_value_t = 43889)]
    port: u16,
}

#[derive(Subcommand)]
enum Commands {
    /// Check health of DEK Core
    Health,
    /// Detailed local status of DEK Core
    Status,
    /// Trigger dynamic configuration reload
    Reload,
    /// Agent configuration commands
    Agent {
        #[command(subcommand)]
        agent_command: AgentCommands,
    },
    /// Manage the DEK background service lifecycle
    Service {
        /// The service action to perform: install, uninstall, start, stop, status
        action: String,
    },
    /// Manage Layer 2 System Proxy Settings (Opt-in redirect)
    Proxy {
        /// action to perform: enable, disable
        action: String,
    },
}

#[derive(Subcommand)]
enum AgentCommands {
    /// Scan for known agent configurations
    Scan,
    /// Rewrite agent configuration to use DEK MCP stdio wrapper
    Rewrite {
        /// Path to the dek-mcp-stdio-wrapper binary
        #[arg(long, default_value = "dek-mcp-stdio-wrapper")]
        wrapper_path: String,
        /// Application data directory (e.g., %APPDATA% on Windows)
        #[arg(long)]
        app_data: Option<String>,
    },
    /// Restore agent configuration from backup
    Restore {
        /// Application data directory
        #[arg(long)]
        app_data: Option<String>,
    },
}

use futures::{SinkExt, StreamExt};
use tokio_util::codec::{Framed, LinesCodec};

async fn send_ipc_request(host: &str, port: u16, req_payload: IpcRequest) -> Result<IpcResponse> {
    let addr = format!("{}:{}", host, port);
    let stream = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("Failed to connect to DEK Core at {}", addr))?;

    let req = dek_ipc::IpcMessage {
        version: "1.0".to_string(),
        payload: req_payload,
    };

    let mut framed = Framed::new(stream, LinesCodec::new_with_max_length(64 * 1024));
    let req_str = serde_json::to_string(&req)?;
    framed.send(req_str).await?;

    if let Some(Ok(line)) = framed.next().await {
        let res_msg: dek_ipc::IpcMessage<IpcResponse> = serde_json::from_str(&line)?;
        Ok(res_msg.payload)
    } else {
        anyhow::bail!("No response received from DEK Core")
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .without_time()
        .with_target(false)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Health => {
            info!("Sending health check request to DEK Core...");
            match send_ipc_request(&cli.host, cli.port, IpcRequest::HealthCheck).await {
                Ok(IpcResponse::HealthStatus {
                    status,
                    core_version,
                }) => {
                    info!("DEK Core Status: {}", status);
                    info!("Core Version: {}", core_version);
                }
                Ok(IpcResponse::Error(e)) => error!("Error from DEK Core: {}", e),
                Ok(_) => error!("Unexpected response from DEK Core"),
                Err(e) => error!("IPC Request Failed: {}", e),
            }
        }
        Commands::Status => {
            info!("Sending status request to DEK Core...");
            match send_ipc_request(&cli.host, cli.port, IpcRequest::Status).await {
                Ok(IpcResponse::ServiceStatus(status)) => {
                    info!("--- DEK Core Service Status ---");
                    info!("Core Version: {}", status.core_version);
                    info!("Uptime (seconds): {}", status.uptime_seconds);
                    info!("eBPF Active: {}", status.ebpf_active);
                    info!("Active Bundle: {}", status.active_bundle_version.unwrap_or_else(|| "None".to_string()));
                    info!("Update State: {}", status.update_state);
                    info!("-------------------------------");
                }
                Ok(IpcResponse::Error(e)) => error!("Error from DEK Core: {}", e),
                Ok(_) => error!("Unexpected response from DEK Core"),
                Err(e) => error!("IPC Request Failed: {}", e),
            }
        }
        Commands::Reload => {
            info!("Sending ReloadConfig request to DEK Core...");
            match send_ipc_request(&cli.host, cli.port, IpcRequest::ReloadConfig).await {
                Ok(IpcResponse::ReloadStatus { status }) => {
                    info!("DEK Core Reload Status: {}", status);
                }
                Ok(IpcResponse::Error(e)) => error!("Error from DEK Core: {}", e),
                Ok(_) => error!("Unexpected response from DEK Core"),
                Err(e) => error!("IPC Request Failed: {}", e),
            }
        }
        Commands::Agent { agent_command } => {
            let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
            match agent_command {
                AgentCommands::Scan => {
                    let rewriter =
                        ClaudeDesktopRewriter::new(PathBuf::from(app_data), PathBuf::from("dummy"));
                    let agents = rewriter.scan()?;
                    info!("Found {} agent configurations:", agents.len());
                    for agent in agents {
                        info!("- {}: {:?}", agent.agent_id, agent.path);
                    }
                }
                AgentCommands::Rewrite {
                    wrapper_path,
                    app_data: override_app_data,
                } => {
                    let data_dir = override_app_data.unwrap_or(app_data);
                    let rewriter = ClaudeDesktopRewriter::new(
                        PathBuf::from(data_dir),
                        PathBuf::from(wrapper_path),
                    );
                    let agents = rewriter.scan()?;

                    if agents.is_empty() {
                        info!("No agent configs found.");
                        return Ok(());
                    }

                    for agent in agents {
                        info!("Planning rewrite for {}...", agent.agent_id);
                        let plan = rewriter.plan_rewrite(&agent)?;
                        let report = rewriter.apply_rewrite(plan)?;
                        info!("Success! Backup saved to: {:?}", report.backup_path);
                    }
                }
                AgentCommands::Restore {
                    app_data: override_app_data,
                } => {
                    let data_dir = override_app_data.unwrap_or(app_data);
                    let rewriter =
                        ClaudeDesktopRewriter::new(PathBuf::from(data_dir), PathBuf::from("dummy"));
                    match rewriter.restore("claude-desktop") {
                        Ok(_) => {
                            info!("Successfully restored claude-desktop config from backup.")
                        }
                        Err(e) => error!("Failed to restore: {}", e),
                    }
                }
            }
        }
        Commands::Service { action } => {
            let manager = OsServiceManager::new();
            match action.as_str() {
                "install" => {
                    info!("Installing Pollen DEK service...");
                    manager.install()?;
                    info!("Service installed successfully.");
                }
                "uninstall" => {
                    info!("Uninstalling Pollen DEK service...");
                    manager.uninstall()?;
                    info!("Service uninstalled successfully.");
                }
                "start" => {
                    info!("Starting Pollen DEK service...");
                    manager.start()?;
                    info!("Service started.");
                }
                "stop" => {
                    info!("Stopping Pollen DEK service...");
                    manager.stop()?;
                    info!("Service stopped.");
                }
                "status" => {
                    match manager.status() {
                        Ok(s) => info!("Service Status:\n{}", s),
                        Err(e) => error!("Failed to get status: {}", e),
                    }
                }
                _ => {
                    error!("Unknown service action: {}. Valid actions: install, uninstall, start, stop, status", action);
                    std::process::exit(1);
                }
            }
        }
        Commands::Proxy { action } => {
            match action.as_str() {
                "enable" => {
                    info!("Enabling Layer 2 System Proxy Redirect...");
                    proxy::enable_system_proxy()?;
                    info!("System proxy enabled. Traffic redirected to DEK MCP Proxy.");
                }
                "disable" => {
                    info!("Disabling Layer 2 System Proxy Redirect...");
                    proxy::disable_system_proxy()?;
                    info!("System proxy disabled.");
                }
                _ => {
                    error!("Unknown proxy action: {}. Valid actions: enable, disable", action);
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}
