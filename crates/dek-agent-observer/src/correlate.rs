use crate::model::AgentObservationEvent;

pub fn correlate_shadow_candidate(event: &mut AgentObservationEvent) {
    if event.agent_id.is_none() {
        // Simple heuristic for shadow candidates
        // e.g. a process making lots of network requests not registered
        event.shadow_candidate_id = Some(format!("shadow-candidate-{}", event.trace_id));
    }
}
