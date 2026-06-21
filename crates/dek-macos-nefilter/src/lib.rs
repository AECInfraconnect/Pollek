// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::Result;
use dek_domain_schema::CompiledNetworkRules;
use dek_enforcement_api::NetworkEnforcer;
use tracing::{info, warn};

// Stub implementation for cross-compilation testing.
// In reality, this communicates with the macOS Network Extension via XPC or Unix Sockets.

#[derive(Debug, Default)]
pub struct NeFilterClient {
    connected: bool,
}

impl NeFilterClient {
    pub fn new() -> Self {
        Self { connected: false }
    }
}

impl NetworkEnforcer for NeFilterClient {
    fn start(&mut self) -> Result<()> {
        anyhow::bail!(
            "macOS Network Extension integration is not compiled. The current build is a stub."
        );
    }

    fn stop(&mut self) -> Result<()> {
        info!("Disconnecting from PollenDEKNetworkExtension");
        self.connected = false;
        Ok(())
    }

    fn apply_rules(&self, rules: &CompiledNetworkRules) -> Result<()> {
        if !self.connected {
            warn!("Cannot push rules; NEFilterClient is not connected.");
            return Ok(());
        }

        info!(
            "OS Enforcement (macOS): pushing compiled rules to Pollen Network Extension for policy '{}' (v{}, risk={})",
            rules.policy_id, rules.version, rules.risk_tier
        );

        Ok(())
    }

    fn clear_rules(&self) -> Result<()> {
        info!("Clearing rules in macOS Network Extension");
        Ok(())
    }
}
