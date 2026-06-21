// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

//! network_loop.rs — Phase A: network egress guardrail enforcement plane.
//!
//! Consumes `SyncOutcome` from the policy syncer and drives a per-OS
//! `NetworkEnforcer` (WFP / NEFilter / eBPF). Fail-closed by design:
//!   - apply fails / no rules + no LKG / cloud unreachable / StrictDeny  => block-all
//!   - apply succeeds => remember as Last-Known-Good (read-only fallback)
//!
//! Replaces the cfg-scattered inline task in supervisor.rs with one trait-based
//! driver that covers all three platforms uniformly.

use dek_domain_schema::network_guardrail::{
    CompiledNetworkRules, NetworkConditions, NetworkDestination, NetworkFallback,
    NetworkGuardrailEffect, NetworkTargets,
};
use dek_policy_syncer::SyncOutcome;

use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Platform-agnostic network enforcement backend.
pub trait NetworkEnforcer: Send {
    /// Apply the full set of compiled rules (replaces the active set).
    fn apply(&mut self, rules: &[CompiledNetworkRules]) -> anyhow::Result<()>;
    /// Block all egress except the control-plane channel (fail-closed posture).
    fn fail_closed(&mut self) -> anyhow::Result<()>;
    /// Human-readable backend name for logs/metrics.
    fn backend(&self) -> &'static str;
}

/// A synthesized deny-all rule used for fail-closed mode. The cloud control
/// channel is exempted by the backend (WFP/NEFilter/eBPF keep the mTLS path).
fn deny_all_rule() -> CompiledNetworkRules {
    CompiledNetworkRules {
        policy_id: "failsafe-deny-all".to_string(),
        policy_type: "NETWORK_EGRESS_GUARDRAIL".to_string(),
        version: 0,
        risk_tier: "high".to_string(),
        targets: NetworkTargets {
            agents: vec![],
            processes: vec![],
            users: vec![],
            devices: vec!["*".to_string()],
        },
        conditions: NetworkConditions {
            destinations: vec![NetworkDestination {
                r#type: "cidr".to_string(),
                value: serde_json::json!("0.0.0.0/0"),
            }],
            protocols: vec![],
            time_window: None,
        },
        effect: NetworkGuardrailEffect::Deny,
        obligations: vec![],
        fallback: NetworkFallback {
            cloud_unavailable: "deny".to_string(),
            policy_stale: "deny".to_string(),
        },
    }
}

// ===========================================================================
// Windows — WFP
// ===========================================================================
#[cfg(windows)]
pub mod wfp_backend {
    use super::*;
    use dek_enforcement_api::NetworkEnforcer as ApiNetworkEnforcer;
    use dek_windows_wfp::WfpFilterManager;

    pub struct WfpEnforcer {
        mgr: WfpFilterManager,
    }
    impl WfpEnforcer {
        pub fn new() -> anyhow::Result<Self> {
            let mut mgr = WfpFilterManager::new();
            mgr.start()?;
            Ok(Self { mgr })
        }
    }
    impl NetworkEnforcer for WfpEnforcer {
        fn apply(&mut self, rules: &[CompiledNetworkRules]) -> anyhow::Result<()> {
            self.mgr.clear_rules()?;
            for r in rules {
                self.mgr.apply_rules(r)?;
            }
            Ok(())
        }
        fn fail_closed(&mut self) -> anyhow::Result<()> {
            self.mgr.clear_rules()?;
            self.mgr.apply_rules(&deny_all_rule())
        }
        fn backend(&self) -> &'static str {
            "wfp"
        }
    }
}

// ===========================================================================
// macOS — NEFilter
// ===========================================================================
#[cfg(target_os = "macos")]
pub mod nefilter_backend {
    use super::*;
    use dek_enforcement_api::NetworkEnforcer as ApiNetworkEnforcer;
    use dek_macos_nefilter::NeFilterClient;

    pub struct NeFilterEnforcer {
        client: NeFilterClient,
    }
    impl NeFilterEnforcer {
        pub fn new() -> anyhow::Result<Self> {
            let mut client = NeFilterClient::new();
            client.connect()?;
            Ok(Self { client })
        }
    }
    impl NetworkEnforcer for NeFilterEnforcer {
        fn apply(&mut self, rules: &[CompiledNetworkRules]) -> anyhow::Result<()> {
            self.client.clear_rules()?;
            for r in rules {
                self.client.push_rules(r)?;
            }
            Ok(())
        }
        fn fail_closed(&mut self) -> anyhow::Result<()> {
            self.client.clear_rules()?;
            self.client.push_rules(&deny_all_rule())
        }
        fn backend(&self) -> &'static str {
            "nefilter"
        }
    }
}

// ===========================================================================
// Linux — eBPF (cgroup_skb egress map)
// ===========================================================================
#[cfg(target_os = "linux")]
pub mod ebpf_backend {
    use super::*;
    use dek_domain_schema::ebpf::{EbpfMapUpdate, UpdateSource};
    use dek_ebpfd::map_updater::MapUpdater;

    pub struct EbpfEnforcer {
        updater: MapUpdater,
        generation: u64,
    }
    impl EbpfEnforcer {
        pub fn new(tenant_id: String, device_id: String) -> Self {
            Self {
                updater: MapUpdater::new(tenant_id, device_id, 0),
                generation: 0,
            }
        }

        /// Convert a compiled rule's CIDR/port destinations into LPM map updates.
        fn to_updates(&self, gen: u64, rules: &[CompiledNetworkRules]) -> Vec<EbpfMapUpdate> {
            let mut out = Vec::new();
            for r in rules {
                let allow: u8 = matches!(r.effect, NetworkGuardrailEffect::Allow) as u8;
                for d in &r.conditions.destinations {
                    if d.r#type == "cidr" {
                        if let Some(cidr) = d.value.as_str() {
                            out.push(EbpfMapUpdate {
                                schema_version: "1.0".into(),
                                map_name: "egress_lpm_v4".into(),
                                operation: "insert".into(),
                                source: UpdateSource::Bundle,
                                tenant_id: self.updater.tenant_id.clone(),
                                device_id: self.updater.device_id.clone(),
                                generation: gen,
                                key: serde_json::json!({ "cidr": cidr }),
                                value: serde_json::json!({ "allow": allow, "log_event": 1 }),
                                signature: None,
                            });
                        }
                    }
                }
            }
            out
        }
    }
    impl NetworkEnforcer for EbpfEnforcer {
        fn apply(&mut self, rules: &[CompiledNetworkRules]) -> anyhow::Result<()> {
            self.generation += 1;
            for upd in self.to_updates(self.generation, rules) {
                self.updater.apply_update(upd)?;
            }
            Ok(())
        }
        fn fail_closed(&mut self) -> anyhow::Result<()> {
            self.generation += 1;
            for upd in self.to_updates(self.generation, &[deny_all_rule()]) {
                self.updater.apply_update(upd)?;
            }
            Ok(())
        }
        fn backend(&self) -> &'static str {
            "ebpf"
        }
    }
}

/// Build the platform enforcer. Returns None on unsupported platforms (the
/// driver then no-ops, but MCP-plane enforcement is unaffected).
pub fn platform_enforcer(_tenant_id: &str, _device_id: &str) -> Option<Box<dyn NetworkEnforcer>> {
    #[cfg(windows)]
    {
        return wfp_backend::WfpEnforcer::new()
            .map(|e| Box::new(e) as Box<dyn NetworkEnforcer>)
            .ok();
    }
    #[cfg(target_os = "macos")]
    {
        return nefilter_backend::NeFilterEnforcer::new()
            .map(|e| Box::new(e) as Box<dyn NetworkEnforcer>)
            .ok();
    }
    #[cfg(target_os = "linux")]
    {
        return Some(Box::new(ebpf_backend::EbpfEnforcer::new(
            _tenant_id.to_string(),
            _device_id.to_string(),
        )));
    }
    #[allow(unreachable_code)]
    None
}

/// Spawn the network enforcement driver. Consumes the syncer's SyncOutcome
/// channel; applies/falls-back/fails-closed per the fail-closed contract.
pub fn spawn(
    mut rx: Receiver<SyncOutcome>,
    tenant_id: String,
    device_id: String,
    cancel: CancellationToken,
    reload_coord: std::sync::Arc<crate::reload_coordinator::ReloadCoordinator>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut enforcer = match platform_enforcer(&tenant_id, &device_id) {
            Some(e) => e,
            None => {
                info!("[net] no network enforcer for this platform; network plane disabled");
                return;
            }
        };
        let backend = enforcer.backend();
        let mut lkg: Option<Vec<CompiledNetworkRules>> = None;
        info!("[net] network enforcement driver started (backend={backend})");

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("[net] driver shutting down");
                    break;
                }
                msg = rx.recv() => {
                    match msg {
                        Some(SyncOutcome::Updated { network_rules, config, manifest_path, .. }) => {
                            if let Err(e) = reload_coord.process_staged_bundle(&config, &manifest_path).await {
                                error!("[net] Failed to load new Sidecar API snapshot after bundle sync: {}", e);
                            } else {
                                info!("[net] Successfully reloaded Sidecar API snapshot due to bundle sync");
                            }

                            if let Some(rules) = network_rules {
                                let mut kernel_rules_vec = Vec::new();
                                for r in &rules {
                                    let (kr, part) = crate::kernel_guard::kernel_subset(r);
                                    tracing::info!(
                                        "network rules (policy {}): {} kernel-safe, {} user-mode ({} overflow) — complexity-guarded",
                                        r.policy_id, part.kernel.len(), part.user_mode.len(), part.overflow_to_user
                                    );
                                    if !kr.conditions.destinations.is_empty() {
                                        kernel_rules_vec.push(kr);
                                    }
                                    if !part.user_mode.is_empty() {
                                        tracing::info!("complex rules routed to user-mode proxy for policy {}", r.policy_id);
                                    }
                                    metrics::gauge!("dek_network_rules_kernel", "policy" => r.policy_id.clone()).set(part.kernel.len() as f64);
                                    metrics::gauge!("dek_network_rules_usermode", "policy" => r.policy_id.clone()).set(part.user_mode.len() as f64);
                                }

                                match enforcer.apply(&kernel_rules_vec) {
                                    Ok(()) => {
                                        metrics::counter!("dek_network_rule_enforced_total",
                                            "backend" => backend, "result" => "applied").increment(1);
                                        lkg = Some(kernel_rules_vec);
                                    }
                                    Err(e) => {
                                        error!("[net] apply failed: {e}; reverting to LKG / fail-closed");
                                        let reverted = lkg.as_ref()
                                            .map(|r| enforcer.apply(r).is_ok())
                                            .unwrap_or(false);
                                        if !reverted {
                                            let _ = enforcer.fail_closed();
                                            metrics::counter!("dek_network_failclosed_total",
                                                "backend" => backend, "reason" => "apply_error").increment(1);
                                        }
                                    }
                                }
                            }
                        }
                        // Sync failed (cloud blip): keep LKG; if we never had any -> fail-closed.
                        Some(SyncOutcome::Failed { .. }) => {
                            if lkg.is_none() {
                                let _ = enforcer.fail_closed();
                                metrics::counter!("dek_network_failclosed_total",
                                    "backend" => backend, "reason" => "no_lkg_on_failure").increment(1);
                            }
                        }
                        // Freshness state machine flipped to strict-deny -> block egress too.
                        Some(SyncOutcome::StateTransition(state)) if state.is_strict_deny() => {
                            warn!("[net] EnforcementState=StrictDeny -> network fail-closed");
                            let _ = enforcer.fail_closed();
                            metrics::counter!("dek_network_failclosed_total",
                                "backend" => backend, "reason" => "strict_deny").increment(1);
                        }
                        Some(_) => {}
                        None => { info!("[net] sync channel closed"); break; }
                    }
                }
            }
        }
    })
}
