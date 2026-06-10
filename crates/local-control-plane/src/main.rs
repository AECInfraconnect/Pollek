use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

use dek_control_plane_api::identity::ControlPlaneIdentity;

mod app;
mod auth;
mod bundle;
mod compiler;
mod config;
mod error;
mod policy;
mod push;
mod registry;
mod signing;
mod state;
mod store;
mod telemetry;

use config::LocalControlPlaneConfig;
use signing::LocalSigner;
use state::AppState;

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
        telemetry_store: store,
        signer,
        build_number: Arc::new(AtomicU64::new(1)),
        api_token,
        auth_disabled: cfg.auth_disabled,
        bundle_tx,
    };

    let static_dir = cfg.dashboard_dir.to_string_lossy().to_string();
    let app = app::create_app(state, &static_dir);

    let listener = TcpListener::bind(&cfg.bind_addr).await?;
    info!("Local Control Plane listening on http://{}", cfg.bind_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
