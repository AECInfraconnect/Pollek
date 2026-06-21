use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use dek_control_plane_api::identity::ControlPlaneIdentity;
use tokio::sync::broadcast;

use crate::signing::LocalSigner;
use crate::store;

#[derive(Clone)]
pub struct AppState {
    pub identity: ControlPlaneIdentity,
    pub registry_store: Arc<dyn store::RegistryStore>,
    pub policy_store: Arc<dyn store::PolicyStore>,
    pub telemetry_store: Arc<dyn store::TelemetryStore>,
    pub pdp_store: Arc<dyn store::PdpStore>,
    pub observability_store: Arc<dyn store::ObservabilityStore>,
    pub signer: Arc<LocalSigner>,
    pub build_number: Arc<AtomicU64>,
    pub api_token: String,
    pub auth_disabled: bool,
    pub bundle_tx: broadcast::Sender<String>,
    pub pdp_credentials: Arc<crate::pdp_credentials::PdpCredentialsStore>,
}
