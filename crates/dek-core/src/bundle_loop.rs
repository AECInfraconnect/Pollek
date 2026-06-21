//! bundle_loop.rs — periodic unified bundle-sync pipeline + binary auto-update.
//!
//! Lifted verbatim from `main.rs::spawn_bundle_sync_task`, made `pub`.
//! Each tick runs `BundleSyncAgent::run_pipeline()` (fetch -> verify ed25519 ->
//! merge -> stage active_bundle), and if the returned config carries an
//! `update_config` with a new version, triggers the health-gated A/B updater.

use dek_bundle_sync::BundleSyncAgent;
use metrics::counter;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout, Duration};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn, Instrument};

pub fn spawn_bundle_sync_task(
    cancel_token: CancellationToken,
    sync_agent: Arc<BundleSyncAgent>,
    bundle_sync_interval: u64,
    metrics_client: Arc<RwLock<reqwest::Client>>,
    pinned_key: String,
    reload_coordinator: Arc<crate::reload_coordinator::ReloadCoordinator>,
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
                            Ok(Ok((new_config, staged_path))) => {
                                counter!("dek_core_bundle_sync_success_total").increment(1);
                                if let Err(e) = reload_coordinator.process_staged_bundle(&new_config, &staged_path).await {
                                    error!("Bundle Activation Failed: {}", e);
                                    counter!("dek_core_bundle_activation_errors_total").increment(1);
                                } else {
                                    // Enforce Enterprise Profiles
                                    use dek_config::{EnterpriseProfile, ActivationMode};
                                    if (new_config.enterprise_profile == EnterpriseProfile::Enterprise || new_config.enterprise_profile == EnterpriseProfile::Regulated)
                                        && new_config.activation_mode != ActivationMode::Full {
                                            warn!("Enterprise Profile enforces 'Full' activation mode. Overriding '{}'", format!("{:?}", new_config.activation_mode));
                                        }
                                }

                                if let Some(update) = new_config.update_config {
                                    if update.version != current_version {
                                        info!("New binary update found: version {}", update.version);
                                        let client = metrics_client.read().await.clone();
                                        match crate::updater::run_update(
                                            &client,
                                            &update.download_url,
                                            &update.signature_b64,
                                            &pinned_key,
                                        ).await {
                                            Ok(_) => {
                                                info!("Update staged successfully. Version updated to {}", update.version);
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
