#![allow(unsafe_code)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::Result;
use dek_domain_schema::CompiledNetworkRules;
#[allow(unused_imports)]
use dek_enforcement_api::NetworkEnforcer;
use std::sync::Arc;
#[allow(unused_imports)]
use tracing::{info, warn};

pub fn probe_available() -> bool {
    #[cfg(target_os = "macos")]
    return true;
    #[cfg(not(target_os = "macos"))]
    return false;
}

#[allow(dead_code)]
pub struct NeFilterClient {
    connected: bool,
    socket_path: String,
    spool: Option<Arc<dek_secure_spool::Spool>>,
    #[cfg(target_os = "macos")]
    stream: Option<std::os::unix::net::UnixStream>,
}

impl NeFilterClient {
    pub fn new(spool: Option<Arc<dek_secure_spool::Spool>>) -> Self {
        Self {
            connected: false,
            socket_path: "/var/run/pollen/nefilter.sock".into(),
            spool,
            #[cfg(target_os = "macos")]
            stream: None,
        }
    }
}

impl dek_enforcement_api::NetworkEnforcer for NeFilterClient {
    #[allow(unused_variables)]
    fn apply_rules(&self, rules: &CompiledNetworkRules) -> Result<()> {
        if !self.connected {
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        {
            use std::io::Write;
            let payload = serde_json::to_vec(&NeRuleMessage::from_compiled(rules))?;
            if let Some(stream) = &self.stream {
                let mut s = stream.try_clone()?;
                s.write_all(&(payload.len() as u32).to_be_bytes())?;
                s.write_all(&payload)?;
            }
        }
        Ok(())
    }

    fn clear_rules(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        if let Some(stream) = &self.stream {
            use std::io::Write;
            let mut s = stream.try_clone()?;
            let clear = serde_json::to_vec(&NeRuleMessage::clear())?;
            let _ = s.write_all(&(clear.len() as u32).to_be_bytes());
            let _ = s.write_all(&clear);
        }
        Ok(())
    }

    fn start(&mut self) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            use std::os::unix::net::UnixStream;
            warn!("macOS NE: requires signing + entitlement + notarization + MDM (dev prototype)");
            match UnixStream::connect(&self.socket_path) {
                Ok(s) => {
                    self.stream = Some(s);
                    self.connected = true;
                    info!("connected to PollenDEKNetworkExtension");
                    Ok(())
                }
                Err(e) => anyhow::bail!("NE socket connect failed: {e}"),
            }
        }
        #[cfg(not(target_os = "macos"))]
        anyhow::bail!("macOS NE not compiled on this OS");
    }

    fn stop(&mut self) -> Result<()> {
        self.connected = false;
        #[cfg(target_os = "macos")]
        {
            self.stream = None;
        }
        Ok(())
    }
}

/// message format ระหว่าง container app <-> NEFilterDataProvider
#[derive(serde::Serialize, serde::Deserialize)]
pub struct NeRuleMessage {
    pub action: String, // "apply" | "clear"
    pub policy_id: String,
    pub block_domains: Vec<String>,
    pub block_cidrs: Vec<String>,
    pub block_ports: Vec<u16>,
}

impl NeRuleMessage {
    pub fn from_compiled(rules: &CompiledNetworkRules) -> Self {
        Self {
            action: "apply".into(),
            policy_id: rules.policy_id.clone(),
            block_domains: vec![],
            block_cidrs: vec![],
            block_ports: vec![],
        }
    }
    pub fn clear() -> Self {
        Self {
            action: "clear".into(),
            policy_id: String::new(),
            block_domains: vec![],
            block_cidrs: vec![],
            block_ports: vec![],
        }
    }
}
