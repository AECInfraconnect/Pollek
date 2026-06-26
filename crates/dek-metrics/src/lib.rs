// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::{Context, Result};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Notify, RwLock};
use tracing::{error, info, warn};

/// Install the global Prometheus recorder with default describes and an identifying service label.
pub fn install_recorder(service: &str) -> Result<PrometheusHandle> {
    let builder = PrometheusBuilder::new()
        .add_global_label("service", service)
        .add_global_label("system", "pollek-dek");

    let handle = builder
        .install_recorder()
        .context("failed to install Prometheus recorder")?;

    // Add descriptions for known metrics
    metrics::describe_counter!(
        "dek_proxy_requests_total",
        "Total number of proxy requests handled by MCP Proxy."
    );
    metrics::describe_histogram!(
        "dek_proxy_request_duration_seconds",
        "Duration of MCP Proxy requests."
    );
    metrics::describe_counter!(
        "dek_pdp_unavailable_total",
        "Total requests rejected due to PDP unavailability."
    );
    metrics::describe_counter!(
        "dek_pdp_error_total",
        "Total requests resulting in a PDP evaluation error."
    );
    metrics::describe_histogram!(
        "dek_policy_eval_latency_ms",
        "Latency of PDP policy evaluation in milliseconds."
    );
    metrics::describe_counter!("dek_svid_renew_total", "Total successful SVID renewals.");
    metrics::describe_counter!("dek_svid_renew_errors_total", "Total failed SVID renewals.");
    metrics::describe_gauge!(
        "dek_svid_expiry_seconds",
        "Remaining seconds until the current SVID expires."
    );
    metrics::describe_counter!(
        "dek_telemetry_spool_evicted_total",
        "Total telemetry events evicted due to full spool."
    );
    metrics::describe_gauge!(
        "dek_telemetry_spool_rows",
        "Current number of rows in the telemetry spool database."
    );

    Ok(handle)
}

/// Spawns a background task to periodically push metrics to a remote endpoint.
pub fn spawn_push(
    handle: PrometheusHandle,
    push_url: String,
    client: Arc<RwLock<Client>>,
    shutdown: Arc<Notify>,
    interval: Duration,
) {
    tokio::spawn(async move {
        info!("Started Prometheus metrics push loop to {}", push_url);
        let mut interval_timer = tokio::time::interval(interval);

        loop {
            tokio::select! {
                _ = shutdown.notified() => {
                    info!("Metrics push loop received shutdown signal. Exiting cleanly.");
                    break;
                }
                _ = interval_timer.tick() => {
                    let snapshot = handle.render();

                    let mut backoff = Duration::from_millis(500);
                    let mut success = false;

                    for attempt in 1..=3 {
                        let c = client.read().await.clone();
                        match c.post(&push_url).body(snapshot.clone()).send().await {
                            Ok(resp) => {
                                if resp.status().is_success() {
                                    success = true;
                                    break;
                                } else {
                                    warn!(
                                        attempt,
                                        status = %resp.status(),
                                        "Failed to push metrics."
                                    );
                                }
                            }
                            Err(e) => {
                                warn!(attempt, error = %e, "Failed to push metrics due to network error.");
                            }
                        }

                        if attempt < 3 {
                            tokio::time::sleep(backoff).await;
                            backoff *= 2;
                        }
                    }

                    if !success {
                        error!("Failed to push metrics after 3 attempts. Will retry next interval.");
                    }
                }
            }
        }
    });
}
