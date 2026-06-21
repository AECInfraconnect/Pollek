use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use dek_config::DekConfig;
use dek_policy_router::PolicyRouter;

#[derive(Debug, Clone, PartialEq)]
pub enum ReloadState {
    Idle,
    Warming,
    Preflighting,
    Active,
    Failed,
}

pub struct ReloadCoordinator {
    state: Arc<Mutex<ReloadState>>,
}

impl ReloadCoordinator {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ReloadState::Idle)),
        }
    }

    pub async fn process_staged_bundle(&self, config: &DekConfig, staged_path: &Path) -> Result<()> {
        let mut state = self.state.lock().await;
        if *state != ReloadState::Idle && *state != ReloadState::Active && *state != ReloadState::Failed {
            warn!("Reload already in progress: {:?}", *state);
            return Err(anyhow::anyhow!("Reload already in progress"));
        }

        *state = ReloadState::Warming;
        info!("ReloadCoordinator: Warming components from {:?}", staged_path);

        // Read payload
        let payload_str = std::fs::read_to_string(staged_path)?;
        let payload: serde_json::Value = serde_json::from_str(&payload_str)?;

        // Task 3.2: Warm Runtime
        let mut router = PolicyRouter::new();
        dek_router_builder::load_router_config(&mut router, &payload);

        // Load adapters
        if let Some(openfga_cfg) = &config.policy_config.as_ref().and_then(|c| c.openfga.as_ref()) {
            if let Ok(adapter) = dek_openfga::OpenFgaAdapter::new(&openfga_cfg.endpoint, &openfga_cfg.store_id, None) {
                router.register_evaluator("openfga", Box::new(adapter));
            }
        }
        if let Some(cedar_cfg) = &config.policy_config.as_ref().and_then(|c| c.cedar.as_ref()) {
            if let Ok(adapter) = dek_cedar::CedarAdapter::new(&cedar_cfg.policy_src) {
                router.register_evaluator("cedar", Box::new(adapter));
            }
        }

        // Task 3.3: Preflight Tests
        *state = ReloadState::Preflighting;
        if !config.preflight_tests.is_empty() {
            info!("ReloadCoordinator: Running {} preflight tests", config.preflight_tests.len());
            for test in &config.preflight_tests {
                let decision = router.authorize(test.input.clone()).await?;
                if decision.decision != test.expected_decision {
                    *state = ReloadState::Failed;
                    error!("Preflight test '{}' failed: expected {}, got {}", test.name, test.expected_decision, decision.decision);
                    return Err(anyhow::anyhow!("Preflight test failed"));
                }
            }
            info!("ReloadCoordinator: All preflight tests passed.");
        }

        // Task 3.4: Shadow and Canary Modes
        let active_path = dek_config::paths::get_active_bundle_path();
        let shadow_path = dek_config::paths::get_data_dir().join("shadow_bundle.json");
        let lkg_path = dek_config::paths::get_data_dir().join("active_bundle_lkg.json");

        match config.activation_mode {
            dek_config::ActivationMode::Full => {
                if active_path.exists() {
                    let _ = std::fs::copy(&active_path, &lkg_path);
                }
                std::fs::rename(staged_path, &active_path)?;
                info!("ReloadCoordinator: Activated bundle atomically to Full mode.");
            }
            dek_config::ActivationMode::ObserveOnly
            | dek_config::ActivationMode::Shadow
            | dek_config::ActivationMode::Canary => {
                std::fs::rename(staged_path, &shadow_path)?;
                info!("ReloadCoordinator: Activated bundle to Shadow/Canary mode at shadow_bundle.json");
            }
        }

        *state = ReloadState::Active;

        Ok(())
    }
}
