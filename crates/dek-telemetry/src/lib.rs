pub mod redactor;
pub mod spooler;

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
use tokio::sync::RwLock;
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
    client: Arc<RwLock<reqwest::Client>>,
    endpoint_url: String,
}

impl CloudTelemetrySink {
    pub fn new(endpoint_url: &str, mtls: &MtlsConfig, client_key_override: Option<&[u8]>, db_path: &str) -> Result<Arc<Self>> {
        let client = Arc::new(RwLock::new(mtls.build_client(client_key_override)?));
        let spooler = Arc::new(Spooler::new(db_path)?);
        let redactor = Arc::new(Redactor::new());

        let sink = Arc::new(Self {
            spooler: spooler.clone(),
            redactor,
            client: client.clone(),
            endpoint_url: endpoint_url.to_string(),
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

        Ok(sink)
    }

    fn init_otlp_metrics(endpoint_url: &str, _mtls: &MtlsConfig) -> Result<()> {
        let exporter = opentelemetry_otlp::new_exporter()
            .http()
            .with_endpoint(format!("{}/v1/metrics", endpoint_url))
            .build_metrics_exporter(Box::new(opentelemetry_sdk::metrics::reader::DefaultTemporalitySelector::new()))
            .map_err(|e| anyhow::anyhow!("Failed to build metrics exporter: {}", e))?;
            
        let reader = PeriodicReader::builder(exporter, opentelemetry_sdk::runtime::Tokio)
            .with_interval(Duration::from_secs(10))
            .build();
            
        let provider = SdkMeterProvider::builder()
            .with_reader(reader)
            .with_resource(Resource::new(vec![
                KeyValue::new("service.name", "pollen-dek")
            ]))
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

    pub fn emit_async(&self, mut event: Value, priority: Priority) {
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

            let mut to_delete = Vec::new();
            for (id, event) in batch {
                let bg_client = self.client.read().await.clone();
                let bg_url = self.endpoint_url.clone();
                
                let strategy = ExponentialBackoff::from_millis(500)
                    .factor(2)
                    .max_delay(Duration::from_secs(5))
                    .take(3);

                let res = Retry::spawn(strategy, || async {
                    let c = bg_client.clone();
                    match c.post(&bg_url).json(&event).send().await {
                        Ok(res) if res.status().is_success() => Ok(()),
                        Ok(res) => {
                            warn!("[Telemetry] Cloud rejected event. Status: {}", res.status());
                            Err(anyhow::anyhow!("Status {}", res.status()))
                        }
                        Err(e) => {
                            warn!("[Telemetry] Network error sending event: {}", e);
                            Err(e.into())
                        }
                    }
                })
                .await;

                if res.is_ok() {
                    to_delete.push(id);
                } else {
                    // Stop processing batch on network failure, wait for next loop
                    break;
                }
            }

            if !to_delete.is_empty() {
                if let Err(e) = self.spooler.delete_batch(&to_delete) {
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
