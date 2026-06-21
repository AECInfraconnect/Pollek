// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

pub mod redactor;
pub mod routing;
pub mod spooler;
pub mod fallback_spool;

use anyhow::Result;
use dek_config::MtlsConfig;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::Resource;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::Retry;
use tracing::{error, info, warn};

use crate::redactor::Redactor;
pub use crate::spooler::{Priority, Spooler};

#[derive(Debug, Serialize, Deserialize)]
pub struct TelemetryEnvelope {
    pub ts: String,
    pub tenant_id: Option<String>,
    pub device_id: String,
    pub spiffe_id: String,
    pub dek_version: String,
    pub os: String,
    pub event_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub egress: Option<EgressLog>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpDecisionLog>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EgressLog {
    pub dest_ip: String,
    pub dest_port: u16,
    pub fqdn: Option<String>,
    pub cgroup_id: u64,
    pub pid: u32,
    pub verdict: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpDecisionLog {
    pub principal: String,
    pub tool: String,
    pub method: String,
    pub engine: String,
    pub verdict: String,
    pub reason: String,
    pub request_id: String,
}

pub struct CloudTelemetrySink {
    spooler: Arc<Spooler>,
    redactor: Arc<Redactor>,
    client: Arc<tokio::sync::RwLock<reqwest::Client>>,
    endpoint_url: String,
    enterprise_profile: Arc<std::sync::RwLock<dek_config::EnterpriseProfile>>,
    api_token: Option<String>,
    fallback: Arc<fallback_spool::SecureFallback>,
}

impl CloudTelemetrySink {
    pub fn new(
        endpoint_url: &str,
        mtls: &MtlsConfig,
        client_key_override: Option<&[u8]>,
        db_path: &str,
        api_token: Option<String>,
        tenant_id: String,
        device_id: String,
    ) -> Result<Arc<Self>> {
        let client = Arc::new(tokio::sync::RwLock::new(
            mtls.build_client(client_key_override)?,
        ));
        let spooler = Arc::new(Spooler::new(db_path)?);
        let redactor = Arc::new(Redactor::new());

        let sink = Arc::new(Self {
            spooler: spooler.clone(),
            redactor,
            client: client.clone(),
            endpoint_url: endpoint_url.to_string(),
            enterprise_profile: Arc::new(std::sync::RwLock::new(
                dek_config::EnterpriseProfile::default(),
            )),
            api_token,
            fallback: Arc::new(fallback_spool::SecureFallback::new(tenant_id, device_id)?),
        });

        // Initialize OTLP Metrics Provider
        Self::init_otlp_metrics(endpoint_url, mtls)?;

        let bg_sink = sink.clone();
        tokio::spawn(async move {
            bg_sink.start_flusher().await;
        });

        // Start Heartbeat
        let hb_sink = sink.clone();
        tokio::spawn(async move {
            hb_sink.start_heartbeat().await;
        });

        // Start fallback replay
        let fb_sink = sink.clone();
        let fallback_url = endpoint_url.to_string();
        tokio::spawn(async move {
            fb_sink.fallback.start_replay(
                fallback_url,
                fb_sink.client.clone(),
                fb_sink.api_token.clone()
            ).await;
        });

        Ok(sink)
    }

    fn init_otlp_metrics(endpoint_url: &str, _mtls: &MtlsConfig) -> Result<()> {
        let exporter = opentelemetry_otlp::new_exporter()
            .http()
            .with_endpoint(format!("{}/v1/metrics", endpoint_url))
            .build_metrics_exporter(Box::new(
                opentelemetry_sdk::metrics::reader::DefaultTemporalitySelector::new(),
            ))
            .map_err(|e| anyhow::anyhow!("Failed to build metrics exporter: {}", e))?;

        let reader = PeriodicReader::builder(exporter, opentelemetry_sdk::runtime::Tokio)
            .with_interval(Duration::from_secs(10))
            .build();

        let provider = SdkMeterProvider::builder()
            .with_reader(reader)
            .with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                "pollen-dek",
            )]))
            .build();

        global::set_meter_provider(provider);
        info!("[Telemetry] OTLP Metrics provider initialized");
        Ok(())
    }

    pub async fn update_mtls(&self, mtls: &MtlsConfig) -> Result<()> {
        let new_client = mtls.build_client(None)?;
        let mut client_lock = self.client.write().await;
        *client_lock = new_client;

        // (In a real system, we might also need to update the OTLP exporter's client)
        info!("[Telemetry] Successfully updated internal HTTP client with new mTLS configuration");
        Ok(())
    }

    pub fn set_enterprise_profile(&self, profile: dek_config::EnterpriseProfile) {
        if let Ok(mut lock) = self.enterprise_profile.write() {
            *lock = profile;
        }
    }

    pub fn emit_async(&self, mut event: Value, priority: Priority) {
        // Enforce Enterprise Regulated Profile: Strip any raw payload capture
        let profile = self
            .enterprise_profile
            .read()
            .map(|p| p.clone())
            .unwrap_or_default();
        if profile == dek_config::EnterpriseProfile::Regulated {
            if let Some(obj) = event.as_object_mut() {
                obj.remove("raw_payload");
                obj.remove("http_body");
            }
        }

        // Redact PII / Secrets before queueing
        self.redactor.redact_value(&mut event);

        // Queue to SQLite spooler
        if let Err(e) = self.spooler.push(priority, &event) {
            error!("[Telemetry] Failed to spool event: {}", e);
        }
    }

    async fn start_flusher(&self) {
        info!("[Telemetry] Flusher loop started");
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;

            let batch = match self.spooler.pop_batch(50) {
                Ok(b) => b,
                Err(e) => {
                    error!("[Telemetry] Spool read error: {}", e);
                    continue;
                }
            };

            if batch.is_empty() {
                continue;
            }

            let grouped = routing::group_by_endpoint(batch);
            let base = self
                .endpoint_url
                .trim_end_matches("/v1/telemetry/events")
                .trim_end_matches('/')
                .to_string();

            let mut all_ok_ids: Vec<i64> = Vec::new();

            for (suffix, (ids, events)) in grouped {
                let url = format!("{}{}", base, suffix);
                let payload = serde_json::json!({ "events": events });

                let bg_client = self.client.read().await.clone();
                let strategy = ExponentialBackoff::from_millis(500)
                    .factor(2)
                    .max_delay(Duration::from_secs(5))
                    .take(3);

                let sink_token = self.api_token.clone();

                let res = Retry::start(strategy, || async {
                    let c = bg_client.clone();
                    let mut req = c.post(&url).json(&payload);
                    if let Some(t) = &sink_token {
                        req = req.header("Authorization", format!("Bearer {}", t));
                    }
                    match req.send().await {
                        Ok(res) if res.status().is_success() => Ok(()),
                        Ok(res) if res.status().is_client_error() => {
                            warn!(
                                "[Telemetry] Cloud rejected batch to {} (4xx). Status: {}",
                                suffix,
                                res.status()
                            );
                            Err(anyhow::anyhow!("Non-retryable {}", res.status()))
                        }
                        Ok(res) => {
                            warn!(
                                "[Telemetry] Cloud rejected batch to {} (5xx). Status: {}",
                                suffix,
                                res.status()
                            );
                            Err(anyhow::anyhow!("Retryable {}", res.status()))
                        }
                        Err(e) => {
                            warn!(
                                "[Telemetry] Network error sending batch to {}: {}",
                                suffix, e
                            );
                            Err(e.into())
                        }
                    }
                })
                .await;

                match res {
                    Ok(()) => all_ok_ids.extend(ids),
                    Err(e) => {
                        if e.to_string().contains("Non-retryable") {
                            warn!(
                                "[Telemetry] Cloud rejected batch (4xx). Spooling to secure fallback for endpoint {}",
                                suffix
                            );
                            let events_to_fallback: Vec<serde_json::Value> = events.clone();
                            if let Err(fe) = self.fallback.append_batch(events_to_fallback) {
                                error!("[Telemetry] Failed to append to secure fallback: {}", fe);
                            }
                            all_ok_ids.extend(ids);
                        } else {
                            warn!(
                                "[Telemetry] endpoint {} failed: {}; will retry next flush",
                                suffix, e
                            );
                        }
                    }
                }
            }

            if !all_ok_ids.is_empty() {
                if let Err(e) = self.spooler.delete_batch(&all_ok_ids) {
                    error!("[Telemetry] Failed to delete flushed events: {}", e);
                }
            }
        }
    }

    async fn start_heartbeat(&self) {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;

            let q_depth = self.spooler.len().unwrap_or(0);
            let heartbeat = serde_json::json!({
                "schema_version": "1.0",
                "event_type": "heartbeat",
                "device_id": "dek-device-01",
                "status": if q_depth > 1000 { "degraded" } else { "healthy" },
                "telemetry_queue_depth": q_depth,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });

            self.emit_async(heartbeat, Priority::Normal);
        }
    }
}
