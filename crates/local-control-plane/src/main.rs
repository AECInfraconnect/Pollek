use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

use anyhow::Context;
use dek_control_plane_api::identity::ControlPlaneIdentity;
use dek_secure_spool::key_manager::OsKeyStore;
use sha2::Digest;

use local_control_plane::app;
use local_control_plane::auth;
use local_control_plane::config::LocalControlPlaneConfig;
use local_control_plane::pdp_credentials::PdpCredentialsStore;
use local_control_plane::signing::LocalSigner;
use local_control_plane::state::AppState;
use local_control_plane::store;

mod panic_guard;

fn local_device_id() -> String {
    if let Ok(id) = std::env::var("POLLEK_DEVICE_ID") {
        let trimmed = id.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "local-device".to_string());
    let mut hasher = sha2::Sha256::new();
    hasher.update(host.as_bytes());
    let digest = hasher.finalize();
    format!("dev_{}", hex::encode(&digest[..8]))
}

fn load_sqlite_spool_key(data_dir: &Path) -> anyhow::Result<[u8; 32]> {
    std::fs::create_dir_all(data_dir).with_context(|| {
        format!(
            "failed to create local control-plane data dir {}",
            data_dir.display()
        )
    })?;
    let key_path = data_dir.join("telemetry-spool-master.key");
    let store = dek_secure_spool::os::DefaultOsKeyStore::new(key_path);
    store
        .load_or_create_master_key()
        .map_err(|err| anyhow::anyhow!("failed to load local telemetry spool key: {err}"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    panic_guard::install_panic_hook();
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .map_err(|_| ())
            .ok();
    }
    tracing_subscriber::fmt::init();

    #[allow(clippy::expect_used)]
    let metrics_handle = dek_metrics::install_recorder("local-control-plane")
        .expect("Failed to install Prometheus recorder"); //

    let cfg = LocalControlPlaneConfig::from_env()?;
    let device_id = local_device_id();
    let spool_key = load_sqlite_spool_key(&cfg.data_dir)?;

    let store = Arc::new(store::SqliteStore::new(&cfg.db_url).await?);
    let signer = Arc::new(LocalSigner::load_or_create(&cfg.data_dir)?);

    info!(
        "local control-plane signing key: {} (pub {})",
        signer.key_id,
        signer.public_key_b64()
    );

    if cfg.auth_disabled {
        tracing::warn!("DEK_LCP_AUTH_DISABLE=1: Authentication is disabled!");
    }

    let api_token = auth::load_or_create_token(&cfg.data_dir)?;
    let (bundle_tx, _) = tokio::sync::broadcast::channel(100);

    let state = AppState {
        identity: ControlPlaneIdentity::local_default(),
        registry_store: store.clone(),
        policy_store: store.clone(),
        telemetry_store: store.clone(),
        pdp_store: store.clone(),
        observability_store: store.clone(),
        deployment_store: store,
        signer,
        build_number: Arc::new(AtomicU64::new(1)),
        api_token,
        auth_disabled: cfg.auth_disabled,
        bundle_tx,
        pdp_credentials: Arc::new(PdpCredentialsStore::new(&cfg.data_dir)),
        def_store: Arc::new(dek_fingerprint_defs::loader::DefinitionStore::load(
            cfg.data_dir.join("defs/active.json"),
            None,
        )),
        latest_snapshot: Arc::new(tokio::sync::RwLock::new(None)),
        secure_spool: Arc::new(dek_secure_spool::sqlite_spool::SqliteSpool::new(
            &cfg.data_dir.join("telemetry_spool.db"),
            &spool_key,
            dek_secure_spool::sqlite_spool::DEFAULT_MAX_ROWS,
        )?),
        telemetry_tx: tokio::sync::broadcast::channel(100).0,
    };

    // Spawn TelemetrySink Background Loop
    let (telemetry_mpsc_tx, mut telemetry_mpsc_rx) =
        tokio::sync::mpsc::channel::<pollek_contract::PollekTelemetryEnvelopeV1>(1000);

    let spool = state.secure_spool.clone();
    let sse_tx = state.telemetry_tx.clone();
    let telemetry_store = state.telemetry_store.clone();
    tokio::spawn(async move {
        while let Some(env) = telemetry_mpsc_rx.recv().await {
            if let Ok(bytes) = serde_json::to_vec(&env) {
                let priority = dek_secure_spool::sqlite_spool::Priority::Normal;
                if let Err(e) = spool.push(priority, &bytes) {
                    tracing::error!("Failed to spool telemetry: {}", e);
                }
            }
            if let Ok(value) = serde_json::to_value(&env) {
                if let Err(e) = telemetry_store
                    .put_telemetry(&env.tenant_id, &env.event_type, &env.event_id, &value)
                    .await
                {
                    tracing::error!("Failed to persist telemetry event: {}", e);
                }
            }
            // Broadcast to SSE clients
            let _ = sse_tx.send(env);
        }
    });

    let telemetry_sink = dek_enforcement_api::control_method::TelemetrySink {
        tx: telemetry_mpsc_tx,
        ctx: Arc::new(dek_enforcement_api::control_method::EmitCtx {
            tenant_id: "local".to_string(),
            device_id: device_id.clone(),
        }),
    };

    if std::env::var("POLLEK_ENABLE_SIMULATED_EGRESS")
        .ok()
        .as_deref()
        == Some("1")
    {
        let sim_sink = telemetry_sink.clone();
        tokio::spawn(async move {
            use dek_enforcement_api::egress_observer::EgressEventSource;
            let source = local_control_plane::egress_simulator::SimulatorEgressSource {
                deterministic: false,
            };
            if let Err(e) = source.start_observing(sim_sink).await {
                tracing::error!("SimulatorEgressSource error: {}", e);
            }
        });
    } else {
        tracing::info!(
            "Egress simulator disabled. Set POLLEK_ENABLE_SIMULATED_EGRESS=1 for demo fixtures."
        );
    }

    // Spawn Anomaly Detector (P2)
    tokio::spawn(local_control_plane::anomaly_detector::start_anomaly_detector(state.clone()));

    let static_dir = cfg.dashboard_dir.to_string_lossy().to_string();
    let app = app::create_app(state.clone(), &static_dir, metrics_handle);

    store::seed_pdp_defaults(&state.pdp_store).await?;

    // 3.2 Observe -> Suggest -> Enforce Loop
    let _ = local_control_plane::governance::start_governance_loop(state.clone()).await;

    // Phase 5: Pollek Cloud Registry Sync Loop
    let _ = local_control_plane::cloud_sync::start_cloud_registry_sync_loop(state.clone()).await;

    let listener = TcpListener::bind(&cfg.bind_addr).await?;
    info!("Local Control Plane listening on http://{}", cfg.bind_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
