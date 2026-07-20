// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! map_updater.rs — the userspace bridge that writes compiled policy into the
//! kernel eBPF maps.
//!
//! The kernel program (`dek-ebpf-prog`) already enforces: `dek_connect4` reads
//! `CGROUP_POLICY_MAP`, `VERDICT_MAP` (LPM), and `PORTS_MAP` and returns
//! `verdict.allow` (0 drops the connect). What was missing was this side —
//! opening the pinned maps and writing real `PolicyVerdict` entries so the
//! program has something to act on. On Linux with the maps pinned under
//! `BPFFS_PATH` and the `kernel-maps` feature on, `apply_update` performs the
//! real write; otherwise it degrades to a validated no-op so callers on
//! unsupported hosts stay fail-open.

use anyhow::{bail, Result};
use dek_domain_schema::ebpf::{EbpfMapUpdate, UpdateSource};
use tracing::info;

/// The canonical pinned map a logical update targets, plus the parsed key.
/// Kept OS-agnostic so the routing/translation logic is unit-testable anywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapTarget {
    /// LPM trie keyed by (prefix_len, host-order IPv4). Pinned name `VERDICT_MAP`.
    VerdictLpmV4 { prefix_len: u32, ip_host: u32 },
    /// Hash keyed by host-order destination port. Pinned name `PORTS_MAP`.
    Ports { port: u16 },
    /// Hash keyed by cgroup id. Pinned name `CGROUP_POLICY_MAP`.
    CgroupPolicy { cgroup_id: u64 },
}

impl MapTarget {
    /// The pinned map file name under `BPFFS_PATH`.
    pub fn pinned_name(&self) -> &'static str {
        match self {
            MapTarget::VerdictLpmV4 { .. } => "VERDICT_MAP",
            MapTarget::Ports { .. } => "PORTS_MAP",
            MapTarget::CgroupPolicy { .. } => "CGROUP_POLICY_MAP",
        }
    }
}

/// A parsed `{ allow, log_event }` verdict, OS-agnostic for testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedVerdict {
    pub allow: u8,
    pub log_event: u8,
}

pub struct MapUpdater {
    pub tenant_id: String,
    pub device_id: String,
    pub current_generation: u64,
}

impl MapUpdater {
    pub fn new(tenant_id: String, device_id: String, current_generation: u64) -> Self {
        Self {
            tenant_id,
            device_id,
            current_generation,
        }
    }

    pub fn validate_update(&self, update: &EbpfMapUpdate) -> Result<()> {
        if update.tenant_id != self.tenant_id {
            bail!("Unauthorized map update: tenant_id mismatch");
        }
        if update.device_id != self.device_id {
            bail!("Unauthorized map update: device_id mismatch");
        }
        if update.generation < self.current_generation {
            bail!("Unauthorized map update: generation rollback attempt");
        }

        // Hybrid signature check: out-of-band updates always require a signature;
        // high-risk maps require one even from a bundle source. Matched on the
        // raw map name (unchanged from the original) so bundle-verified compiled
        // updates from the network loop keep their existing behavior.
        let requires_signature = match update.source {
            UpdateSource::OutOfBand => true,
            UpdateSource::Bundle => matches!(
                update.map_name.as_str(),
                "VERDICT_MAP" | "PORTS_MAP" | "CGROUP_POLICY_MAP"
            ),
        };
        if requires_signature && update.signature.is_none() {
            bail!("Unauthorized map update: signature strictly required for this source/map");
        }

        Ok(())
    }

    /// Parse a validated update into a concrete map target + verdict. This is
    /// the pure translation the kernel writer consumes; it is fully testable
    /// without a kernel.
    pub fn plan_update(&self, update: &EbpfMapUpdate) -> Result<(MapTarget, ParsedVerdict)> {
        let target = match canonical_map_name(&update.map_name) {
            "VERDICT_MAP" => {
                let cidr = update
                    .key
                    .get("cidr")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("VERDICT_MAP update requires key.cidr"))?;
                let (ip_host, prefix_len) = parse_cidr_v4(cidr)?;
                MapTarget::VerdictLpmV4 {
                    prefix_len,
                    ip_host,
                }
            }
            "PORTS_MAP" => {
                let port = update
                    .key
                    .get("port")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow::anyhow!("PORTS_MAP update requires numeric key.port"))?;
                if port > u16::MAX as u64 {
                    bail!("PORTS_MAP port {port} out of range");
                }
                MapTarget::Ports { port: port as u16 }
            }
            "CGROUP_POLICY_MAP" => {
                let cgroup_id = update
                    .key
                    .get("cgroup_id")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        anyhow::anyhow!("CGROUP_POLICY_MAP update requires numeric key.cgroup_id")
                    })?;
                MapTarget::CgroupPolicy { cgroup_id }
            }
            other => bail!("Unknown or unsupported map: {other}"),
        };
        let verdict = parse_verdict(&update.value)?;
        Ok((target, verdict))
    }

    pub fn apply_update(&mut self, update: EbpfMapUpdate) -> Result<()> {
        self.validate_update(&update)?;

        // Advance generation to prevent replay of older updates.
        if update.generation > self.current_generation {
            self.current_generation = update.generation;
        }

        // Parse first so a malformed update fails before we touch the kernel.
        let (target, verdict) = self.plan_update(&update)?;
        let is_delete = update.operation.eq_ignore_ascii_case("delete");

        self.write_to_kernel(&target, verdict, is_delete)?;

        info!(
            "OS Enforcement: applied compiled update to eBPF map '{}' (gen={}, src={:?}, delete={})",
            target.pinned_name(),
            update.generation,
            update.source,
            is_delete
        );
        Ok(())
    }

    /// Perform the real pinned-map write on Linux; a validated no-op elsewhere.
    #[cfg(all(target_os = "linux", feature = "kernel-maps"))]
    fn write_to_kernel(
        &self,
        target: &MapTarget,
        verdict: ParsedVerdict,
        is_delete: bool,
    ) -> Result<()> {
        use aya::maps::{HashMap as AyaHashMap, LpmTrie, Map, MapData};
        use dek_ebpf_common::PolicyVerdict;

        let pin_path = format!("{}/{}", crate::BPFFS_PATH, target.pinned_name());
        let map_data = MapData::from_pin(&pin_path)
            .map_err(|e| anyhow::anyhow!("open pinned map {pin_path}: {e}"))?;
        let pv = PolicyVerdict {
            allow: verdict.allow,
            log_event: verdict.log_event,
        };

        match target {
            MapTarget::VerdictLpmV4 {
                prefix_len,
                ip_host,
            } => {
                let mut map: LpmTrie<_, u32, PolicyVerdict> =
                    LpmTrie::try_from(Map::LpmTrie(map_data))?;
                let key = aya::maps::lpm_trie::Key::new(*prefix_len, *ip_host);
                if is_delete {
                    map.remove(&key)?;
                } else {
                    map.insert(&key, pv, 0)?;
                }
            }
            MapTarget::Ports { port } => {
                let mut map: AyaHashMap<_, u16, PolicyVerdict> =
                    AyaHashMap::try_from(Map::HashMap(map_data))?;
                if is_delete {
                    map.remove(port)?;
                } else {
                    map.insert(port, pv, 0)?;
                }
            }
            MapTarget::CgroupPolicy { cgroup_id } => {
                let mut map: AyaHashMap<_, u64, PolicyVerdict> =
                    AyaHashMap::try_from(Map::HashMap(map_data))?;
                if is_delete {
                    map.remove(cgroup_id)?;
                } else {
                    map.insert(cgroup_id, pv, 0)?;
                }
            }
        }
        Ok(())
    }

    /// Off-Linux, or when the `kernel-maps` feature is disabled: validated
    /// no-op. The kernel program (if any) keeps enforcing on its current map
    /// contents, and unsupported hosts stay fail-open by design.
    #[cfg(not(all(target_os = "linux", feature = "kernel-maps")))]
    fn write_to_kernel(
        &self,
        target: &MapTarget,
        _verdict: ParsedVerdict,
        _is_delete: bool,
    ) -> Result<()> {
        tracing::warn!(
            "kernel-maps writer unavailable on this build; validated update to {} not applied",
            target.pinned_name()
        );
        Ok(())
    }
}

/// Accept both the canonical pinned names and the compiler's logical aliases
/// (e.g. `egress_lpm_v4` -> `VERDICT_MAP`).
pub fn canonical_map_name(name: &str) -> &str {
    match name {
        "egress_lpm_v4" | "VERDICT_MAP" => "VERDICT_MAP",
        "egress_ports" | "PORTS_MAP" => "PORTS_MAP",
        "cgroup_policy" | "CGROUP_POLICY_MAP" => "CGROUP_POLICY_MAP",
        other => other,
    }
}

/// Parse `"10.0.0.0/8"` into (host-order u32 ip, prefix_len). The kernel keys
/// the LPM trie with host-order IPv4 (`u32::from_be(user_ip4)`), so we match.
pub fn parse_cidr_v4(cidr: &str) -> Result<(u32, u32)> {
    let (ip_part, prefix_part) = cidr
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("CIDR '{cidr}' missing '/prefix'"))?;
    let ip: std::net::Ipv4Addr = ip_part
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid IPv4 in CIDR '{cidr}'"))?;
    let prefix_len: u32 = prefix_part
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid prefix in CIDR '{cidr}'"))?;
    if prefix_len > 32 {
        bail!("CIDR prefix {prefix_len} > 32");
    }
    Ok((u32::from(ip), prefix_len))
}

fn parse_verdict(value: &serde_json::Value) -> Result<ParsedVerdict> {
    let allow = value
        .get("allow")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("verdict requires numeric 'allow'"))?;
    let log_event = value.get("log_event").and_then(|v| v.as_u64()).unwrap_or(0);
    if allow > 1 {
        bail!("verdict 'allow' must be 0 or 1, got {allow}");
    }
    Ok(ParsedVerdict {
        allow: allow as u8,
        log_event: u8::from(log_event != 0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn updater() -> MapUpdater {
        MapUpdater::new("t".into(), "d".into(), 0)
    }

    fn base_update(map: &str, key: serde_json::Value, value: serde_json::Value) -> EbpfMapUpdate {
        EbpfMapUpdate {
            schema_version: "1.0".into(),
            map_name: map.into(),
            operation: "insert".into(),
            source: UpdateSource::Bundle,
            tenant_id: "t".into(),
            device_id: "d".into(),
            generation: 1,
            key,
            value,
            signature: Some("sig".into()),
        }
    }

    #[test]
    fn parses_cidr_to_host_order_ip_and_prefix() {
        let expected = u32::from(std::net::Ipv4Addr::new(10, 0, 0, 0));
        assert_eq!(parse_cidr_v4("10.0.0.0/8").ok(), Some((expected, 8)));
        assert!(parse_cidr_v4("10.0.0.0").is_err());
        assert!(parse_cidr_v4("999.0.0.0/8").is_err());
        assert!(parse_cidr_v4("10.0.0.0/33").is_err());
    }

    #[test]
    fn canonical_names_map_aliases() {
        assert_eq!(canonical_map_name("egress_lpm_v4"), "VERDICT_MAP");
        assert_eq!(canonical_map_name("VERDICT_MAP"), "VERDICT_MAP");
        assert_eq!(canonical_map_name("PORTS_MAP"), "PORTS_MAP");
    }

    #[test]
    fn plans_verdict_lpm_update() {
        let up = base_update(
            "egress_lpm_v4",
            json!({ "cidr": "192.168.1.0/24" }),
            json!({ "allow": 0, "log_event": 1 }),
        );
        let expected = MapTarget::VerdictLpmV4 {
            prefix_len: 24,
            ip_host: u32::from(std::net::Ipv4Addr::new(192, 168, 1, 0)),
        };
        assert_eq!(
            updater().plan_update(&up).ok(),
            Some((
                expected,
                ParsedVerdict {
                    allow: 0,
                    log_event: 1
                }
            ))
        );
    }

    #[test]
    fn plans_ports_and_cgroup_updates() {
        let ports = base_update("PORTS_MAP", json!({ "port": 443 }), json!({ "allow": 1 }));
        assert_eq!(
            updater().plan_update(&ports).ok(),
            Some((
                MapTarget::Ports { port: 443 },
                ParsedVerdict {
                    allow: 1,
                    log_event: 0
                }
            ))
        );

        let cg = base_update(
            "CGROUP_POLICY_MAP",
            json!({ "cgroup_id": 12345 }),
            json!({ "allow": 0 }),
        );
        let planned = updater().plan_update(&cg).ok().map(|(t, _)| t);
        assert_eq!(planned, Some(MapTarget::CgroupPolicy { cgroup_id: 12345 }));
    }

    #[test]
    fn rejects_malformed_updates() {
        let bad = base_update("PORTS_MAP", json!({ "port": 80 }), json!({ "allow": 5 }));
        assert!(updater().plan_update(&bad).is_err());
        let missing = base_update("VERDICT_MAP", json!({}), json!({ "allow": 1 }));
        assert!(updater().plan_update(&missing).is_err());
        let unknown = base_update("MYSTERY_MAP", json!({ "x": 1 }), json!({ "allow": 1 }));
        assert!(updater().plan_update(&unknown).is_err());
    }

    #[test]
    fn apply_update_bumps_generation_and_is_validated() {
        let mut u = updater();
        let up = base_update("PORTS_MAP", json!({ "port": 22 }), json!({ "allow": 0 }));
        // On a build without the kernel-maps feature write_to_kernel is a no-op,
        // so this exercises validate + plan + generation bump end to end.
        assert!(u.apply_update(up).is_ok());
        assert_eq!(u.current_generation, 1);
    }

    #[test]
    fn rejects_generation_rollback_and_tenant_mismatch() {
        let mut u = MapUpdater::new("t".into(), "d".into(), 5);
        let old = base_update("PORTS_MAP", json!({ "port": 1 }), json!({ "allow": 1 }));
        assert!(u.apply_update(old).is_err());

        let wrong_tenant = EbpfMapUpdate {
            tenant_id: "other".into(),
            ..base_update("PORTS_MAP", json!({ "port": 1 }), json!({ "allow": 1 }))
        };
        assert!(u.validate_update(&wrong_tenant).is_err());
    }
}
