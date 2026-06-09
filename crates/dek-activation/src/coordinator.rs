use crate::snapshot::RuntimeSnapshot;
use crate::{ActivationDecision, ActivationError, ActivationReceipt, ActivationRequest};
use arc_swap::ArcSwap;
use dek_config::DekConfig;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationState {
    Idle,
    Warming,
    Preflighting,
    Activating,
    Failed,
}

pub struct ActivationCoordinator {
    state: Arc<Mutex<ActivationState>>,
    pub snapshot: Arc<ArcSwap<RuntimeSnapshot>>,
    generation: std::sync::atomic::AtomicU64,
}

impl ActivationCoordinator {
    pub fn new(initial_snapshot: RuntimeSnapshot) -> Self {
        Self {
            state: Arc::new(Mutex::new(ActivationState::Idle)),
            snapshot: Arc::new(ArcSwap::from_pointee(initial_snapshot)),
            generation: std::sync::atomic::AtomicU64::new(1),
        }
    }

    pub async fn process_activation(
        &self,
        req: ActivationRequest,
        config: &DekConfig,
    ) -> Result<ActivationDecision, anyhow::Error> {
        let mut state = self.state.lock().await;
        if *state != ActivationState::Idle && *state != ActivationState::Failed {
            warn!("Activation already in progress: {:?}", *state);
            return Ok(ActivationDecision::Deferred(
                "Activation already in progress".into(),
            ));
        }

        *state = ActivationState::Warming;
        info!(
            "ActivationCoordinator: Starting activation for bundle at {:?}",
            req.manifest_path
        );

        // Enforce Enterprise Profiles
        match config.enterprise_profile {
            dek_config::EnterpriseProfile::Enterprise
            | dek_config::EnterpriseProfile::Regulated
                if config.activation_mode != dek_config::ActivationMode::Full =>
            {
                *state = ActivationState::Failed;
                return Ok(ActivationDecision::Rejected(
                    ActivationError::ProfileViolation(format!(
                        "Profile {:?} requires Full activation mode",
                        config.enterprise_profile
                    )),
                ));
            }
            _ => {}
        }

        // 1. Read Payload
        let payload_str = match std::fs::read_to_string(&req.manifest_path) {
            Ok(s) => s,
            Err(e) => {
                *state = ActivationState::Failed;
                return Ok(ActivationDecision::Rejected(ActivationError::SchemaFailed(
                    e.to_string(),
                )));
            }
        };

        // Parse metadata from payload
        let payload: serde_json::Value = match serde_json::from_str(&payload_str) {
            Ok(p) => p,
            Err(e) => {
                *state = ActivationState::Failed;
                return Ok(ActivationDecision::Rejected(ActivationError::SchemaFailed(
                    e.to_string(),
                )));
            }
        };

        // 2. Hydrate Runtime
        let router = match crate::hydration::hydrate_runtime(config, &payload).await {
            Ok(r) => r,
            Err(e) => {
                *state = ActivationState::Failed;
                return Ok(ActivationDecision::Rejected(e));
            }
        };

        // 3. Preflight Tests
        *state = ActivationState::Preflighting;
        if let Err(e) = crate::preflight::run_preflight_tests(config, router.clone()).await {
            *state = ActivationState::Failed;
            return Ok(ActivationDecision::Rejected(e));
        }

        // 4. Activate modes and update LKG
        *state = ActivationState::Activating;
        crate::lkg::update_lkg();
        if let Err(e) =
            crate::modes::handle_activation_mode(&req.manifest_path, config.activation_mode.clone())
        {
            *state = ActivationState::Failed;
            return Ok(ActivationDecision::Rejected(e));
        }

        // 5. Atomic Snapshot Swap
        let gen = self
            .generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut metadata = crate::snapshot::DekMetadata {
            enterprise_profile: config.enterprise_profile.clone(),
            ..Default::default()
        };
        if let Some(t) = payload.get("tenant_id").and_then(|v| v.as_str()) {
            metadata.tenant_id = t.to_string();
        }
        if let Some(s) = payload.get("spiffe_id").and_then(|v| v.as_str()) {
            metadata.spiffe_id = Some(s.to_string());
        }
        if let Some(jwt_cfg) = payload.get("jwt_config") {
            if let Some(pem) = jwt_cfg.get("public_key_pem").and_then(|v| v.as_str()) {
                metadata.jwt_public_key_pem = Some(pem.to_string());
            }
            if let Some(jwks_val) = jwt_cfg.get("jwks") {
                if let Ok(jwks) = serde_json::from_value(jwks_val.clone()) {
                    metadata.jwks = Some(jwks);
                }
            }
            if let Some(issuer) = jwt_cfg.get("issuer_url").and_then(|v| v.as_str()) {
                metadata.issuer_url = Some(issuer.to_string());
            }
            if let Some(aud_val) = jwt_cfg.get("audience") {
                if let Ok(aud) = serde_json::from_value(aud_val.clone()) {
                    metadata.audience = Some(aud);
                }
            }
        }

        let current_plugin_host = self.snapshot.load().plugin_host.clone();
        let new_snapshot = RuntimeSnapshot::new(
            gen,
            format!("{}_{}", req.tenant_id, req.device_id),
            0,
            router,
            metadata,
            current_plugin_host,
        );
        new_snapshot.router.clear_caches().await;
        self.snapshot.store(Arc::new(new_snapshot));

        *state = ActivationState::Idle;
        info!(
            "ActivationCoordinator: Successfully swapped to generation {}",
            gen
        );

        Ok(ActivationDecision::Activated(ActivationReceipt {
            timestamp_version: 0,
            bundle_id: format!("{}_{}", req.tenant_id, req.device_id),
            mode: config.activation_mode.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::ActivationSource;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_sequential_reloads() {
        let plugin_host = Arc::new(
            dek_wasm_host::WasmtimePluginHost::new(std::collections::HashMap::new()).unwrap(),
        );
        let router = Arc::new(dek_policy_router::PolicyRouter::new());
        let metadata = crate::snapshot::DekMetadata::default();
        let snapshot = RuntimeSnapshot::new(0, "test_0".into(), 0, router, metadata, plugin_host);

        let coordinator = ActivationCoordinator::new(snapshot);

        for _i in 1..=100 {
            let req = ActivationRequest {
                source: ActivationSource::LocalAdmin,
                tenant_id: "t1".into(),
                device_id: "d1".into(),
                manifest_path: "mock".into(),
            };

            // Simulate the hot-reload
            let gen = coordinator
                .generation
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let metadata = crate::snapshot::DekMetadata::default();
            let new_router = Arc::new(dek_policy_router::PolicyRouter::new());
            let current_plugin_host = coordinator.snapshot.load().plugin_host.clone();

            let new_snapshot = RuntimeSnapshot::new(
                gen,
                format!("{}_{}", req.tenant_id, req.device_id),
                0,
                new_router,
                metadata,
                current_plugin_host,
            );
            new_snapshot.router.clear_caches().await;
            coordinator.snapshot.store(Arc::new(new_snapshot));
        }

        assert_eq!(
            coordinator
                .generation
                .load(std::sync::atomic::Ordering::SeqCst),
            101
        );
    }
}
