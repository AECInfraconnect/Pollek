// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::Result;
use dek_domain_schema::CompiledNetworkRules;
use dek_enforcement_api::NetworkEnforcer;
use tracing::{info, warn};

// Stub implementation for cross-compilation testing.
// In reality, this communicates with the macOS Network Extension via XPC or Unix Sockets.

pub fn probe_available() -> bool {
    #[cfg(target_os = "macos")]
    return true;
    #[cfg(not(target_os = "macos"))]
    return false;
}

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
        #[cfg(target_os = "macos")]
        {
            info!("Initializing macOS Network Extension IPC client (Beta/Dev Prototype)");
            warn!("macOS L4 enforcement is NOT production-ready. Requires signing, entitlement approval, notarization, and MDM deployment.");
            // Skeleton: Connect to Unix Domain Socket provided by NEFilterDataProvider
            let _socket_path = "/var/run/pollen/nefilter.sock";
            self.connected = true;
            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        anyhow::bail!("macOS Network Extension integration is not compiled on this OS.");
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
