// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::print_stdout, clippy::print_stderr)]
// Copyright (c) 2026 AEC Infraconnect

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dek_agent_connector::{AgentConfigRewriter, ClaudeDesktopRewriter};
use dek_ipc::{IpcRequest, IpcResponse};
use std::path::PathBuf;

mod service;
use service::{OsServiceManager, ServiceManager};

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
    /// Update Pollen DEK (Proxy to dek-updater)
    Update {
        #[arg(long, default_value = "beta")]
        channel: String,
    },
    /// Enroll device
    Enroll {
        #[arg(long)]
        cloud_url: String,
    },
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
    /// Trigger an emergency rollback of the Pollen DEK Core
    Rollback,
    /// Unenroll device (removes local identity and config)
    Unenroll {
        #[arg(long, help = "Wipe all local secrets from the platform keystore")]
        wipe_local_secrets: bool,
    },
    /// Revoke device identity on Cloud and remove locally
    RevokeLocal {
        #[arg(long)]
        reason: String,
    },
    /// Check system configuration and permissions
    Doctor,
    /// Repair bootstrap.json using data from the secure keystore
    RepairBootstrap,
    /// Export logs and state for troubleshooting (redacts secrets)
    ExportDiagnostics {
        #[arg(long, default_value_t = true)]
        redact: bool,
    },
    /// Rotate device identity manually
    RotateIdentity,
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
    /// Print DEK capabilities matrix
    Capabilities,
    /// Switch control-plane profile (local <-> cloud)
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Manage Fingerprint Definitions
    Fingerprint {
        #[command(subcommand)]
        action: FingerprintCommands,
    },
}

#[derive(Subcommand)]
pub enum FingerprintCommands {
    /// Update definitions from cloud
    Update,
    /// View active definition status
    Status,
    /// Rollback definition to previous version
    Rollback {
        /// Version to rollback to (optional)
        #[arg(long)]
        version: Option<String>,
    },
    /// Import definition offline
    Import {
        #[arg(long)]
        file: String,
        #[arg(long)]
        sig: String,
    },
}

#[derive(Subcommand)]
enum ProfileAction {
    /// Set profile: local (default url http://127.0.0.1:3000) or cloud
    Set {
        mode: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        tenant_id: Option<String>,
        #[arg(long)]
        trusted_key: Option<String>,
        #[arg(long)]
        token: Option<String>,
    },
    /// Show current profile
    Show,
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
        Commands::Update { channel } => {
            info!(
                "Initiating update via dek-updater (channel: {})...",
                channel
            );
            let exe_path = std::env::current_exe()?;
            let updater_dir = exe_path.parent().unwrap_or(std::path::Path::new("."));
            let updater_exe = if cfg!(windows) {
                updater_dir.join("dek-updater.exe")
            } else {
                updater_dir.join("dek-updater")
            };

            if !updater_exe.exists() {
                error!("Updater not found at {:?}", updater_exe);
                std::process::exit(1);
            }

            let status = std::process::Command::new(&updater_exe)
                .arg("upgrade")
                .arg("--channel")
                .arg(&channel)
                .status()?;

            if !status.success() {
                error!("Update failed.");
                std::process::exit(1);
            }
            info!("Update successful.");
        }
        Commands::Enroll { cloud_url } => {
            service::enroll::run(&cloud_url).await?;
        }
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
                    info!(
                        "Active Bundle: {}",
                        status
                            .active_bundle_version
                            .unwrap_or_else(|| "None".to_string())
                    );
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
        Commands::Rollback => {
            if let Err(e) = service::rollback::run() {
                error!("Rollback failed: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Unenroll { wipe_local_secrets } => {
            if let Err(e) = service::unenroll::run(wipe_local_secrets) {
                error!("Unenroll failed: {}", e);
                std::process::exit(1);
            }
        }
        Commands::RevokeLocal { reason } => {
            if let Err(e) = service::revoke::run(&reason).await {
                error!("Revoke failed: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Doctor => {
            if let Err(e) = service::doctor::run() {
                error!("Doctor check failed: {}", e);
                std::process::exit(1);
            }
        }
        Commands::RepairBootstrap => {
            if let Err(e) = service::doctor::repair_bootstrap() {
                error!("Repair bootstrap failed: {}", e);
                std::process::exit(1);
            }
        }
        Commands::ExportDiagnostics { redact } => {
            if let Err(e) = service::doctor::export_diagnostics(redact) {
                error!("Export diagnostics failed: {}", e);
                std::process::exit(1);
            }
        }
        Commands::RotateIdentity => {
            if let Err(e) = service::rotate::run(&cli.host, cli.port).await {
                error!("Rotate identity failed: {}", e);
                std::process::exit(1);
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
                "status" => match manager.status() {
                    Ok(s) => info!("Service Status:\n{}", s),
                    Err(e) => error!("Failed to get status: {}", e),
                },
                _ => {
                    error!("Unknown service action: {}. Valid actions: install, uninstall, start, stop, status", action);
                    std::process::exit(1);
                }
            }
        }
        Commands::Proxy { action } => match action.as_str() {
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
                error!(
                    "Unknown proxy action: {}. Valid actions: enable, disable",
                    action
                );
                std::process::exit(1);
            }
        },
        Commands::Capabilities => {
            let caps = serde_json::json!({
                "mcp_http_pep": { "linux": true, "windows": true, "macos": true },
                "mcp_stdio_pep": { "linux": true, "windows": true, "macos": true },
                "network_egress_ebpf": { "linux": true, "windows": false, "macos": false },
                "system_transparent_interception": { "linux": "limited", "windows": false, "macos": false },
                "opt_in_proxy_redirect": { "linux": true, "windows": true, "macos": true },
                "envoy_istio_ext_authz": { "linux": true, "windows": false, "macos": false }
            });
            println!("{}", serde_json::to_string_pretty(&caps)?);
        }
        Commands::Profile { action } => match action {
            ProfileAction::Set {
                mode,
                url,
                tenant_id,
                trusted_key,
                token,
            } => {
                let m = mode.parse::<service::profile::ProfileMode>()?;
                service::profile::set_profile(m, url, tenant_id, trusted_key, token)?;
            }
            ProfileAction::Show => service::profile::show_profile()?,
        },
        Commands::Fingerprint { action } => {
            match action {
                FingerprintCommands::Update => {
                    info!("Triggering fingerprint definition update...");
                    match send_ipc_request(&cli.host, cli.port, IpcRequest::FingerprintAction { action: "update".to_string(), payload: None, sig: None }).await {
                        Ok(IpcResponse::FingerprintStatus { version, message }) => {
                            info!("Update triggered. New version: {} ({})", version, message);
                        }
                        Ok(IpcResponse::Error(e)) => error!("Error: {}", e),
                        _ => error!("Unexpected response"),
                    }
                }
                FingerprintCommands::Status => {
                    info!("Fetching fingerprint definition status...");
                    match send_ipc_request(&cli.host, cli.port, IpcRequest::FingerprintAction { action: "status".to_string(), payload: None, sig: None }).await {
                        Ok(IpcResponse::FingerprintStatus { version, message }) => {
                            info!("Fingerprint Version: {} ({})", version, message);
                        }
                        Ok(IpcResponse::Error(e)) => error!("Error: {}", e),
                        _ => error!("Unexpected response"),
                    }
                }
                FingerprintCommands::Rollback { version: _ } => {
                    info!("Rolling back fingerprint definition...");
                    match send_ipc_request(&cli.host, cli.port, IpcRequest::FingerprintAction { action: "rollback".to_string(), payload: None, sig: None }).await {
                        Ok(IpcResponse::FingerprintStatus { version, message }) => {
                            info!("Rolled back to Version: {} ({})", version, message);
                        }
                        Ok(IpcResponse::Error(e)) => error!("Error: {}", e),
                        _ => error!("Unexpected response"),
                    }
                }
                FingerprintCommands::Import { file, sig } => {
                    info!("Importing fingerprint definition offline...");
                    let payload = std::fs::read(&file).context("failed to read def file")?;
                    let sig_content = std::fs::read_to_string(&sig).context("failed to read sig file")?;
                    
                    match send_ipc_request(&cli.host, cli.port, IpcRequest::FingerprintAction { 
                        action: "import".to_string(), 
                        payload: Some(payload), 
                        sig: Some(sig_content) 
                    }).await {
                        Ok(IpcResponse::FingerprintStatus { version, message }) => {
                            info!("Import successful. Active Version: {} ({})", version, message);
                        }
                        Ok(IpcResponse::Error(e)) => error!("Error: {}", e),
                        _ => error!("Unexpected response"),
                    }
                }
            }
        }
    }
    Ok(())
}
