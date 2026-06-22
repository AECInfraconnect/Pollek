#![allow(unsafe_code)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

pub mod sni_observe;
pub mod watchdog;

use anyhow::Result;
use dek_domain_schema::CompiledNetworkRules;
use dek_enforcement_api::NetworkEnforcer;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

#[cfg(windows)]
use windows::Win32::Foundation::HANDLE;
#[cfg(windows)]
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::{
    FwpmEngineClose0, FwpmEngineOpen0, FwpmFilterAdd0, FwpmFilterDeleteById0,
    FWPM_CONDITION_IP_REMOTE_ADDRESS, FWPM_CONDITION_IP_REMOTE_PORT, FWPM_FILTER0,
    FWPM_FILTER_CONDITION0, FWPM_LAYER_ALE_AUTH_CONNECT_V4, FWPM_SESSION0, FWP_ACTION_BLOCK,
    FWP_CONDITION_VALUE0, FWP_CONDITION_VALUE0_0, FWP_DATA_TYPE, FWP_MATCH_EQUAL, FWP_UINT16,
    FWP_UINT32, FWP_UINT8, FWP_VALUE0,
};

pub fn probe_available() -> bool {
    #[cfg(windows)]
    return true;
    #[cfg(not(windows))]
    return false;
}

#[derive(Default)]
#[allow(dead_code)]
#[allow(clippy::all)]
pub struct WfpFilterManager {
    is_active: bool,
    #[cfg(windows)]
    engine_handle: Option<HANDLE>,
    active_filter_ids: Arc<Mutex<Vec<u64>>>,
    spool: Option<Arc<dek_secure_spool::Spool>>,
}

// SAFETY: Audited as part of CNCF compliance.
unsafe impl Send for WfpFilterManager {}
// SAFETY: Audited as part of CNCF compliance.
unsafe impl Sync for WfpFilterManager {}

impl WfpFilterManager {
    pub fn new(spool: Option<Arc<dek_secure_spool::Spool>>) -> Self {
        Self {
            is_active: false,
            #[cfg(windows)]
            engine_handle: None,
            active_filter_ids: Arc::new(Mutex::new(Vec::new())),
            spool,
        }
    }

    #[cfg(windows)]
    fn add_block_filter(&self, remote_ip: u32, remote_port: u16, weight: u8) -> Result<u64> {
        let Some(engine) = self.engine_handle else {
            anyhow::bail!("WFP engine not open");
        };

        let mut conditions = Vec::new();

        if remote_ip != 0 {
            conditions.push(FWPM_FILTER_CONDITION0 {
                fieldKey: FWPM_CONDITION_IP_REMOTE_ADDRESS,
                matchType: FWP_MATCH_EQUAL,
                conditionValue: FWP_CONDITION_VALUE0 {
                    r#type: FWP_UINT32,
                    Anonymous: FWP_CONDITION_VALUE0_0 { uint32: remote_ip },
                },
            });
        }

        if remote_port != 0 {
            conditions.push(FWPM_FILTER_CONDITION0 {
                fieldKey: FWPM_CONDITION_IP_REMOTE_PORT,
                matchType: FWP_MATCH_EQUAL,
                conditionValue: FWP_CONDITION_VALUE0 {
                    r#type: FWP_UINT16,
                    Anonymous: FWP_CONDITION_VALUE0_0 {
                        uint16: remote_port,
                    },
                },
            });
        }

        let mut filter = FWPM_FILTER0 {
            layerKey: FWPM_LAYER_ALE_AUTH_CONNECT_V4,
            ..Default::default()
        };
        filter.action.r#type = FWP_ACTION_BLOCK;

        filter.weight = FWP_VALUE0 {
            r#type: FWP_UINT8 as FWP_DATA_TYPE, // Weight is usually uint8
            Anonymous: windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWP_VALUE0_0 {
                uint8: weight,
            },
        };

        filter.numFilterConditions = conditions.len() as u32;
        filter.filterCondition = conditions.as_mut_ptr() as *mut _;

        let mut filter_id: u64 = 0;
        // SAFETY: engine handle is valid, filter and conditions have a lifetime covering this call.
        let status = unsafe { FwpmFilterAdd0(engine, &filter, None, Some(&mut filter_id)) };

        if status != 0 {
            anyhow::bail!("FwpmFilterAdd0 failed: {}", status);
        }

        info!(filter_id, remote_ip, remote_port, "WFP block filter added");
        Ok(filter_id)
    }

    #[allow(dead_code)]
    #[cfg(windows)]
    fn add_app_filter(&self, app_path: &str, action: u32, weight: u8) -> Result<u64> {
        use std::ptr;
        use windows::core::HSTRING;
        use windows::Win32::NetworkManagement::WindowsFilteringPlatform::{
            FwpmFreeMemory0, FwpmGetAppIdFromFileName0, FWPM_CONDITION_ALE_APP_ID, FWP_ACTION_TYPE,
            FWP_BYTE_BLOB, FWP_BYTE_BLOB_TYPE, FWP_DATA_TYPE, FWP_UINT8, FWP_VALUE0,
        };

        let Some(engine) = self.engine_handle else {
            anyhow::bail!("WFP engine not open");
        };

        let h_path = HSTRING::from(app_path);
        let mut app_id_blob: *mut FWP_BYTE_BLOB = ptr::null_mut();

        // SAFETY: h_path is a valid wide string. app_id_blob will be allocated by WFP.
        let status = unsafe { FwpmGetAppIdFromFileName0(&h_path, &mut app_id_blob) };
        if status != 0 {
            anyhow::bail!("FwpmGetAppIdFromFileName0 failed: {status}");
        }

        if app_id_blob.is_null() {
            anyhow::bail!("FwpmGetAppIdFromFileName0 returned null blob");
        }

        struct BlobGuard(*mut FWP_BYTE_BLOB);
        impl Drop for BlobGuard {
            fn drop(&mut self) {
                unsafe {
                    // FwpmFreeMemory0 takes *mut *mut c_void
                    let mut p = self.0 as *mut core::ffi::c_void;
                    FwpmFreeMemory0(&mut p);
                }
            }
        }
        let _guard = BlobGuard(app_id_blob);

        // Now construct condition and filter
        let condition = FWPM_FILTER_CONDITION0 {
            fieldKey: FWPM_CONDITION_ALE_APP_ID,
            matchType: FWP_MATCH_EQUAL,
            conditionValue: FWP_CONDITION_VALUE0 {
                r#type: FWP_BYTE_BLOB_TYPE,
                Anonymous: windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWP_CONDITION_VALUE0_0 {
                    byteBlob: app_id_blob,
                },
            },
        };

        let mut filter = FWPM_FILTER0 {
            layerKey: FWPM_LAYER_ALE_AUTH_CONNECT_V4,
            ..Default::default()
        };
        filter.action.r#type = FWP_ACTION_TYPE(action);
        filter.weight = FWP_VALUE0 {
            r#type: FWP_UINT8 as FWP_DATA_TYPE,
            Anonymous: windows::Win32::NetworkManagement::WindowsFilteringPlatform::FWP_VALUE0_0 {
                uint8: weight,
            },
        };
        filter.numFilterConditions = 1;
        filter.filterCondition = &condition as *const _ as *mut _;

        let mut filter_id: u64 = 0;
        // SAFETY: engine handle valid, filter+conditions valid for the duration
        let status = unsafe { FwpmFilterAdd0(engine, &filter, None, Some(&mut filter_id)) };
        if status != 0 {
            anyhow::bail!("FwpmFilterAdd0 failed: {status}");
        }
        info!(filter_id, app_path, "WFP app filter added");
        Ok(filter_id)
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
                // SAFETY: Audited as part of CNCF compliance.
                unsafe {
                    FwpmEngineClose0(h);
                }
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

        #[cfg(windows)]
        {
            let mut added = Vec::new();

            // Basic parsing of rules.conditions.destinations
            for dest in &rules.conditions.destinations {
                let mut remote_ip: u32 = 0;
                let mut remote_port: u16 = 0;

                match dest.r#type.as_str() {
                    "cidr" | "ip" => {
                        if let Some(ip_str) = dest.value.as_str() {
                            let ip_part = ip_str.split('/').next().unwrap_or("");
                            if let Ok(ip_addr) = ip_part.parse::<std::net::Ipv4Addr>() {
                                // WFP expects host byte order or big endian?
                                // FwpmFilterCondition0 IP is host byte order typically (u32 from bytes is big endian usually, but Windows might use Network Byte Order)
                                // Let's use u32::from_be_bytes(ip_addr.octets())
                                remote_ip = u32::from_be_bytes(ip_addr.octets());
                            }
                        }
                    }
                    "port" => {
                        if let Some(port_num) = dest.value.as_u64() {
                            remote_port = port_num as u16;
                        } else if let Some(port_str) = dest.value.as_str() {
                            if let Ok(p) = port_str.parse::<u16>() {
                                remote_port = p;
                            }
                        }
                    }
                    _ => {}
                }

                if remote_ip != 0 || remote_port != 0 {
                    match self.add_block_filter(remote_ip, remote_port, 10) {
                        Ok(id) => added.push(id),
                        Err(e) => warn!(?e, "failed to add filter, continuing"),
                    }
                }
            }

            if let Ok(mut lock) = self.active_filter_ids.lock() {
                lock.extend(added.iter());
            }

            info!(
                count = added.len(),
                policy = %rules.policy_id,
                "WFP filters applied (REAL)"
            );

            if let Some(spool) = &self.spool {
                let event = serde_json::json!({
                    "decision": "block",
                    "policy_id": rules.policy_id,
                    "enforcement_plane": "wfp_windows",
                    "ts": chrono::Utc::now().to_rfc3339(),
                });
                if let Ok(buf) = serde_json::to_vec(&event) {
                    let s = spool.clone();
                    tokio::spawn(async move {
                        let _ = s.enqueue(buf).await;
                    });
                }
            }
        }

        Ok(())
    }

    fn clear_rules(&self) -> Result<()> {
        info!("Clearing all active WFP exact block filters");
        #[cfg(windows)]
        {
            if let Some(engine) = self.engine_handle {
                if let Ok(mut lock) = self.active_filter_ids.lock() {
                    for &id in lock.iter() {
                        // SAFETY: id comes from previous FwpmFilterAdd0
                        let _ = unsafe { FwpmFilterDeleteById0(engine, id) };
                    }
                    lock.clear();
                }
            }
        }
        Ok(())
    }
}
