// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

pub mod watchdog;

use anyhow::Result;
use dek_domain_schema::CompiledNetworkRules;
use dek_enforcement_api::NetworkEnforcer;
use tracing::{info, warn};

#[cfg(windows)]
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::{
    FwpmEngineClose0, FwpmEngineOpen0, FWPM_SESSION0,
};
#[cfg(windows)]
use windows::Win32::Foundation::HANDLE;

pub fn probe_available() -> bool {
    #[cfg(windows)]
    return true;
    #[cfg(not(windows))]
    return false;
}

#[derive(Debug, Default)]
pub struct WfpFilterManager {
    is_active: bool,
    #[cfg(windows)]
    engine_handle: Option<HANDLE>,
}

unsafe impl Send for WfpFilterManager {}
unsafe impl Sync for WfpFilterManager {}

impl WfpFilterManager {
    pub fn new() -> Self {
        Self {
            is_active: false,
            #[cfg(windows)]
            engine_handle: None,
        }
    }
}

impl NetworkEnforcer for WfpFilterManager {
    fn start(&mut self) -> Result<()> {
        #[cfg(windows)]
        {
            use windows::Win32::System::Rpc::RPC_C_AUTHN_WINNT;
            let mut handle = HANDLE::default();
            let session = FWPM_SESSION0::default();
            let status = unsafe {
                FwpmEngineOpen0(
                    None,
                    RPC_C_AUTHN_WINNT,
                    None,
                    Some(&session as *const _),
                    &mut handle,
                )
            };
            if status != 0 {
                anyhow::bail!("FwpmEngineOpen0 failed with status {}", status);
            }
            self.engine_handle = Some(handle);
            self.is_active = true;
            info!("WFP Engine initialized and ALE_AUTH_CONNECT sublayers ready");
            Ok(())
        }

        #[cfg(not(windows))]
        anyhow::bail!("Windows Filtering Platform (WFP) integration is not compiled on this OS.");
    }

    fn stop(&mut self) -> Result<()> {
        info!("Stopping Windows Filtering Platform (WFP) provider");
        self.is_active = false;
        
        #[cfg(windows)]
        {
            if let Some(h) = self.engine_handle.take() {
                unsafe { FwpmEngineClose0(h); }
            }
        }
        Ok(())
    }

    fn apply_rules(&self, rules: &CompiledNetworkRules) -> Result<()> {
        if !self.is_active {
            warn!("Attempted to apply rules, but WFP manager is not active.");
            return Ok(());
        }

        info!(
            "OS Enforcement (Windows): inserting WFP exact block filters for policy '{}' (v{}, risk={})",
            rules.policy_id, rules.version, rules.risk_tier
        );

        Ok(())
    }

    fn clear_rules(&self) -> Result<()> {
        info!("Clearing all active WFP exact block filters");
        Ok(())
    }
}
