//! Agent correlation join — attribute a raw runtime signal (pid / cgroup /
//! executable hash / process name / flow) to a **discovered agent_id**.
//!
//! This is Phase 1 of the AI-agent observe/enforce roadmap: kernel-grade sensors
//! (eBPF ring buffer, Windows ETW, macOS EndpointSecurity) see *flows and
//! processes*, but the agent identity lives in user space. Without this join,
//! enforcement can only be device-wide, never agent-scoped. The correlator turns
//! a [`ProcessSignal`] into an agent attribution so an [`AgentObservationEvent`]
//! can be stamped before it reaches the telemetry spool.
//!
//! Pure user-mode logic — deterministic and unit-testable in CI (Tier 1); it
//! does not touch the kernel.

use crate::model::{AgentObservationEvent, ProcessSignal};
use std::collections::HashMap;

/// A discovered/registered agent's process identity, used to build the index.
/// The Local Control Plane assembles these from the registry (registered agents
/// together with their `ProcessEvidence` / `SuggestedAgentRegistration`); the
/// correlator stays dependency-light and does not import the discovery crate.
#[derive(Debug, Clone, Default)]
pub struct AgentProcessBinding {
    pub agent_id: String,
    /// Live pids currently attributed to this agent (may recycle — always
    /// cross-checked against `exe_path_hash` when both are present).
    pub pids: Vec<u32>,
    /// sha256 of the normalized executable path — the stable identity key.
    pub exe_path_hash: Option<String>,
    /// Process names this agent runs under (weak signal; may be ambiguous).
    pub process_names: Vec<String>,
    /// cgroup ids currently attributed to this agent.
    pub cgroup_ids: Vec<u64>,
}

/// Which key produced the match, most-to-least reliable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchBasis {
    /// pid and executable hash both agree — strongest.
    PidAndExe,
    /// stable executable-hash identity.
    ExeHash,
    /// cgroup / scope id.
    Cgroup,
    /// live pid only (no hash to cross-check — pid recycling risk).
    Pid,
    /// process name, and only when exactly one agent claims it.
    ProcessNameUnique,
}

impl MatchBasis {
    /// Confidence 0–100 for the attribution.
    pub fn confidence(self) -> u8 {
        match self {
            MatchBasis::PidAndExe => 100,
            MatchBasis::ExeHash => 90,
            MatchBasis::Cgroup => 85,
            MatchBasis::Pid => 70,
            MatchBasis::ProcessNameUnique => 50,
        }
    }
}

/// A resolved attribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentResolution {
    pub agent_id: String,
    pub basis: MatchBasis,
    pub confidence: u8,
}

/// Index over agent process identities for fast signal → agent_id resolution.
#[derive(Debug, Default)]
pub struct AgentCorrelator {
    by_pid: HashMap<u32, String>,
    by_cgroup: HashMap<u64, String>,
    by_exe_hash: HashMap<String, String>,
    /// name → set of agent_ids; used only when exactly one agent claims a name.
    by_name: HashMap<String, Vec<String>>,
}

impl AgentCorrelator {
    /// Build the index from the current agent process bindings.
    pub fn from_bindings(bindings: &[AgentProcessBinding]) -> Self {
        let mut c = AgentCorrelator::default();
        for b in bindings {
            if b.agent_id.is_empty() {
                continue;
            }
            for &pid in &b.pids {
                c.by_pid.insert(pid, b.agent_id.clone());
            }
            for &cg in &b.cgroup_ids {
                c.by_cgroup.insert(cg, b.agent_id.clone());
            }
            if let Some(h) = b.exe_path_hash.as_ref().filter(|h| !h.is_empty()) {
                c.by_exe_hash.insert(h.clone(), b.agent_id.clone());
            }
            for name in &b.process_names {
                let key = name.trim().to_ascii_lowercase();
                if key.is_empty() {
                    continue;
                }
                let entry = c.by_name.entry(key).or_default();
                if !entry.contains(&b.agent_id) {
                    entry.push(b.agent_id.clone());
                }
            }
        }
        c
    }

    /// Number of indexed agents' pids (test/inspection helper).
    pub fn pid_count(&self) -> usize {
        self.by_pid.len()
    }

    /// Resolve a raw signal to an agent, honoring reliability precedence:
    /// pid+exe agreement → exe hash → cgroup → pid → unique process name.
    pub fn resolve(&self, sig: &ProcessSignal) -> Option<AgentResolution> {
        let by_exe = sig
            .exe_path_hash
            .as_ref()
            .filter(|h| !h.is_empty())
            .and_then(|h| self.by_exe_hash.get(h));
        let by_pid = sig.pid.and_then(|p| self.by_pid.get(&p));

        // Strongest: pid and exe hash present and agree.
        if let (Some(exe_agent), Some(pid_agent)) = (by_exe, by_pid) {
            if exe_agent == pid_agent {
                return Some(res(exe_agent, MatchBasis::PidAndExe));
            }
            // Disagreement (pid recycled onto a different binary): trust the
            // stable executable identity over the recycled pid.
            return Some(res(exe_agent, MatchBasis::ExeHash));
        }
        // Stable executable identity.
        if let Some(agent) = by_exe {
            return Some(res(agent, MatchBasis::ExeHash));
        }
        // cgroup / scope.
        if let Some(cg) = sig.cgroup_id {
            if let Some(agent) = self.by_cgroup.get(&cg) {
                return Some(res(agent, MatchBasis::Cgroup));
            }
        }
        // Live pid alone (no hash to cross-check).
        if let Some(agent) = by_pid {
            return Some(res(agent, MatchBasis::Pid));
        }
        // Weakest: process name, only when unambiguous.
        if let Some(name) = sig.process_name.as_ref() {
            let key = name.trim().to_ascii_lowercase();
            if let Some(agents) = self.by_name.get(&key) {
                if agents.len() == 1 {
                    return Some(res(&agents[0], MatchBasis::ProcessNameUnique));
                }
            }
        }
        None
    }

    /// Stamp `agent_id` onto an agent-less event using its `process_signal`.
    /// Returns the attribution when one was applied. Never overwrites an
    /// already-attributed event.
    pub fn enrich_event(&self, event: &mut AgentObservationEvent) -> Option<AgentResolution> {
        if event.agent_id.is_some() {
            return None;
        }
        let sig = event.process_signal.as_ref()?;
        let resolution = self.resolve(sig)?;
        event.agent_id = Some(resolution.agent_id.clone());
        Some(resolution)
    }
}

fn res(agent_id: &str, basis: MatchBasis) -> AgentResolution {
    AgentResolution {
        agent_id: agent_id.to_string(),
        basis,
        confidence: basis.confidence(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(agent: &str) -> AgentProcessBinding {
        AgentProcessBinding {
            agent_id: agent.to_string(),
            ..Default::default()
        }
    }

    fn sig() -> ProcessSignal {
        ProcessSignal::default()
    }

    #[test]
    fn pid_and_exe_agreement_is_highest_confidence() {
        let c = AgentCorrelator::from_bindings(&[AgentProcessBinding {
            pids: vec![4242],
            exe_path_hash: Some("hashA".into()),
            ..binding("agent_claude_code")
        }]);
        assert_eq!(
            c.resolve(&ProcessSignal {
                pid: Some(4242),
                exe_path_hash: Some("hashA".into()),
                ..sig()
            }),
            Some(AgentResolution {
                agent_id: "agent_claude_code".into(),
                basis: MatchBasis::PidAndExe,
                confidence: 100,
            })
        );
    }

    #[test]
    fn exe_hash_wins_over_recycled_pid() {
        // pid 4242 now belongs to agent B by the live index, but the signal's
        // executable hash identifies agent A → trust the stable identity.
        let c = AgentCorrelator::from_bindings(&[
            AgentProcessBinding {
                exe_path_hash: Some("hashA".into()),
                ..binding("agent_a")
            },
            AgentProcessBinding {
                pids: vec![4242],
                ..binding("agent_b")
            },
        ]);
        assert_eq!(
            c.resolve(&ProcessSignal {
                pid: Some(4242),
                exe_path_hash: Some("hashA".into()),
                ..sig()
            }),
            Some(AgentResolution {
                agent_id: "agent_a".into(),
                basis: MatchBasis::ExeHash,
                confidence: 90,
            })
        );
    }

    #[test]
    fn cgroup_then_pid_then_name_precedence() {
        let c = AgentCorrelator::from_bindings(&[AgentProcessBinding {
            pids: vec![10],
            cgroup_ids: vec![999],
            process_names: vec!["claude".into()],
            ..binding("agent_x")
        }]);
        // cgroup beats pid-only
        assert_eq!(
            c.resolve(&ProcessSignal {
                pid: Some(10),
                cgroup_id: Some(999),
                ..sig()
            })
            .map(|r| r.basis),
            Some(MatchBasis::Cgroup)
        );
        // pid-only when no stronger key
        assert_eq!(
            c.resolve(&ProcessSignal {
                pid: Some(10),
                ..sig()
            })
            .map(|r| r.basis),
            Some(MatchBasis::Pid)
        );
        // name-only, unambiguous
        assert_eq!(
            c.resolve(&ProcessSignal {
                process_name: Some("Claude".into()),
                ..sig()
            })
            .map(|r| r.basis),
            Some(MatchBasis::ProcessNameUnique)
        );
    }

    #[test]
    fn ambiguous_process_name_does_not_resolve() {
        let c = AgentCorrelator::from_bindings(&[
            AgentProcessBinding {
                process_names: vec!["node".into()],
                ..binding("agent_a")
            },
            AgentProcessBinding {
                process_names: vec!["node".into()],
                ..binding("agent_b")
            },
        ]);
        assert!(c
            .resolve(&ProcessSignal {
                process_name: Some("node".into()),
                ..sig()
            })
            .is_none());
    }

    #[test]
    fn miss_returns_none() {
        let c = AgentCorrelator::from_bindings(&[AgentProcessBinding {
            pids: vec![1],
            ..binding("agent_a")
        }]);
        assert!(c
            .resolve(&ProcessSignal {
                pid: Some(2),
                process_name: Some("unknown".into()),
                ..sig()
            })
            .is_none());
    }

    #[test]
    fn enrich_stamps_only_agentless_events() {
        let c = AgentCorrelator::from_bindings(&[AgentProcessBinding {
            exe_path_hash: Some("h".into()),
            ..binding("agent_a")
        }]);
        let mut ev = AgentObservationEvent {
            event_id: "e1".into(),
            tenant_id: "local".into(),
            trace_id: "t1".into(),
            agent_id: None,
            shadow_candidate_id: None,
            tool_id: None,
            resource_id: None,
            surface: "network".into(),
            action: "connect".into(),
            pep_type: None,
            risk_level: None,
            timestamp: "2026-07-23T00:00:00Z".into(),
            payload_json: "{}".into(),
            token_usage: None,
            browser_scope: None,
            event_kind: Default::default(),
            decision: None,
            tool_call: None,
            resource_access: None,
            latency_ms: None,
            provider: None,
            process_signal: Some(ProcessSignal {
                exe_path_hash: Some("h".into()),
                ..ProcessSignal::default()
            }),
        };
        assert_eq!(
            c.enrich_event(&mut ev).map(|r| r.agent_id),
            Some("agent_a".to_string())
        );
        assert_eq!(ev.agent_id.as_deref(), Some("agent_a"));

        // Already attributed → never overwritten.
        ev.agent_id = Some("agent_manual".into());
        assert!(c.enrich_event(&mut ev).is_none());
        assert_eq!(ev.agent_id.as_deref(), Some("agent_manual"));
    }
}
