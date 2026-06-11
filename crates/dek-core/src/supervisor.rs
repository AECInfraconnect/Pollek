#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! supervisor.rs (v2) — aligned to the REAL dek-core structure.
//!
//! Supersedes the earlier generic draft: uses `CancellationToken` (not Notify),
//! the real deps (`BundleSyncAgent`, `CloudTelemetrySink`, metrics client), and
//! calls the extracted `ipc_server` / `bundle_loop` modules. Formalizes today's
//! `core_main()` into an owned unit and adds the health-gated probation step
//! after services are up.
//!
//! main.rs modules:
//!   mod supervisor; mod ipc_server; mod bundle_loop; mod ipc_client;
//!   mod probation; mod ebpf; mod keystore_migration; mod updater;
//!   mod service_integration;
//!
//!   fn main() -> anyhow::Result<()> {
//!       service_integration::run_as_service_if_needed(run())
//!   }
//!   async fn run() -> anyhow::Result<()> {
//!       supervisor::Supervisor::bootstrap().await?.run().await
//!   }

use anyhow::{Context, Result};
use dek_bundle_sync::BundleSyncAgent;
use dek_config::BootstrapConfig;
use dek_telemetry::CloudTelemetrySink;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

fn env_var(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

pub struct Supervisor {
    cloud_url: String,
    ipc_addr: String,
    bundle_interval: u64,
    bootstrap: BootstrapConfig,
    pinned_key: String,
    client_key_override: Option<Vec<u8>>,
    bundle_agent: Arc<BundleSyncAgent>,
    telemetry_sink: Arc<CloudTelemetrySink>,
    metrics_client: Arc<RwLock<reqwest::Client>>,
    cancel: CancellationToken,
    start_time: Instant,
    pending_update: Option<crate::probation::ProbationMarker>,
    _ebpf: Option<dek_ebpfd::EbpfHandle>,
}

impl Supervisor {
    /// Ordered, one-time startup. Mirrors core_main() up to task spawning.
    pub async fn bootstrap() -> Result<Self> {
        #[allow(clippy::print_stderr)]
        {
            dek_config::logging::init_logging("dek-core")
                .unwrap_or_else(|e| eprintln!("Failed to initialize logging: {e}"));
        }
        info!("Starting Pollen DEK Core Supervisor...");

        let config_dir = dek_config::paths::get_config_dir();
        let pending_update = crate::probation::detect(&config_dir);

        let bootstrap_path = env_var(
            "DEK_BOOTSTRAP_PATH",
            &dek_config::paths::get_bootstrap_path().to_string_lossy(),
        );
        let bootstrap =
            BootstrapConfig::load_or_default(&bootstrap_path).context("load bootstrap")?;

        tracing::info!(
            "DEBUG BOOTSTRAP: Loaded from {}, key is: {}",
            bootstrap_path,
            bootstrap.pinned_bundle_public_key
        );

        let cloud_url = if !bootstrap.cloud_url.is_empty() {
            bootstrap.cloud_url.clone()
        } else {
            env_var("POLLEN_CLOUD_URL", "https://127.0.0.1:43891")
        };

        if !cloud_url.starts_with("https://")
            && !cloud_url.contains("127.0.0.1")
            && !cloud_url.contains("localhost")
        {
            error!("Fatal: POLLEN_CLOUD_URL must be https:// (downgrade protection).");
            std::process::exit(1);
        }
        let ipc_addr = env_var("DEK_IPC_ADDR", "127.0.0.1:43889");
        let bundle_interval = env_var("DEK_BUNDLE_SYNC_INTERVAL", "10")
            .parse()
            .unwrap_or(10);

        // Keystore migration (fail-open to file). Pull overrides if it succeeded.
        let mut client_key_override: Option<Vec<u8>> = None;
        let mut pinned_key_override: Option<String> = None;
        if cloud_url.starts_with("https://")
            && crate::keystore_migration::run_migration(&bootstrap, &cloud_url).await
        {
            let ks = dek_keystore::get_keystore();
            if let Ok(k) = ks.load_key("mtls_client_key") {
                client_key_override = Some(k);
            }
            if let Ok(pk) = ks.load_key("pinned_bundle_public_key") {
                if let Ok(s) = String::from_utf8(pk) {
                    pinned_key_override = Some(s);
                }
            }
        }
        let pinned_key = pinned_key_override
            .clone()
            .unwrap_or_else(|| bootstrap.pinned_bundle_public_key.clone());
        tracing::info!(
            "DEBUG BOOTSTRAP OVERRIDE: override={:?} -> final_pinned_key={}",
            pinned_key_override,
            pinned_key
        );

        let tenant_id = bootstrap.tenant_id.as_deref().unwrap_or("unknown_tenant");
        let bundle_agent = Arc::new(BundleSyncAgent::new(
            &cloud_url,
            tenant_id,
            &bootstrap.device_id,
            &bootstrap.mtls,
            &pinned_key,
            client_key_override.as_deref(),
            bootstrap.local_api_token.clone(),
        )?);
        let data_dir = dek_config::paths::get_data_dir();
        let telemetry_sink = CloudTelemetrySink::new(
            cloud_url.trim_end_matches('/'),
            &bootstrap.mtls,
            client_key_override.as_deref(),
            &data_dir.join("telemetry.db").to_string_lossy(),
            bootstrap.local_api_token.clone(),
        )?;
        let metrics_client = Arc::new(RwLock::new(
            bootstrap
                .mtls
                .build_client(client_key_override.as_deref())
                .context("build metrics mTLS client")?,
        ));

        let (dns_tx, mut dns_rx) = tokio::sync::mpsc::channel::<dek_ebpfd::DnsObservation>(1024);
        let sink = telemetry_sink.clone();
        tokio::spawn(async move {
            while let Some(obs) = dns_rx.recv().await {
                sink.emit_async(
                    serde_json::json!({
                        "event_type": "pollen.dek.dns_observe",
                        "cgroup_id": obs.cgroup_id,
                        "qname": obs.qname,
                        "answers": obs.answers,
                        "is_response": obs.is_response,
                    }),
                    dek_telemetry::Priority::Low,
                );
            }
        });

        let _ebpf = crate::ebpf::load_and_attach(Some(dns_tx)).await;

        Ok(Self {
            cloud_url,
            ipc_addr,
            bundle_interval,
            bootstrap,
            pinned_key,
            client_key_override,
            bundle_agent,
            telemetry_sink,
            metrics_client,
            cancel: CancellationToken::new(),
            start_time: Instant::now(),
            pending_update,
            _ebpf,
        })
    }

    pub async fn run(mut self) -> Result<()> {
        let reload_coordinator = Arc::new(crate::reload_coordinator::ReloadCoordinator::new());

        let renew_cfg = crate::svid_renewal::RenewalConfig {
            renew_url: format!(
                "{}/v1/tenants/{}/devices/{}/spire/svid/renew",
                self.cloud_url.trim_end_matches('/'),
                self.bootstrap
                    .tenant_id
                    .as_deref()
                    .unwrap_or("unknown_tenant"),
                self.bootstrap.device_id
            ),
            device_id: self.bootstrap.device_id.clone(),
            mtls: self.bootstrap.mtls.clone(),
        };

        // 1) IPC first so probation/dekctl can probe immediately.
        let ipc_handle: JoinHandle<()> = crate::ipc_server::spawn_ipc_server_task(
            self.cancel.clone(),
            self.ipc_addr.clone(),
            self.telemetry_sink.clone(),
            self.bundle_agent.clone(),
            self.metrics_client.clone(),
            self.start_time,
            reload_coordinator.clone(),
            renew_cfg.clone(),
        )
        .await?;

        crate::service_integration::notify_ready();

        // 2) Bundle sync + auto-update loop.

        use dek_policy_syncer::{FreshnessConfig, PolicySyncer};

        let max_stale = std::env::var("DEK_MAX_STALE_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(86400);

        let fresh_cfg = FreshnessConfig {
            max_bundle_age_secs: max_stale, // Configurable for tests
            grace_secs: 600,
        };
        let tenant_id = self
            .bootstrap
            .tenant_id
            .clone()
            .unwrap_or_else(|| "unknown_tenant".to_string());

        let (health_tx, health_rx) =
            tokio::sync::watch::channel(crate::svid_renewal_failclosed::IdentityHealth::Healthy);

        let syncer = PolicySyncer::new(
            self.bundle_agent.clone(),
            Some(self.telemetry_sink.clone()),
            fresh_cfg,
            self.bootstrap.device_id.clone(),
            tenant_id.clone(),
            self.cloud_url.clone(),
            self.pinned_key.clone(),
            self.bootstrap.local_api_token.clone(),
        );

        let snapshot_ref = reload_coordinator.activation.snapshot.clone();
        let enforcement_ref = syncer.enforcement();
        let telemetry_for_api = Some(self.telemetry_sink.clone());
        tokio::spawn(async move {
            if let Err(e) = crate::api::start_sidecar_api(
                snapshot_ref,
                enforcement_ref,
                telemetry_for_api,
                health_rx,
                std::env::var("DEK_API_PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(43890),
            )
            .await
            {
                error!("Sidecar API failed: {}", e);
            }
        });

        let (sync_tx, sync_rx) = tokio::sync::mpsc::channel::<dek_policy_syncer::SyncOutcome>(100);
        let bundle_handle = syncer.clone().spawn(
            std::time::Duration::from_secs(self.bundle_interval),
            self.cancel.clone(),
            Some(sync_tx),
        );

        // Network egress guardrail plane (Phase A) — trait-based, all 3 OS, fail-closed.
        let _net_handle = crate::network_loop::spawn(
            sync_rx,
            tenant_id.clone(),
            self.bootstrap.device_id.clone(),
            self.cancel.clone(),
            reload_coordinator.clone(),
        );

        // 3) Probation finalize (only if an update is on trial). After services up.
        if let Some(marker) = self.pending_update.take() {
            let cloud = self.cloud_url.clone();
            let bootstrap = self.bootstrap.clone();
            let config_dir = dek_config::paths::get_config_dir();
            let bundle_path = dek_config::paths::get_active_bundle_path();
            let ipc_addr = self.ipc_addr.clone();
            tokio::spawn(async move {
                let probe = move || {
                    let addr = ipc_addr.clone();
                    async move { crate::ipc_client::health_ok(&addr).await }
                };
                crate::probation::finalize(
                    config_dir,
                    cloud,
                    bootstrap,
                    bundle_path,
                    crate::probation::ProbationSettings::default(),
                    marker,
                    probe,
                )
                .await; // abort path: self_replace + exit(1) inside finalize
            });
        }

        // (Telemetry/metrics-push tasks: spawn here as today, using
        //  self.telemetry_sink / self.metrics_client / self.client_key_override.)
        let _ = &self.client_key_override;

        // Trust Bundle Poller + Hot Rebuild
        let (jwks_tx, _jwks_rx) = tokio::sync::watch::channel(Vec::<serde_json::Value>::new());
        let (roots_changed_tx, mut roots_changed_rx) = tokio::sync::watch::channel(0u64);

        if self.cloud_url.starts_with("https://") {
            if let Ok(tb_client) = self.bootstrap.mtls.build_client(None) {
                let _poller = dek_spire_node::spawn_trust_bundle_poller(
                    tb_client,
                    self.cloud_url.clone(),
                    self.bootstrap.mtls.root_ca_path.clone(),
                    jwks_tx,
                    roots_changed_tx,
                    self.cancel.clone(),
                );
            }
        }

        let mtls_clone = self.bootstrap.mtls.clone();
        let metrics_client_clone = self.metrics_client.clone();
        let override_key = self.client_key_override.clone();
        tokio::spawn(async move {
            while roots_changed_rx.changed().await.is_ok() {
                let seq = *roots_changed_rx.borrow();
                tracing::info!("trust root changed (seq={seq}); rebuilding mTLS clients");
                if let Ok(new_client) = mtls_clone.build_client(override_key.as_deref()) {
                    *metrics_client_clone.write().await = new_client;
                }
            }
        });

        // Spawn SVID Renewal Task (Only in Cloud Mode)
        let is_local_mode =
            self.cloud_url.contains("127.0.0.1") || self.cloud_url.contains("localhost");
        let renew_handle = if !is_local_mode {
            crate::svid_renewal::spawn_svid_renewal_task(
                self.cancel.clone(),
                renew_cfg,
                self.telemetry_sink.clone(),
                self.bundle_agent.clone(),
                self.metrics_client.clone(),
                health_tx,
            )
        } else {
            tokio::spawn(async {})
        };

        // 4) Wait for shutdown signal -> cancel -> bounded drain.
        Self::wait_for_signal().await;
        info!("Shutdown signal received; cancelling tasks...");
        self.cancel.cancel();

        let drain = async {
            let _ = ipc_handle.await;
            let _ = bundle_handle;
            let _ = renew_handle.await;
            let _ = _net_handle.await;
        };
        if tokio::time::timeout(Duration::from_secs(15), drain)
            .await
            .is_err()
        {
            error!("Graceful shutdown timed out.");
        }
        info!("dek-core stopped cleanly.");
        Ok(())
    }

    #[cfg(unix)]
    async fn wait_for_signal() {
        use tokio::signal::unix::{signal, SignalKind};
        #[allow(clippy::expect_used)] let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
        #[allow(clippy::expect_used)] let mut int = signal(SignalKind::interrupt()).expect("SIGINT handler");
        tokio::select! { _ = term.recv() => {}, _ = int.recv() => {} }
    }

    #[cfg(not(unix))]
    async fn wait_for_signal() {
        let _ = tokio::signal::ctrl_c().await;
    }
}
