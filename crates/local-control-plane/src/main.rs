use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

use dek_control_plane_api::identity::ControlPlaneIdentity;

use local_control_plane::app;
use local_control_plane::auth;
use local_control_plane::config::LocalControlPlaneConfig;
use local_control_plane::pdp_credentials::PdpCredentialsStore;
use local_control_plane::signing::LocalSigner;
use local_control_plane::state::AppState;
use local_control_plane::store;

mod panic_guard;

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
    };

    // Spawn Anomaly Detector (P2)
    tokio::spawn(local_control_plane::anomaly_detector::start_anomaly_detector(state.clone()));

    let static_dir = cfg.dashboard_dir.to_string_lossy().to_string();
    let app = app::create_app(state.clone(), &static_dir, metrics_handle);

    store::seed_pdp_defaults(&state.pdp_store).await?;

    // 3.2 Observe -> Suggest -> Enforce Loop
    let _ = local_control_plane::governance::start_governance_loop(state.clone()).await;

    // Phase 5: Pollen Cloud Registry Sync Loop
    let _ = local_control_plane::cloud_sync::start_cloud_registry_sync_loop(state.clone()).await;

    let listener = TcpListener::bind(&cfg.bind_addr).await?;
    info!("Local Control Plane listening on http://{}", cfg.bind_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
