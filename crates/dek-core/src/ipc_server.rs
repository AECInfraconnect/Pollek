//! ipc_server.rs — local IPC endpoint (health / status / reload).
//!
//! Lifted verbatim from `main.rs::spawn_ipc_server_task`, made `pub`, with the
//! IPC constants moved here. Behaviour is unchanged: bounded concurrency
//! (Semaphore), per-connection span, LinesCodec framing, graceful drain.

use anyhow::Result;
use dek_bundle_sync::BundleSyncAgent;
use dek_ipc::{IpcMessage, IpcRequest, IpcResponse};
use dek_telemetry::CloudTelemetrySink;
use futures::{SinkExt, StreamExt};
use metrics::counter;
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tokio::sync::{RwLock, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout, Duration};
use tokio_util::codec::{Framed, LinesCodec};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn, Instrument};
use uuid::Uuid;

pub const IPC_READ_TIMEOUT_SECS: u64 = 5;
pub const IPC_MAX_LINE_BYTES: usize = 64 * 1024;
pub const IPC_MAX_CONCURRENT_CONNECTIONS: usize = 32;

pub async fn spawn_ipc_server_task(
    cancel_token: CancellationToken,
    ipc_listen_addr: String,
    telemetry_sink: Arc<CloudTelemetrySink>,
    bundle_agent: Arc<BundleSyncAgent>,
    metrics_client: Arc<RwLock<reqwest::Client>>,
    start_time: Instant,
    reload_coordinator: Arc<crate::reload_coordinator::ReloadCoordinator>,
    renew_cfg: crate::svid_renewal::RenewalConfig,
) -> Result<JoinHandle<()>> {
    let socket_addr: std::net::SocketAddr = ipc_listen_addr.parse()?;
    let socket = if socket_addr.is_ipv6() {
        tokio::net::TcpSocket::new_v6()?
    } else {
        tokio::net::TcpSocket::new_v4()?
    };
    let _ = socket.set_reuseaddr(true);
    socket.bind(socket_addr)?;
    let listener = socket.listen(1024)?;
    info!("IPC Endpoint listening on {}", ipc_listen_addr);

    let ipc_semaphore = Arc::new(Semaphore::new(IPC_MAX_CONCURRENT_CONNECTIONS));

    Ok(tokio::spawn(async move {
        let mut ipc_join_set = tokio::task::JoinSet::new();

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("IPC Listener shutting down due to cancellation. Waiting up to 10s for active connections to finish...");
                    break;
                }
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((socket, addr)) => {
                            let permit = match ipc_semaphore.clone().try_acquire_owned() {
                                Ok(p) => p,
                                Err(_) => {
                                    warn!("IPC connection limit reached, rejecting new connection");
                                    counter!("dek_core_ipc_connections_rejected_total").increment(1);
                                    continue;
                                }
                            };

                            debug!("Accepted IPC connection from {}", addr);
                            counter!("dek_core_ipc_connections_total").increment(1);

                            let sink_clone = telemetry_sink.clone();
                            let sync_agent_clone = bundle_agent.clone();
                            let metrics_client_clone = metrics_client.clone();
                            let reload_coordinator_clone = reload_coordinator.clone();
                            let renew_cfg_clone = renew_cfg.clone();

                            ipc_join_set.spawn({
                                let req_id = Uuid::new_v4();
                                let span = tracing::info_span!(
                                    "ipc_connection",
                                    request_id = %req_id,
                                    remote_addr = %addr
                                );

                                async move {
                                    let _permit = permit; // Drop when handler finishes -> slot returned
                                    let start = Instant::now();
                                    info!("Handling IPC connection");

                                    let codec = LinesCodec::new_with_max_length(IPC_MAX_LINE_BYTES);
                                    let mut framed = Framed::new(socket, codec);

                                    if let Ok(Some(Ok(line))) = timeout(Duration::from_secs(IPC_READ_TIMEOUT_SECS), framed.next()).await {
                                        if let Ok(req_msg) = serde_json::from_str::<IpcMessage<IpcRequest>>(&line) {
                                            if !req_msg.version.starts_with("1.") {
                                                let err_msg = IpcMessage {
                                                    version: "1.0".to_string(),
                                                    payload: IpcResponse::Error(format!("Unsupported IPC version: {}", req_msg.version)),
                                                };
                                                if let Ok(err_str) = serde_json::to_string(&err_msg) {
                                                    let _ = framed.send(err_str).await;
                                                }
                                                return;
                                            }
                                            info!("Received IPC Request version {}: {:?}", req_msg.version, req_msg.payload);
                                            let res = match req_msg.payload {
                                                IpcRequest::HealthCheck => IpcResponse::HealthStatus {
                                                    status: "HEALTHY".to_string(),
                                                    core_version: "0.1.0".to_string(),
                                                },
                                                IpcRequest::Status => {
                                                    let uptime = start_time.elapsed().as_secs();
                                                    let bundle_path = dek_config::paths::get_active_bundle_path();
                                                    let bundle_version = std::fs::read_to_string(&bundle_path)
                                                        .ok()
                                                        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                                                        .and_then(|v| v.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()));
                                                    let ebpf_active = cfg!(target_os = "linux");
                                                    let update_state = "IDLE".to_string();
                                                    IpcResponse::ServiceStatus(dek_ipc::ServiceStatus {
                                                        uptime_seconds: uptime,
                                                        ebpf_active,
                                                        active_bundle_version: bundle_version,
                                                        update_state,
                                                        core_version: "0.1.0".to_string(),
                                                    })
                                                },
                                                IpcRequest::ReloadConfig => {
                                                    info!("Received ReloadConfig IPC command. Triggering unified sync pipeline...");
                                                    match sync_agent_clone.run_pipeline().await {
                                                        Ok((new_config, staged_path)) => {
                                                            let mut success = true;
                                                            if let Err(e) = reload_coordinator_clone.process_staged_bundle(&new_config, &staged_path).await {
                                                                error!("Bundle Activation Failed via IPC: {}", e);
                                                                success = false;
                                                            }
                                                            if let Err(e) = sink_clone.update_mtls(&new_config.mtls).await {
                                                                error!("Failed to update Telemetry Sink mTLS: {}", e);
                                                            }
                                                            if let Err(e) = sync_agent_clone.update_mtls(&new_config.mtls).await {
                                                                error!("Failed to update Bundle Sync mTLS: {}", e);
                                                            }
                                                            match new_config.mtls.build_client(None) {
                                                                Ok(c) => {
                                                                    let mut mc = metrics_client_clone.write().await;
                                                                    *mc = c;
                                                                }
                                                                Err(e) => {
                                                                    error!("Failed to build metrics client with new mTLS config: {}", e);
                                                                }
                                                            }
                                                            if success {
                                                                IpcResponse::ReloadStatus { status: "SUCCESS".to_string() }
                                                            } else {
                                                                IpcResponse::ReloadStatus { status: "PARTIAL_FAILURE".to_string() }
                                                            }
                                                        },
                                                        Err(e) => {
                                                            error!("Failed to reload config: {}", e);
                                                            IpcResponse::Error(format!("Reload failed: {}", e))
                                                        }
                                                    }
                                                },
                                                IpcRequest::RotateIdentity => {
                                                    match crate::svid_renewal::force_renew(
                                                        &renew_cfg_clone,
                                                        &sink_clone,
                                                        &sync_agent_clone,
                                                        &metrics_client_clone,
                                                    ).await {
                                                        Ok(id) => IpcResponse::RotateStatus { status: format!("rotated:{id}") },
                                                        Err(e) => IpcResponse::Error(format!("rotation failed: {e}")),
                                                    }
                                                }
                                            };
                                            let res_msg = IpcMessage { version: req_msg.version, payload: res };
                                            if let Ok(res_str) = serde_json::to_string(&res_msg) {
                                                if let Err(e) = framed.send(res_str).await {
                                                    error!("Failed to send IPC response: {}", e);
                                                }
                                            }
                                        } else {
                                            warn!("Failed to parse IPC message from line: {}", line);
                                            sink_clone.emit_async(json!({
                                                "event_type": "pollen.dek.ipc_error",
                                                "error": "parse_failure"
                                            }), dek_telemetry::spooler::Priority::Normal);
                                            let err_msg = IpcMessage {
                                                version: "1.0".to_string(),
                                                payload: IpcResponse::Error("Failed to parse request".to_string()),
                                            };
                                            if let Ok(err_str) = serde_json::to_string(&err_msg) {
                                                let _ = framed.send(err_str).await;
                                            }
                                        }
                                    } else {
                                        warn!("IPC connection read timed out or failed.");
                                        let err_msg = IpcMessage {
                                            version: "1.0".to_string(),
                                            payload: IpcResponse::Error("Request timed out or failed to read".to_string()),
                                        };
                                        if let Ok(err_str) = serde_json::to_string(&err_msg) {
                                            let _ = framed.send(err_str).await;
                                        }
                                    }

                                    let latency = start.elapsed().as_secs_f64();
                                    metrics::histogram!("dek_core_ipc_request_duration_seconds").record(latency);
                                }.instrument(span)
                            });
                        }
                        Err(e) => {
                            error!(error = %e, "IPC accept() failed");
                            counter!("dek_core_ipc_accept_errors_total").increment(1);
                            sleep(Duration::from_millis(100)).await; // brief backpressure
                        }
                    }
                }
            }
        }

        info!("IPC Listener task has exited the loop!");

        match timeout(Duration::from_secs(10), async {
            while let Some(res) = ipc_join_set.join_next().await {
                if let Err(e) = res {
                    warn!("Active IPC task panicked during shutdown: {}", e);
                }
            }
        })
        .await
        {
            Ok(_) => info!("All active IPC connections closed gracefully."),
            Err(_) => warn!(
                "Grace period expired! Forcefully terminating remaining active IPC connections."
            ),
        }
    }))
}
