// SPDX-License-Identifier: Apache-2.0
//! kernel_guard.rs — keep complex policy OUT of the kernel data path.
//!
//! The kernel network plane (eBPF on Linux, WFP callout on Windows) is fast and
//! unbypassable, but the eBPF verifier bounds program/map size and only handles
//! simple, exact matching. Pushing regex/wildcard/conditional/time-window rules
//! — or too many entries — risks verifier rejection or runtime instability
//! (worst case: load failure / instability). This guard classifies each
//! destination and routes only KERNEL-SAFE matches to the kernel; everything
//! else falls to the user-mode plane (proxy/PDP), which is expressive but
//! slower. Net effect: the kernel never sees a rule it can't safely enforce, so
//! we avoid crashes/overload while preserving full policy coverage.

use dek_domain_schema::network_guardrail::{
    CompiledNetworkRules, NetworkConditions, NetworkDestination,
};

/// Max kernel map entries we will install. eBPF maps are fixed-capacity; staying
/// well under the limit avoids verifier/map-full failures. Overflow → user-mode.
pub const MAX_KERNEL_ENTRIES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleComplexity {
    /// Exact CIDR / port / exact-domain — safe to enforce in kernel.
    KernelSafe,
    /// Regex/wildcard/conditional/time-window — must run in user mode.
    UserModeOnly,
}

/// Classify a single destination for kernel-safety.
pub fn classify_destination(dest: &NetworkDestination, has_conditions: bool) -> RuleComplexity {
    if has_conditions {
        return RuleComplexity::UserModeOnly; // time_window / protocol predicates
    }
    match dest.r#type.as_str() {
        "cidr" | "port" => RuleComplexity::KernelSafe,
        "domain" => {
            // exact domain is kernel-safe; wildcard/regex domains are not.
            let v = dest.value.as_str().unwrap_or("");
            if v.contains('*') || v.contains('?') || v.starts_with('/') || v.is_empty() {
                RuleComplexity::UserModeOnly
            } else {
                RuleComplexity::KernelSafe
            }
        }
        // unknown / regex / wildcard types → user mode
        _ => RuleComplexity::UserModeOnly,
    }
}

/// Decide whether time/protocol conditions make a ruleset user-mode-only.
fn conditions_force_usermode(c: &NetworkConditions) -> bool {
    c.time_window.is_some() || !c.protocols.is_empty()
}

/// Result of partitioning: which destinations go to which plane.
#[derive(Debug, Default)]
pub struct PartitionedRules {
    pub kernel: Vec<NetworkDestination>,
    pub user_mode: Vec<NetworkDestination>,
    /// destinations dropped from kernel purely due to the entry cap (still
    /// enforced in user mode — never silently dropped).
    pub overflow_to_user: usize,
}

/// Split a compiled ruleset's destinations into kernel-safe vs user-mode.
/// Conditional rulesets (time/protocol) go entirely to user mode.
pub fn partition_rules(rules: &CompiledNetworkRules) -> PartitionedRules {
    let mut out = PartitionedRules::default();
    let force_user = conditions_force_usermode(&rules.conditions);

    for dest in &rules.conditions.destinations {
        let class = if force_user {
            RuleComplexity::UserModeOnly
        } else {
            classify_destination(dest, false)
        };
        match class {
            RuleComplexity::KernelSafe => {
                if out.kernel.len() < MAX_KERNEL_ENTRIES {
                    out.kernel.push(dest.clone());
                } else {
                    out.user_mode.push(dest.clone()); // cap overflow → user mode
                    out.overflow_to_user += 1;
                }
            }
            RuleComplexity::UserModeOnly => out.user_mode.push(dest.clone()),
        }
    }
    out
}

/// Build a kernel-only ruleset (clone of `rules` but with only kernel-safe
/// destinations) for handing to the eBPF/WFP backend.
pub fn kernel_subset(rules: &CompiledNetworkRules) -> (CompiledNetworkRules, PartitionedRules) {
    let part = partition_rules(rules);
    let mut kernel_rules = rules.clone();
    kernel_rules.conditions = NetworkConditions {
        destinations: part.kernel.clone(),
        protocols: vec![],     // conditions already forced user-mode if present
        time_window: None,
    };
    (kernel_rules, part)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dek_domain_schema::network_guardrail::*;
    use serde_json::json;

    fn dest(t: &str, v: serde_json::Value) -> NetworkDestination {
        NetworkDestination { r#type: t.into(), value: v }
    }
    fn rules_with(dests: Vec<NetworkDestination>, time_window: Option<String>) -> CompiledNetworkRules {
        CompiledNetworkRules {
            policy_id: "p".into(), policy_type: "NETWORK_EGRESS_GUARDRAIL".into(), version: 1,
            risk_tier: "low".into(), targets: NetworkTargets::default(),
            conditions: NetworkConditions { destinations: dests, protocols: vec![], time_window },
            effect: NetworkGuardrailEffect::Deny, obligations: vec![],
            fallback: NetworkFallback { cloud_unavailable: "deny".into(), policy_stale: "deny".into() },
        }
    }

    #[test]
    fn cidr_and_port_are_kernel_safe() {
        assert_eq!(classify_destination(&dest("cidr", json!("10.0.0.0/8")), false), RuleComplexity::KernelSafe);
        assert_eq!(classify_destination(&dest("port", json!(443)), false), RuleComplexity::KernelSafe);
    }

    #[test]
    fn wildcard_domain_is_user_mode() {
        assert_eq!(classify_destination(&dest("domain", json!("*.evil.com")), false), RuleComplexity::UserModeOnly);
        assert_eq!(classify_destination(&dest("domain", json!("api.exact.com")), false), RuleComplexity::KernelSafe);
    }

    #[test]
    fn conditions_force_user_mode() {
        // time_window present -> everything user-mode even if dests are simple
        let r = rules_with(vec![dest("cidr", json!("1.0.0.0/8"))], Some("09:00-17:00".into()));
        let p = partition_rules(&r);
        assert_eq!(p.kernel.len(), 0);
        assert_eq!(p.user_mode.len(), 1);
    }

    #[test]
    fn partition_splits_correctly() {
        let r = rules_with(vec![
            dest("cidr", json!("10.0.0.0/8")),       // kernel
            dest("domain", json!("*.bad.com")),       // user
            dest("port", json!(8080)),                // kernel
            dest("regex", json!(".*")),               // user
        ], None);
        let p = partition_rules(&r);
        assert_eq!(p.kernel.len(), 2);
        assert_eq!(p.user_mode.len(), 2);
    }

    #[test]
    fn cap_overflow_goes_user_mode() {
        let many: Vec<_> = (0..(MAX_KERNEL_ENTRIES + 5))
            .map(|i| dest("port", json!(i as u32))).collect();
        let r = rules_with(many, None);
        let p = partition_rules(&r);
        assert_eq!(p.kernel.len(), MAX_KERNEL_ENTRIES);
        assert_eq!(p.overflow_to_user, 5);
        assert_eq!(p.user_mode.len(), 5);
    }

    #[test]
    fn kernel_subset_has_only_safe_dests() {
        let r = rules_with(vec![
            dest("cidr", json!("10.0.0.0/8")),
            dest("domain", json!("*.bad.com")),
        ], None);
        let (ks, _) = kernel_subset(&r);
        assert_eq!(ks.conditions.destinations.len(), 1);
        assert_eq!(ks.conditions.destinations[0].r#type, "cidr");
    }
}
