use crate::agent_correlator::{AgentCorrelator, AgentResolution};
use crate::model::AgentObservationEvent;

pub fn correlate_shadow_candidate(event: &mut AgentObservationEvent) {
    if event.agent_id.is_none() && event.shadow_candidate_id.is_none() {
        // Simple heuristic for shadow candidates
        // e.g. a process making lots of network requests not registered
        event.shadow_candidate_id = Some(format!("shadow-candidate-{}", event.trace_id));
    }
}

/// Full correlation for an agent-less event: first try to attribute it to a
/// discovered agent via its `process_signal` (the real join), and only fall
/// back to a shadow candidate when no known agent matches. Returns the agent
/// attribution when one was applied.
pub fn correlate_event(
    event: &mut AgentObservationEvent,
    correlator: &AgentCorrelator,
) -> Option<AgentResolution> {
    let resolution = correlator.enrich_event(event);
    if resolution.is_none() {
        correlate_shadow_candidate(event);
    }
    resolution
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::agent_correlator::AgentProcessBinding;
    use crate::model::ProcessSignal;

    fn agentless_event(signal: Option<ProcessSignal>) -> AgentObservationEvent {
        AgentObservationEvent {
            event_id: "e".into(),
            tenant_id: "local".into(),
            trace_id: "trace-1".into(),
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
            process_signal: signal,
        }
    }

    #[test]
    fn known_agent_is_attributed_not_shadowed() {
        let correlator = AgentCorrelator::from_bindings(&[AgentProcessBinding {
            agent_id: "agent_a".into(),
            exe_path_hash: Some("h".into()),
            ..Default::default()
        }]);
        let mut ev = agentless_event(Some(ProcessSignal {
            exe_path_hash: Some("h".into()),
            ..ProcessSignal::default()
        }));
        let r = correlate_event(&mut ev, &correlator);
        assert_eq!(r.unwrap().agent_id, "agent_a");
        assert_eq!(ev.agent_id.as_deref(), Some("agent_a"));
        assert!(ev.shadow_candidate_id.is_none());
    }

    #[test]
    fn unknown_process_falls_back_to_shadow() {
        let correlator = AgentCorrelator::from_bindings(&[]);
        let mut ev = agentless_event(Some(ProcessSignal {
            exe_path_hash: Some("unknown".into()),
            ..ProcessSignal::default()
        }));
        let r = correlate_event(&mut ev, &correlator);
        assert!(r.is_none());
        assert!(ev.agent_id.is_none());
        assert_eq!(
            ev.shadow_candidate_id.as_deref(),
            Some("shadow-candidate-trace-1")
        );
    }
}
