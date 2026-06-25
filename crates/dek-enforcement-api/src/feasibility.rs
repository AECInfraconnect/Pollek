use crate::planner::{
    ControlDomain, ControlLevel, DomainFeasibility, LocalCapabilitySnapshot, MethodStatus, Policy,
    PolicyFeasibilityResult,
};
use dek_agent_discovery::model::{ControlBindingKind, DiscoveredAgentCandidateV2};

pub fn assess(
    candidate: &DiscoveredAgentCandidateV2,
    want: ControlLevel,
    snap: &LocalCapabilitySnapshot,
) -> PolicyFeasibilityResult {
    let mut achievable = want.clone();
    let mut actions = vec![];
    let mut per_domain = vec![];

    for binding in &candidate.suggested_control_bindings {
        let domain = match binding.kind {
            ControlBindingKind::McpStdioWrapper
            | ControlBindingKind::McpHttpProxy
            | ControlBindingKind::OpenAiCompatibleProxy
            | ControlBindingKind::AnthropicCompatibleProxy
            | ControlBindingKind::OllamaProxy => Some(ControlDomain::McpTool),
            ControlBindingKind::NetworkEgressPep => Some(ControlDomain::Network),
            ControlBindingKind::FilePep => Some(ControlDomain::FileSystem),
            ControlBindingKind::ObserveOnly => None,
        };

        if let Some(domain) = domain {
            // Find best method for this domain
            let pref: &[&str] = match domain {
                ControlDomain::Network => {
                    &["linux_ebpf", "macos_netext", "windows_wfp_um", "mcp_http"]
                }
                ControlDomain::FileSystem => &["linux_landlock", "macos_es", "mcp_stdio"],
                ControlDomain::McpTool => &["mcp_stdio", "mcp_http"],
                ControlDomain::Process => &["linux_ebpf", "macos_es", "windows_etw"],
                ControlDomain::Dns => &["macos_netext", "linux_ebpf", "mcp_http"],
            };

            let method = pref
                .iter()
                .find_map(|id| {
                    snap.control_methods.iter().find(|m| {
                        &m.id == id
                            && m.status == MethodStatus::Available
                            && m.domains.contains(&domain)
                            && m.max_level >= want.min_observe()
                    })
                })
                .or_else(|| snap.observe_capable(domain.clone()));

            match method {
                Some(m) => {
                    let lvl = m.max_level.clone().min(want.clone());
                    achievable = achievable.min(lvl.clone());
                    per_domain.push(DomainFeasibility::ok(domain.clone(), m, lvl));
                }
                None => {
                    achievable = ControlLevel::Observe.min(achievable);
                    if let Some(u) = snap.upgrade_for(domain.clone()) {
                        actions.push(u.clone());
                    }
                    per_domain.push(DomainFeasibility::observe_fallback(domain));
                }
            }
        }
    }

    let policy = Policy {
        id: "derived_from_candidate".into(),
        requested_level: want.clone(),
    };

    PolicyFeasibilityResult::build(&policy, achievable, per_domain, actions)
}
