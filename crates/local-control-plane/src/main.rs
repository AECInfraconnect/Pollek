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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

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
        observability_store: store,
        signer,
        build_number: Arc::new(AtomicU64::new(1)),
        api_token,
        auth_disabled: cfg.auth_disabled,
        bundle_tx,
        pdp_credentials: Arc::new(PdpCredentialsStore::new(&cfg.data_dir)),
    };

    let static_dir = cfg.dashboard_dir.to_string_lossy().to_string();
    let app = app::create_app(state.clone(), &static_dir);

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
