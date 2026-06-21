#![allow(clippy::panic)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tracing::{error, info, warn};

use dek_activation::coordinator::ActivationCoordinator;
use dek_activation::snapshot::RuntimeSnapshot;
use dek_activation::{ActivationDecision, ActivationRequest, ActivationSource};
use dek_config::DekConfig;

pub struct ReloadCoordinator {
    pub activation: Arc<ActivationCoordinator>,
}

impl ReloadCoordinator {
    pub fn new() -> Self {
        let initial_router = std::sync::Arc::new(dek_policy_router::PolicyRouter::new());
        let initial_plugin_host = std::sync::Arc::new(
            dek_wasm_host::WasmPluginHost::new(dek_wasm_host::WasmHostConfig::default())
                .unwrap_or_else(|_| panic!("Failed to init dummy WasmPluginHost")),
        );
        let initial_snapshot = RuntimeSnapshot::new(
            0,
            "initial".into(),
            0,
            initial_router,
            dek_activation::snapshot::DekMetadata::default(),
            initial_plugin_host,
        );
        Self {
            activation: Arc::new(ActivationCoordinator::new(initial_snapshot)),
        }
    }

    pub async fn process_staged_bundle(
        &self,
        config: &DekConfig,
        staged_path: &Path,
    ) -> Result<()> {
        info!("ReloadCoordinator: Delegating activation to dek-activation crate");

        let req = ActivationRequest {
            manifest_path: staged_path.to_path_buf(),
            source: ActivationSource::PollSync, // Hardcoded for now until we have CloudPush
            tenant_id: "system".into(),         // We could parse from config or bundle
            device_id: "system".into(),         // Could be populated from actual bootstrap
        };

        match self.activation.process_activation(req, config).await? {
            ActivationDecision::Activated(receipt) => {
                info!("Activation successful: {:?}", receipt);
                Ok(())
            }
            ActivationDecision::Rejected(err) => {
                error!("Activation rejected: {}", err);
                Err(anyhow::anyhow!("Activation rejected: {}", err))
            }
            ActivationDecision::Deferred(msg) => {
                warn!("Activation deferred: {}", msg);
                Err(anyhow::anyhow!("Activation deferred: {}", msg))
            }
        }
    }
}
