// SPDX-License-Identifier: Apache-2.0
//! engine_selector.rs — auto-select the right policy engine when a route does
//! not pin one explicitly.
//!
//! Each engine fits a different decision shape:
//!   - Cedar    : attribute/role authorization (ABAC/RBAC)
//!   - OpenFGA  : relationship / graph authorization (ReBAC, Zanzibar)
//!   - OPA Rego : complex rule logic / data transforms
//!   - eBPF/WFP : network egress (CIDR/port/domain) — kernel, simple match only
//!
//! The selector only ever returns an engine that is actually registered in this
//! build (feature-gated adapters), and falls back conservatively. It never
//! invents an engine, and the router stays fail-closed if none is available.

/// The kind of decision a request needs — inferred from the request shape when
/// the matched route doesn't specify `pdp_required`/`pdp_pool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionKind {
    /// Attribute/role checks → Cedar.
    Authorization,
    /// Relationship/graph checks (user↔resource) → OpenFGA.
    Relationship,
    /// Rule-heavy logic / data transforms → OPA Rego (Wasm).
    ComplexLogic,
    /// Network egress (CIDR/domain/port) → kernel plane (eBPF/WFP).
    NetworkEgress,
}

pub struct EngineSelector;

impl EngineSelector {
    /// Infer the decision kind from method + payload. Heuristic, deterministic,
    /// and side-effect free so it's safe on the hot path.
    pub fn infer_kind(method: &str, payload: &serde_json::Value) -> DecisionKind {
        // network intent: explicit method prefix or a destination in the payload
        if method.starts_with("net.")
            || method.starts_with("network.")
            || payload.get("destination").is_some()
            || payload.get("egress").is_some()
        {
            return DecisionKind::NetworkEgress;
        }
        // relationship intent: zanzibar-style tuples
        if payload.get("relation").is_some()
            || (payload.get("object").is_some() && payload.get("subject").is_some())
        {
            return DecisionKind::Relationship;
        }
        // complex-logic intent: explicit rego query or a flagged complex request
        if payload.get("rego_query").is_some()
            || payload.get("complex").and_then(|v| v.as_bool()) == Some(true)
        {
            return DecisionKind::ComplexLogic;
        }
        DecisionKind::Authorization
    }

    /// Preference order per decision kind. Engine ids match adapter ids
    /// (`cedar`, `openfga`, `opa_wasm`, `ebpf`).
    fn preference(kind: DecisionKind) -> &'static [&'static str] {
        match kind {
            DecisionKind::Authorization => &["cedar", "opa_wasm", "openfga"],
            DecisionKind::Relationship => &["openfga", "cedar", "opa_wasm"],
            DecisionKind::ComplexLogic => &["opa_wasm", "cedar", "openfga"],
            DecisionKind::NetworkEgress => &["ebpf", "cedar", "opa_wasm"],
        }
    }

    /// Choose the best available engine for `kind`. Returns the first preferred
    /// engine that is registered (`available`). `None` → caller stays
    /// fail-closed (deny: no engine to evaluate).
    pub fn select(kind: DecisionKind, available: &[String]) -> Option<String> {
        Self::preference(kind)
            .iter()
            .find(|e| available.iter().any(|a| a == *e))
            .map(|s| s.to_string())
    }

    /// Convenience: infer + select in one call.
    pub fn resolve(method: &str, payload: &serde_json::Value, available: &[String]) -> Option<String> {
        Self::select(Self::infer_kind(method, payload), available)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn avail(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn infer_network() {
        assert_eq!(
            EngineSelector::infer_kind("net.connect", &json!({})),
            DecisionKind::NetworkEgress
        );
        assert_eq!(
            EngineSelector::infer_kind("tools/call", &json!({"destination": "1.2.3.4"})),
            DecisionKind::NetworkEgress
        );
    }

    #[test]
    fn infer_relationship() {
        assert_eq!(
            EngineSelector::infer_kind("check", &json!({"relation": "viewer", "object": "doc:1", "subject": "user:a"})),
            DecisionKind::Relationship
        );
    }

    #[test]
    fn infer_complex_and_default() {
        assert_eq!(
            EngineSelector::infer_kind("eval", &json!({"rego_query": "data.x"})),
            DecisionKind::ComplexLogic
        );
        assert_eq!(
            EngineSelector::infer_kind("tools/call", &json!({"principal": "a"})),
            DecisionKind::Authorization
        );
    }

    #[test]
    fn select_respects_availability() {
        // prefers cedar for authz, but if only opa is built, pick opa
        assert_eq!(
            EngineSelector::select(DecisionKind::Authorization, &avail(&["opa_wasm", "openfga"])),
            Some("opa_wasm".to_string())
        );
        // relationship prefers openfga
        assert_eq!(
            EngineSelector::select(DecisionKind::Relationship, &avail(&["cedar", "openfga"])),
            Some("openfga".to_string())
        );
        // nothing available -> None (caller fail-closed)
        assert_eq!(EngineSelector::select(DecisionKind::Authorization, &avail(&[])), None);
    }

    #[test]
    fn network_prefers_ebpf() {
        assert_eq!(
            EngineSelector::select(DecisionKind::NetworkEgress, &avail(&["ebpf", "cedar"])),
            Some("ebpf".to_string())
        );
    }
}
