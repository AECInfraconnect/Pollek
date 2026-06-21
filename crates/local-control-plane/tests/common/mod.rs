#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::duplicated_attributes
)]
use dek_control_plane_api::identity::ControlPlaneIdentity;
use local_control_plane::{app, auth, signing::LocalSigner, state::AppState, store};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::net::TcpListener;

pub struct LocalControlPlaneHarness {
    pub base_url: String,
    #[allow(dead_code)]
    pub api_token: String,
    // Keep tempdir alive
    _tempdir: tempfile::TempDir,
}

impl LocalControlPlaneHarness {
    pub async fn start() -> Self {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("test.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());

        let store = Arc::new(store::SqliteStore::new(&db_url).await.unwrap());
        let signer = Arc::new(LocalSigner::load_or_create(tempdir.path()).unwrap());
        let api_token = auth::load_or_create_token(tempdir.path()).unwrap();
        let (bundle_tx, _) = tokio::sync::broadcast::channel(10);

        let state = AppState {
            identity: ControlPlaneIdentity::local_default(),
            registry_store: store.clone(),
            policy_store: store.clone(),
            pdp_store: store.clone(),
            telemetry_store: store.clone(),
            observability_store: store,
            signer,
            build_number: Arc::new(AtomicU64::new(1)),
            api_token: api_token.clone(),
            auth_disabled: true, // For tests, or test token
            bundle_tx,
            pdp_credentials: Arc::new(
                local_control_plane::pdp_credentials::PdpCredentialsStore::new(tempdir.path()),
            ),
        };

        let app = app::create_app(state, "dummy_static_dir");

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let base_url = format!("http://127.0.0.1:{}", port);

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self {
            base_url,
            api_token,
            _tempdir: tempdir,
        }
    }
}
