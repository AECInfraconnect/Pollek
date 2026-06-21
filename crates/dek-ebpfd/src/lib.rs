// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! dek-ebpfd — userspace supervisor for the DEK network Control Point.
//!
//! Production fix (HIGH): the loader now RETURNS an `EbpfHandle` that owns the
//! `aya::Ebpf` object for the caller to store for the process lifetime. The
//! previous version dropped `bpf` at function return, which made aya detach and
//! unload the cgroup programs immediately. The handle's `Drop` aborts the
//! background tasks and then drops `Ebpf`, giving a clean teardown on shutdown.
//!
//! DNS-observe: attaches `dek_dns_capture` (cgroup/skb) and reads the ring
//! buffer, parsing each datagram with hickory into a `DnsObservation` (qname +
//! resolved A/AAAA records + TTL, floored). Observe-only; never blocks traffic.

use serde::Serialize;
use std::net::IpAddr;

pub fn probe_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        true
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

pub mod dns_cache;
pub mod map_updater;

/// A parsed DNS observation handed to userspace consumers (telemetry / IP map).
#[derive(Debug, Clone, Serialize)]
pub struct DnsObservation {
    pub cgroup_id: u64,
    pub qname: String,
    pub qtype: String,
    pub answers: Vec<ResolvedRecord>,
    pub is_response: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedRecord {
    pub ip: IpAddr,
    pub ttl_secs: u32,
}

/// Floor applied to record TTLs before they drive any IP-map entry. Guards the
/// kernel-map TTL race and prevents churn from hostile/short TTLs.
pub const MIN_TTL_FLOOR_SECS: u32 = 30;

#[cfg(target_os = "linux")]
pub use linux::{set_runtime_default_action, start_ebpfd_supervisor, EbpfHandle, BPFFS_PATH};

#[cfg(target_os = "linux")]
mod linux {
    use super::{DnsObservation, ResolvedRecord, MIN_TTL_FLOOR_SECS};
    use anyhow::{Context, Result};
    use aya::programs::{CgroupAttachMode, CgroupSkb, CgroupSkbAttachType, CgroupSockAddr};
    use aya::Ebpf;
    use hickory_proto::op::{Message, MessageType};
    use hickory_proto::rr::RData;
    use std::fs;
    use std::net::IpAddr;
    use tokio::sync::mpsc::Sender;
    use tokio::task::{self, JoinHandle};
    use tracing::{info, warn};

    pub const BPFFS_PATH: &str = "/sys/fs/bpf/pollen-dek";

    /// Owns the loaded eBPF object + background tasks for the process lifetime.
    /// Dropping it aborts the tasks and detaches all programs cleanly.
    pub struct EbpfHandle {
        tasks: Vec<JoinHandle<()>>, // declared first => aborted before `_bpf` drops
        _bpf: Ebpf,
    }

    impl Drop for EbpfHandle {
        fn drop(&mut self) {
            for t in &self.tasks {
                t.abort();
            }
            // `_bpf` drops here -> aya detaches cgroup programs + closes maps.
            info!("eBPFD: detached programs and released maps (clean teardown).");
        }
    }

    pub fn set_runtime_default_action(action: u32) -> Result<()> {
        use aya::maps::{Array, MapData};
        let pin_path = format!("{}/RUNTIME_MODE", BPFFS_PATH);
        let map_data = MapData::from_pin(&pin_path).context("load pinned RUNTIME_MODE")?;
        let mut map: Array<_, u32> = Array::try_from(map_data)?;
        map.set(0, action, 0).context("set RUNTIME_MODE action")?;
        info!("eBPFD: set_runtime_default_action to {}", action);
        Ok(())
    }

    /// Load + attach the eBPF programs and start the DNS reader. Returns a handle
    /// the caller MUST keep alive (store it in the supervisor). Dropping it tears
    /// everything down.
    ///
    /// `obs_tx`, if provided, receives every parsed DNS observation.
    pub async fn start_ebpfd_supervisor(
        cgroup_path: &str,
        obs_tx: Option<Sender<DnsObservation>>,
    ) -> Result<EbpfHandle> {
        info!("Starting eBPFD Supervisor (network Control Point)...");

        // Use a helper to securely create directories with 0o700 permissions
        let create_secure_dir = |path: &str| -> Result<()> {
            fs::create_dir_all(path)?;
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path)?.permissions();
            perms.set_mode(0o700);
            fs::set_permissions(path, perms)?;
            Ok(())
        };

        if let Err(e) = create_secure_dir(BPFFS_PATH) {
            warn!(
                "Could not securely create BPFFS path {} ({}); is /sys/fs/bpf mounted?",
                BPFFS_PATH, e
            );
        }
        if let Err(e) = create_secure_dir(cgroup_path) {
            warn!(
                "Could not securely create supervised cgroup {} ({})",
                cgroup_path, e
            );
        } else {
            info!("Scoped cgroup securely ready at {}", cgroup_path);
        }

        // Bytecode embedded at compile time (replace dummy.o with the real
        // artifact in CI: cargo build -p dek-ebpf-prog --target bpfel-unknown-none).
        let bpf_bytes: &[u8] =
            aya::include_bytes_aligned!(concat!(env!("OUT_DIR"), "/dek-ebpf-prog"));
        if bpf_bytes.is_empty() {
            warn!("eBPF bytecode is empty (placeholder). Returning an inert handle.");
            // Still return a handle (with an empty Ebpf) is not possible; bail soft.
            anyhow::bail!("eBPF bytecode placeholder is empty; build dek-ebpf-prog first");
        }

        let mut bpf = Ebpf::load(bpf_bytes).context("load eBPF object")?;

        // Pin policy maps so they persist / can be updated out-of-band.
        for name in [
            "VERDICT_MAP",
            "PORTS_MAP",
            "CGROUP_POLICY_MAP",
            "EVENTS",
            "RUNTIME_MODE",
            "DNS_IP_CACHE_V4",
        ] {
            if let Some(map) = bpf.map_mut(name) {
                let pin = format!("{}/{}", BPFFS_PATH, name);
                let _ = fs::remove_file(&pin);
                if let Err(e) = map.pin(&pin) {
                    warn!("pin {} failed: {}", name, e);
                }
            }
        }

        // ---- connect4 guardrail (kept; permissive until enforcement) ----
        if let Some(prog) = bpf.program_mut("dek_connect4") {
            let p: &mut CgroupSockAddr = prog.try_into().context("connect4 program")?;
            p.load().context("load connect4")?;
            let cg = fs::File::open(cgroup_path).context("open cgroup (connect4)")?;
            p.attach(cg, CgroupAttachMode::Single)
                .context("attach connect4")?;
            info!("Attached cgroup/connect4 to {}", cgroup_path);
        }

        // ---- DNS capture on egress (queries) + ingress (responses) ----
        if let Some(prog) = bpf.program_mut("dek_dns_capture") {
            let p: &mut CgroupSkb = prog.try_into().context("dns_capture program")?;
            p.load().context("load dns_capture")?;
            let cg_e = fs::File::open(cgroup_path).context("open cgroup (egress)")?;
            p.attach(cg_e, CgroupSkbAttachType::Egress, CgroupAttachMode::Single)
                .context("attach egress")?;
            let cg_i = fs::File::open(cgroup_path).context("open cgroup (ingress)")?;
            p.attach(cg_i, CgroupSkbAttachType::Ingress, CgroupAttachMode::Single)
                .context("attach ingress")?;
            info!(
                "Attached cgroup/skb DNS capture (egress+ingress) to {}",
                cgroup_path
            );
        } else {
            warn!("dek_dns_capture program not found in object");
        }

        let mut tasks: Vec<JoinHandle<()>> = Vec::new();

        // ---- DNS ring buffer reader ----
        if let Some(dns_map) = bpf.take_map("DNS_EVENTS") {
            match aya::maps::RingBuf::try_from(dns_map) {
                Ok(ring) => {
                    if let Ok(mut async_fd) = tokio::io::unix::AsyncFd::new(ring) {
                        tasks.push(task::spawn(async move {
                            info!("eBPFD DNS ring-buffer reader started");
                            loop {
                                let mut guard = match async_fd.readable_mut().await {
                                    Ok(g) => g,
                                    Err(_) => break,
                                };
                                let ring = guard.get_inner_mut();
                                while let Some(item) = ring.next() {
                                    let bytes: &[u8] = &item;
                                    if bytes.len()
                                        < std::mem::size_of::<dek_ebpf_common::DnsCaptureEvent>()
                                    {
                                        continue;
                                    }
                                    let ev: dek_ebpf_common::DnsCaptureEvent = unsafe {
                                        std::ptr::read_unaligned(bytes.as_ptr() as *const _)
                                    };
                                    let dlen = (ev.len as usize).min(ev.data.len());
                                    if let Some(obs) = parse_dns(ev.cgroup_id, &ev.data[..dlen]) {
                                        log_observation(&obs);
                                        for rec in &obs.answers {
                                            if let std::net::IpAddr::V4(ipv4) = rec.ip {
                                                let _ = crate::dns_cache::update_dns_ip_cache_v4(
                                                    ipv4,
                                                    &obs.qname,
                                                    std::time::Duration::from_secs(
                                                        rec.ttl_secs as u64,
                                                    ),
                                                    0, // default policy_id
                                                    0, // default tenant_id
                                                );
                                            }
                                        }
                                        if let Some(tx) = &obs_tx {
                                            let _ = tx.try_send(obs); // drop if consumer is slow
                                        }
                                    }
                                }
                                guard.clear_ready();
                            }
                        }));
                    } else {
                        warn!("DNS_EVENTS -> AsyncFd failed");
                    }
                }
                Err(e) => warn!("DNS_EVENTS -> RingBuf failed: {e}"),
            }
        } else {
            warn!("DNS_EVENTS map not found");
        }

        // ---- DNS Cache Janitor Task ----
        tasks.push(task::spawn(async move {
            info!("eBPFD DNS Cache Janitor started");
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                let _ = crate::dns_cache::cleanup_expired_dns_cache_v4(10000);
            }
        }));

        info!("eBPFD ready; programs attached and held alive by EbpfHandle.");
        Ok(EbpfHandle { tasks, _bpf: bpf })
    }

    fn parse_dns(cgroup_id: u64, payload: &[u8]) -> Option<DnsObservation> {
        let msg = Message::from_vec(payload).ok()?;
        let q = msg.queries().first()?;
        let is_response = msg.header().message_type() == MessageType::Response;

        let mut answers = Vec::new();
        for rec in msg.answers() {
            let ttl = rec.ttl().max(MIN_TTL_FLOOR_SECS);
            match rec.data() {
                Some(RData::A(a)) => answers.push(ResolvedRecord {
                    ip: IpAddr::V4(a.0),
                    ttl_secs: ttl,
                }),
                Some(RData::AAAA(a)) => answers.push(ResolvedRecord {
                    ip: IpAddr::V6(a.0),
                    ttl_secs: ttl,
                }),
                _ => {}
            }
        }

        Some(DnsObservation {
            cgroup_id,
            qname: q.name().to_utf8(),
            qtype: format!("{:?}", q.query_type()),
            answers,
            is_response,
        })
    }

    fn log_observation(obs: &DnsObservation) {
        if obs.answers.is_empty() {
            info!(cgroup = obs.cgroup_id, qname = %obs.qname, qtype = %obs.qtype, "DNS query");
        } else {
            let ips: Vec<String> = obs
                .answers
                .iter()
                .map(|r| format!("{}({}s)", r.ip, r.ttl_secs))
                .collect();
            info!(cgroup = obs.cgroup_id, qname = %obs.qname, resolved = %ips.join(","), "DNS resolved");
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub struct EbpfHandle;

#[cfg(not(target_os = "linux"))]
pub async fn start_ebpfd_supervisor(
    _cgroup_path: &str,
    _obs_tx: Option<tokio::sync::mpsc::Sender<DnsObservation>>,
) -> anyhow::Result<EbpfHandle> {
    tracing::warn!("eBPFD supervisor is Linux-only; app-layer enforcement remains active.");
    Ok(EbpfHandle)
}
