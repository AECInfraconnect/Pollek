#![allow(unsafe_code)]
// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use anyhow::Result;
use dek_domain_schema::{CompiledNetworkRules, NetworkGuardrailEffect};
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
            socket_path: "/var/run/pollek/nefilter.sock".into(),
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
                    info!("connected to PollekDEKNetworkExtension");
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NeRuleMessage {
    pub action: String, // "apply" | "clear"
    pub policy_id: String,
    pub block_domains: Vec<String>,
    pub block_cidrs: Vec<String>,
    pub block_ports: Vec<u16>,
}

impl NeRuleMessage {
    /// Map compiled network rules onto the NE wire message.
    ///
    /// The message schema only carries block lists, so the mapping is:
    /// - effect `DENY` populates the lists; `ALLOW`/`ALLOW_OR_DENY` are not
    ///   expressible in the schema and deliberately produce empty lists
    /// - destination type "domain" (string value) -> `block_domains`
    /// - destination type "cidr" (string value) -> `block_cidrs`
    /// - destination type "port" (JSON number, must fit u16) -> `block_ports`
    /// - any other destination type, or a value of the wrong shape, is
    ///   deliberately skipped with a `warn!` (never silently)
    /// - `targets` (agents/processes/users/devices) has no field in the
    ///   schema, so it is not mapped: the filter applies the block lists to
    ///   every flow it sees
    pub fn from_compiled(rules: &CompiledNetworkRules) -> Self {
        let mut block_domains = Vec::new();
        let mut block_cidrs = Vec::new();
        let mut block_ports = Vec::new();

        if rules.effect == NetworkGuardrailEffect::Deny {
            for dest in &rules.conditions.destinations {
                match dest.r#type.as_str() {
                    "domain" => match dest.value.as_str() {
                        Some(d) => block_domains.push(d.to_string()),
                        None => warn!(value = ?dest.value, "NE: skip domain destination with non-string value"),
                    },
                    "cidr" => match dest.value.as_str() {
                        Some(c) => block_cidrs.push(c.to_string()),
                        None => warn!(value = ?dest.value, "NE: skip cidr destination with non-string value"),
                    },
                    "port" => match dest.value.as_u64().and_then(|p| u16::try_from(p).ok()) {
                        Some(p) => block_ports.push(p),
                        None => warn!(value = ?dest.value, "NE: skip port destination with invalid value"),
                    },
                    other => warn!(dest_type = other, "NE: skip unknown destination type"),
                }
            }
        }

        Self {
            action: "apply".into(),
            policy_id: rules.policy_id.clone(),
            block_domains,
            block_cidrs,
            block_ports,
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

#[cfg(test)]
mod tests {
    use super::*;
    use dek_domain_schema::{
        NetworkConditions, NetworkDestination, NetworkFallback, NetworkGuardrailEffect,
        NetworkTargets,
    };

    fn rules(effect: NetworkGuardrailEffect, destinations: Vec<NetworkDestination>) -> CompiledNetworkRules {
        CompiledNetworkRules {
            policy_id: "pol-123".into(),
            policy_type: "NETWORK_EGRESS_GUARDRAIL".into(),
            version: 1,
            risk_tier: "high".into(),
            targets: NetworkTargets::default(),
            conditions: NetworkConditions {
                destinations,
                protocols: vec![],
                time_window: None,
            },
            effect,
            obligations: vec![],
            fallback: NetworkFallback {
                cloud_unavailable: "deny".into(),
                policy_stale: "deny".into(),
            },
        }
    }

    fn dest(r#type: &str, value: serde_json::Value) -> NetworkDestination {
        NetworkDestination {
            r#type: r#type.into(),
            value,
        }
    }

    #[test]
    fn from_compiled_maps_deny_destinations_into_block_lists() {
        let r = rules(
            NetworkGuardrailEffect::Deny,
            vec![
                dest("domain", serde_json::json!("evil.example.com")),
                dest("cidr", serde_json::json!("10.0.0.0/8")),
                dest("port", serde_json::json!(4444)),
            ],
        );
        let msg = NeRuleMessage::from_compiled(&r);
        assert_eq!(msg.action, "apply");
        assert_eq!(msg.policy_id, "pol-123");
        assert_eq!(msg.block_domains, vec!["evil.example.com"]);
        assert_eq!(msg.block_cidrs, vec!["10.0.0.0/8"]);
        assert_eq!(msg.block_ports, vec![4444]);
    }

    #[test]
    fn from_compiled_empty_rules_keeps_empty_lists() {
        let msg = NeRuleMessage::from_compiled(&rules(NetworkGuardrailEffect::Deny, vec![]));
        assert_eq!(msg.action, "apply");
        assert_eq!(msg.policy_id, "pol-123");
        assert!(msg.block_domains.is_empty());
        assert!(msg.block_cidrs.is_empty());
        assert!(msg.block_ports.is_empty());
    }

    #[test]
    fn from_compiled_allow_effect_blocks_nothing() {
        // the wire schema has only block lists; allow-type effects are
        // deliberately not mapped
        let r = rules(
            NetworkGuardrailEffect::Allow,
            vec![dest("domain", serde_json::json!("ok.example.com"))],
        );
        let msg = NeRuleMessage::from_compiled(&r);
        assert!(msg.block_domains.is_empty());
        assert!(msg.block_cidrs.is_empty());
        assert!(msg.block_ports.is_empty());
    }

    #[test]
    fn from_compiled_skips_unknown_and_malformed_destinations() {
        let r = rules(
            NetworkGuardrailEffect::Deny,
            vec![
                dest("geoip", serde_json::json!("XX")),
                dest("domain", serde_json::json!(42)),
                dest("port", serde_json::json!("443")),
                dest("port", serde_json::json!(70000)),
                dest("domain", serde_json::json!("kept.example.com")),
            ],
        );
        let msg = NeRuleMessage::from_compiled(&r);
        assert_eq!(msg.block_domains, vec!["kept.example.com"]);
        assert!(msg.block_cidrs.is_empty());
        assert!(msg.block_ports.is_empty());
    }

    #[test]
    fn wire_format_round_trips_with_length_prefix() {
        // same framing as apply_rules: u32 BE length + JSON payload
        let msg = NeRuleMessage::from_compiled(&rules(
            NetworkGuardrailEffect::Deny,
            vec![
                dest("domain", serde_json::json!("bad.example.com")),
                dest("cidr", serde_json::json!("192.0.2.0/24")),
                dest("port", serde_json::json!(8080)),
            ],
        ));
        let payload = serde_json::to_vec(&msg).unwrap();
        let mut framed = (payload.len() as u32).to_be_bytes().to_vec();
        framed.extend_from_slice(&payload);

        let len = u32::from_be_bytes(framed[..4].try_into().unwrap()) as usize;
        let decoded: NeRuleMessage = serde_json::from_slice(&framed[4..4 + len]).unwrap();
        assert_eq!(decoded, msg);
    }
}
