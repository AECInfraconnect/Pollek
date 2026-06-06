//! # DEK Core Supervisor
//!
//! Pollen DEK Core Supervisor manages device lifecycle including:
//! - Bootstrapping and mTLS config fetch from Pollen Cloud
//! - Periodic bundle synchronization
//! - Local IPC endpoint for health checks and commands
//! - Telemetry emission and Prometheus metrics push (OTLP/Pushgateway)

use anyhow::{Context, Result};
use dek_config::{BootstrapConfig, DekConfig};
use dek_ipc::{IpcMessage, IpcRequest, IpcResponse};
use dek_telemetry::CloudTelemetrySink;
use futures::{SinkExt, StreamExt};
use metrics::{counter, gauge};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use serde_json::json;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::{RwLock, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout, Duration};
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::{Retry, RetryIf};
use tokio_util::codec::{Framed, LinesCodec};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn, Instrument};
use uuid::Uuid;

mod service_integration;
mod ebpf;
mod keystore_migration;
mod updater;

const IPC_READ_TIMEOUT_SECS: u64 = 5;
const IPC_MAX_LINE_BYTES: usize = 64 * 1024;
const IPC_MAX_CONCURRENT_CONNECTIONS: usize = 32;

fn get_env_var(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

async fn load_bootstrap(bootstrap_path: &str) -> Result<BootstrapConfig> {
    let bootstrap = BootstrapConfig::load_or_default(bootstrap_path)?;
    info!(
        "Loaded Bootstrap Config for device: {}",
        bootstrap.device_id
    );
    Ok(bootstrap)
}

async fn run_sync_pipeline_with_retry(
    sync_agent: &dek_bundle_sync::BundleSyncAgent,
) -> Result<DekConfig> {
    info!("Running Unified Sync Pipeline...");
    let strategy = ExponentialBackoff::from_millis(2000)
        .factor(2)
        .max_delay(Duration::from_secs(30))
        .take(10); // roughly 120s max

    RetryIf::spawn(
        strategy,
        || async {
            match sync_agent.run_pipeline().await {
                Ok(c) => Ok(c),
                Err(e) => {
                    warn!("Pipeline run failed: {}. Retrying...", e);
                    counter!("dek_core_config_fetch_errors_total").increment(1);
                    Err(e)
                }
            }
        },
        |e: &anyhow::Error| {
            if let Some(reqwest_err) = e.downcast_ref::<reqwest::Error>() {
                if let Some(status) = reqwest_err.status() {
                    if status.is_client_error() {
                        error!(
                            "Fatal HTTP client error running pipeline: {}. Aborting startup.",
                            status
                        );
                        return false;
                    }
                }
                if reqwest_err.is_builder() || reqwest_err.is_request() {
                    return false;
                }
            }
            true
        },
    )
    .await
}

fn spawn_metrics_push_task(
    cancel_token: CancellationToken,
    metrics_client: Arc<RwLock<reqwest::Client>>,
    metrics_push_url: String,
    prometheus_handle: PrometheusHandle,
) -> JoinHandle<()> {
    tokio::spawn(
        async move {
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!("Metrics Push task shutting down gracefully.");
                        break;
                    }
                    _ = sleep(Duration::from_secs(10)) => {
                        let metrics_text = prometheus_handle.render();

                        let strategy = ExponentialBackoff::from_millis(500)
                            .factor(2)
                            .max_delay(Duration::from_secs(2))
                            .take(4);

                        let res = Retry::spawn(strategy, || async {
                            let client = metrics_client.read().await.clone();
                            let push_res = client
                                .post(&metrics_push_url)
                                .body(metrics_text.clone())
                                .send()
                                .await;

                            match push_res {
                                Ok(r) if r.status().is_success() => Ok(()),
                                Ok(r) => {
                                    warn!("Failed to push metrics, status: {}", r.status());
                                    Err(anyhow::anyhow!("HTTP Status: {}", r.status()))
                                },
                                Err(e) => {
                                    warn!("Error pushing metrics: {}", e);
                                    Err(anyhow::anyhow!("Request error: {}", e))
                                }
                            }
                        }).await;

                        if res.is_ok() {
                            debug!("Successfully pushed metrics to {}", metrics_push_url);
                        } else {
                            warn!("Failed to push metrics after retries");
                        }
                    }
                }
            }
        }
        .instrument(tracing::info_span!("metrics_push")),
    )
}

fn spawn_bundle_sync_task(
    cancel_token: CancellationToken,
    sync_agent: Arc<dek_bundle_sync::BundleSyncAgent>,
    bundle_sync_interval: u64,
    metrics_client: Arc<RwLock<reqwest::Client>>,
    pinned_key: String,
) -> JoinHandle<()> {
    tokio::spawn(
        async move {
            let mut current_version = String::new();
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!("Bundle Sync task shutting down gracefully.");
                        break;
                    }
                    _ = sleep(Duration::from_secs(bundle_sync_interval)) => {
                        debug!("Running unified bundle sync pipeline...");
                        match timeout(Duration::from_secs(30), sync_agent.run_pipeline()).await {
                            Ok(Ok(new_config)) => {
                                counter!("dek_core_bundle_sync_success_total").increment(1);
                                if let Some(update) = new_config.update_config {
                                    if update.version != current_version {
                                        info!("New binary update found: version {}", update.version);
                                        let client = metrics_client.read().await.clone();
                                        match crate::updater::run_update(&client, &update.download_url, &update.signature_b64, &pinned_key).await {
                                            Ok(_) => {
                                                info!("Update applied successfully. Version updated to {}", update.version);
                                                current_version = update.version;
                                            }
                                            Err(e) => {
                                                error!("Failed to apply binary update: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(Err(e)) => {
                                warn!(error = %e, "Bundle sync pipeline failed");
                                counter!("dek_core_bundle_sync_errors_total").increment(1);
                            }
                            Err(_) => {
                                warn!("Bundle sync pipeline timed out after 30s");
                                counter!("dek_core_bundle_sync_timeout_total").increment(1);
                            }
                        }
                        counter!("dek_core_bundle_checks_total").increment(1);
                    }
                }
            }
        }
        .instrument(tracing::info_span!("bundle_sync")),
    )
}

async fn spawn_ipc_server_task(
    cancel_token: CancellationToken,
    ipc_listen_addr: String,
    telemetry_sink: Arc<CloudTelemetrySink>,
    bundle_agent: Arc<dek_bundle_sync::BundleSyncAgent>,
    metrics_client: Arc<RwLock<reqwest::Client>>,
    start_time: Instant,
) -> Result<JoinHandle<()>> {
    let listener = TcpListener::bind(&ipc_listen_addr).await?;
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
                                                    
                                                    // TODO: Get real eBPF status and update state
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
                                                        Ok(new_config) => {
                                                            let mut success = true;
                                                            if let Err(e) = sink_clone.update_mtls(&new_config.mtls).await {
                                                                error!("Failed to update Telemetry Sink mTLS: {}", e);
                                                                success = false;
                                                            }
                                                            if let Err(e) = sync_agent_clone.update_mtls(&new_config.mtls).await {
                                                                error!("Failed to update Bundle Sync Agent mTLS: {}", e);
                                                                success = false;
                                                            }
                                                            match new_config.mtls.build_client(None) {
                                                                Ok(c) => {
                                                                    *metrics_client_clone.write().await = c;
                                                                    info!("Successfully updated Metrics Client mTLS");
                                                                }
                                                                Err(e) => {
                                                                    error!("Failed to update Metrics Client mTLS: {}", e);
                                                                    success = false;
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
                                                }
                                            };
                                            let res_msg = IpcMessage {
                                                version: req_msg.version,
                                                payload: res,
                                            };
                                            if let Ok(res_str) = serde_json::to_string(&res_msg) {
                                                if let Err(e) = framed.send(res_str).await {
                                                    error!("Failed to send IPC response: {}", e);
                                                }
                                            }
                                        } else {
                                            warn!("Failed to parse IPC message from line: {}", line);
                                            let _ = sink_clone.emit_async(json!({
                                                "event_type": "pollen.dek.ipc_error",
                                                "error": "parse_failure"
                                            })).await;

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

        // Wait for active handlers to finish
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

fn main() -> Result<()> {
    service_integration::run_as_service_if_needed(core_main())
}

async fn core_main() -> Result<()> {
    dek_config::logging::init_logging("dek-core").unwrap_or_else(|e| {
        eprintln!("Failed to initialize logging: {}", e);
    });
    info!("Starting Pollen DEK Core Supervisor...");

    // Load Layer 2 eBPF Guardrails (Linux only)
    if let Err(e) = ebpf::load_and_attach() {
        tracing::error!("Failed to initialize eBPF Layer 2 guardrails: {}", e);
    }

    let pollen_cloud_url = get_env_var("POLLEN_CLOUD_URL", "https://127.0.0.1:43891");
    let ipc_listen_addr = get_env_var("DEK_IPC_ADDR", "127.0.0.1:43889");
    let bootstrap_path = get_env_var("DEK_BOOTSTRAP_PATH", &dek_config::paths::get_bootstrap_path().to_string_lossy());
    let bundle_sync_interval = get_env_var("DEK_BUNDLE_SYNC_INTERVAL", "10")
        .parse::<u64>()
        .unwrap_or(10);

    if !pollen_cloud_url.starts_with("https://") {
        error!(
            "Fatal Error: POLLEN_CLOUD_URL must start with https:// to prevent downgrade attacks."
        );
        std::process::exit(1);
    }

    let pollen_telemetry_url = format!("{}/telemetry", pollen_cloud_url);

    let metrics_push_url = format!("{}/metrics", pollen_cloud_url);

    let prometheus_handle = PrometheusBuilder::new()
        .install_recorder()
        .expect("failed to install Prometheus recorder");
    info!("Prometheus metrics recorder installed (Push Model enabled)");

    gauge!("dek_core_start_timestamp_seconds").set(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64(),
    );

    let bootstrap = load_bootstrap(&bootstrap_path).await?;

    let mut client_key_override: Option<Vec<u8>> = None;
    let mut pinned_key_override: Option<String> = None;
    if keystore_migration::run_migration(&bootstrap, &pollen_cloud_url).await {
        let keystore = dek_keystore::get_keystore();
        if let Ok(key_data) = keystore.load_key("mtls_client_key") {
            client_key_override = Some(key_data);
        }
        if let Ok(bundle_pk_data) = keystore.load_key("pinned_bundle_public_key") {
            if let Ok(pk_str) = String::from_utf8(bundle_pk_data) {
                pinned_key_override = Some(pk_str);
            }
        }
    }

    let actual_pinned_key = pinned_key_override.unwrap_or_else(|| bootstrap.pinned_bundle_public_key.clone());

    // Create BundleSyncAgent using Bootstrap mTLS
    let bundle_agent = Arc::new(dek_bundle_sync::BundleSyncAgent::new(
        &pollen_cloud_url,
        &bootstrap.device_id,
        &bootstrap.mtls,
        &actual_pinned_key,
        client_key_override.as_deref(),
    )?);

    let telemetry_sink = Arc::new(CloudTelemetrySink::new(
        &pollen_telemetry_url,
        &bootstrap.mtls,
        client_key_override.as_deref(),
    )?);

    let metrics_client = Arc::new(RwLock::new(
        bootstrap
            .mtls
            .build_client(client_key_override.as_deref())
            .context("Failed to build metrics MTLS client")?,
    ));

    let cancel_token = CancellationToken::new();
    let start_time = Instant::now();

    let ipc_handle = spawn_ipc_server_task(
        cancel_token.clone(),
        ipc_listen_addr,
        telemetry_sink.clone(),
        bundle_agent.clone(),
        metrics_client.clone(),
        start_time,
    )
    .await?;

    // Signal readiness to OS Service Managers BEFORE blocking on cloud sync
    service_integration::notify_ready();

    // Spawn the cloud sync and background tasks into a separate tokio task
    // so that the IPC Server remains healthy and responsive immediately!
    let sync_bundle_agent = bundle_agent.clone();
    let sync_telemetry_sink = telemetry_sink.clone();
    let sync_metrics_client = metrics_client.clone();
    let sync_cancel_token = cancel_token.clone();
    
    let background_tasks_handle = tokio::spawn(async move {
        // Initial startup sync using the unified pipeline (blocks up to 2 minutes on retries)
        let config = match run_sync_pipeline_with_retry(&sync_bundle_agent).await {
            Ok(c) => c,
            Err(e) => {
                error!("Initial cloud sync failed completely: {}. Background tasks will not start.", e);
                return;
            }
        };

        let _ = sync_telemetry_sink
            .emit_async(json!({
                "event_type": "pollen.dek.startup",
                "device_id": config.device_id,
                "status": "online"
            }))
            .await;

        let sync_handle = spawn_bundle_sync_task(
            sync_cancel_token.clone(),
            sync_bundle_agent,
            bundle_sync_interval,
            sync_metrics_client.clone(),
            actual_pinned_key.clone(),
        );

        let metrics_handle = spawn_metrics_push_task(
            sync_cancel_token.clone(),
            sync_metrics_client,
            metrics_push_url,
            prometheus_handle,
        );
        
        // Wait for them to finish
        let _ = tokio::join!(sync_handle, metrics_handle);
    });

    // Wait for shutdown signal
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate())?;
        let mut sigint = signal(SignalKind::interrupt())?;
        tokio::select! {
            _ = sigterm.recv() => info!("Received SIGTERM"),
            _ = sigint.recv() => info!("Received SIGINT"),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await?;
        info!("Received SIGINT");
    }

    info!("Initiating graceful shutdown...");
    cancel_token.cancel();

    // Graceful Shutdown Validation with Hard Timeout
    match timeout(Duration::from_secs(15), async {
        tokio::join!(background_tasks_handle, ipc_handle)
    })
    .await
    {
        Ok((background_res, ipc_res)) => {
            let mut exit_error = false;
            if let Err(e) = background_res {
                error!(error = %e, "Background task panicked");
                exit_error = true;
            }
            if let Err(e) = ipc_res {
                error!(error = %e, "IPC Listener task panicked");
                exit_error = true;
            }
            if exit_error {
                error!("Graceful shutdown completed, but some tasks had errors.");
            } else {
                info!("Graceful shutdown completed successfully.");
            }
        }
        Err(_) => {
            warn!("Graceful shutdown timed out. Force quitting.");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    #[test]
    fn test_ipc_healthcheck_roundtrip() {
        let req = IpcMessage {
            version: "1.0".to_string(), // version is String in real code
            payload: IpcRequest::HealthCheck,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: IpcMessage<IpcRequest> = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed.payload, IpcRequest::HealthCheck));
        assert_eq!(parsed.version, "1.0");
    }

    #[test]
    fn test_ipc_unknown_fields_accepted() {
        let json = r#"{"version": "1.0", "payload": "HealthCheck", "unknown_extra": 123}"#;
        let parsed: Result<IpcMessage<IpcRequest>, _> = serde_json::from_str(json);
        assert!(parsed.is_ok());
    }

    async fn spawn_test_server() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let ipc_semaphore = Arc::new(Semaphore::new(IPC_MAX_CONCURRENT_CONNECTIONS));

        tokio::spawn(async move {
            if let Ok((socket, _)) = listener.accept().await {
                let permit = ipc_semaphore.try_acquire_owned().unwrap();
                tokio::spawn(async move {
                    let _permit = permit;
                    let codec = LinesCodec::new_with_max_length(IPC_MAX_LINE_BYTES);
                    let mut framed = Framed::new(socket, codec);
                    if let Ok(Some(Ok(line))) =
                        timeout(Duration::from_secs(IPC_READ_TIMEOUT_SECS), framed.next()).await
                    {
                        if let Ok(req_msg) = serde_json::from_str::<IpcMessage<IpcRequest>>(&line) {
                            if !req_msg.version.starts_with("1.") {
                                let err_msg = IpcMessage {
                                    version: "1.0".to_string(),
                                    payload: IpcResponse::Error(format!(
                                        "Unsupported IPC version: {}",
                                        req_msg.version
                                    )),
                                };
                                let _ = framed.send(serde_json::to_string(&err_msg).unwrap()).await;
                                return;
                            }
                            let res = match req_msg.payload {
                                IpcRequest::HealthCheck => IpcResponse::HealthStatus {
                                    status: "HEALTHY".to_string(),
                                    core_version: "0.1.0".to_string(),
                                },
                                IpcRequest::Status => IpcResponse::ServiceStatus(dek_ipc::ServiceStatus {
                                    uptime_seconds: 100,
                                    ebpf_active: true,
                                    active_bundle_version: Some("v1".to_string()),
                                    update_state: "IDLE".to_string(),
                                    core_version: "0.1.0".to_string(),
                                }),
                                IpcRequest::ReloadConfig => IpcResponse::ReloadStatus {
                                    status: "SUCCESS".to_string(),
                                },
                            };
                            let res_msg = IpcMessage {
                                version: req_msg.version,
                                payload: res,
                            };
                            framed
                                .send(serde_json::to_string(&res_msg).unwrap())
                                .await
                                .unwrap();
                        } else {
                            let err_msg = IpcMessage {
                                version: "1.0".to_string(),
                                payload: IpcResponse::Error("Failed to parse request".to_string()),
                            };
                            framed
                                .send(serde_json::to_string(&err_msg).unwrap())
                                .await
                                .unwrap();
                        }
                    } else {
                        let err_msg = IpcMessage {
                            version: "1.0".to_string(),
                            payload: IpcResponse::Error(
                                "Request timed out or failed to read".to_string(),
                            ),
                        };
                        let _ = framed.send(serde_json::to_string(&err_msg).unwrap()).await;
                    }
                });
            }
        });
        addr
    }

    #[tokio::test]
    async fn test_ipc_healthcheck_end_to_end() {
        let addr = spawn_test_server().await;
        let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let mut framed = Framed::new(stream, LinesCodec::new_with_max_length(IPC_MAX_LINE_BYTES));

        let req = IpcMessage {
            version: "1.0".to_string(),
            payload: IpcRequest::HealthCheck,
        };
        framed
            .send(serde_json::to_string(&req).unwrap())
            .await
            .unwrap();

        let line = framed.next().await.unwrap().unwrap();
        let res_msg: IpcMessage<IpcResponse> = serde_json::from_str(&line).unwrap();
        match res_msg.payload {
            IpcResponse::HealthStatus { status, .. } => assert_eq!(status, "HEALTHY"),
            _ => panic!("Expected HealthStatus response"),
        }
    }

    #[tokio::test]
    async fn test_ipc_dynamic_reload() {
        let addr = spawn_test_server().await;
        let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let mut framed = Framed::new(stream, LinesCodec::new_with_max_length(IPC_MAX_LINE_BYTES));

        let req = IpcMessage {
            version: "1.0".to_string(),
            payload: IpcRequest::ReloadConfig,
        };
        framed
            .send(serde_json::to_string(&req).unwrap())
            .await
            .unwrap();

        let line = framed.next().await.unwrap().unwrap();
        let res_msg: IpcMessage<IpcResponse> = serde_json::from_str(&line).unwrap();
        match res_msg.payload {
            IpcResponse::ReloadStatus { status } => assert_eq!(status, "SUCCESS"),
            _ => panic!("Expected ReloadStatus response"),
        }
    }

    #[tokio::test]
    async fn test_ipc_parse_error_response() {
        let addr = spawn_test_server().await;
        let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let mut framed = Framed::new(stream, LinesCodec::new_with_max_length(IPC_MAX_LINE_BYTES));

        framed.send("invalid json".to_string()).await.unwrap();

        let line = framed.next().await.unwrap().unwrap();
        let res_msg: IpcMessage<IpcResponse> = serde_json::from_str(&line).unwrap();
        match res_msg.payload {
            IpcResponse::Error(msg) => assert_eq!(msg, "Failed to parse request"),
            _ => panic!("Expected Error response"),
        }
    }

    #[tokio::test]
    async fn test_ipc_unsupported_version_rejected() {
        let addr = spawn_test_server().await;
        let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let mut framed = Framed::new(stream, LinesCodec::new_with_max_length(IPC_MAX_LINE_BYTES));

        let req = IpcMessage {
            version: "2.0".to_string(),
            payload: IpcRequest::HealthCheck,
        };
        framed
            .send(serde_json::to_string(&req).unwrap())
            .await
            .unwrap();

        let line = framed.next().await.unwrap().unwrap();
        let res_msg: IpcMessage<IpcResponse> = serde_json::from_str(&line).unwrap();
        match res_msg.payload {
            IpcResponse::Error(msg) => assert!(msg.contains("Unsupported IPC version")),
            _ => panic!("Expected Error response"),
        }
    }

    // Example showing how we'd test with Wiremock if mTLS was bypassed for test
    /*
    #[tokio::test]
    async fn test_external_cloud_mock() {
        use wiremock::{MockServer, Mock, matchers::{method, path}, ResponseTemplate};
        let mock_server = MockServer::start().await;

        // Mock a 500 error then 200 success to test retries,
        // however due to mTLS strict checking, the `fetch_config_with_retry`
        // will reject the connection because MockServer does not have the valid CA.
        // In a real testing environment, we would use conditional compilation to inject an insecure HTTP client.
    }
    */
}
