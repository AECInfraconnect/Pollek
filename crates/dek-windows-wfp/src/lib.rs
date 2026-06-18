// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

pub mod watchdog;

use anyhow::Result;
use dek_domain_schema::CompiledNetworkRules;
use dek_enforcement_api::NetworkEnforcer;
use tracing::{info, warn};

// In a real Windows environment, this would call Windows Filtering Platform (WFP) APIs.
// For now, we stub it to allow cross-compilation tests and development without Windows APIs failing.

pub fn probe_available() -> bool {
    false // Kept as stub for now
}

#[derive(Debug, Default)]
pub struct WfpFilterManager {
    is_active: bool,
}

impl WfpFilterManager {
    pub fn new() -> Self {
        Self { is_active: false }
    }
}

impl NetworkEnforcer for WfpFilterManager {
    fn start(&mut self) -> Result<()> {
        anyhow::bail!("Windows Filtering Platform (WFP) integration is not compiled. The current build is a stub.");
    }

    fn stop(&mut self) -> Result<()> {
        info!("Stopping Windows Filtering Platform (WFP) provider");
        self.is_active = false;
        Ok(())
    }

    fn apply_rules(&self, rules: &CompiledNetworkRules) -> Result<()> {
        if !self.is_active {
            warn!("Attempted to apply rules, but WFP manager is not active.");
            return Ok(());
        }

        info!(
            "OS Enforcement (Windows): inserting WFP filters for policy '{}' (v{}, risk={})",
            rules.policy_id, rules.version, rules.risk_tier
        );

        Ok(())
    }

    fn clear_rules(&self) -> Result<()> {
        info!("Clearing all active WFP filters");
        Ok(())
    }
}
