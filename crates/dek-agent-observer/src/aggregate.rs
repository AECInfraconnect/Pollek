use crate::model::AgentObservationEvent;
use std::collections::HashMap;

pub struct AgentStats {
    pub total_events: u64,
    pub resources_accessed: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

pub fn aggregate_events(events: &[AgentObservationEvent]) -> HashMap<String, AgentStats> {
    let mut stats = HashMap::new();

    for event in events {
        let agent_id = event
            .agent_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        let entry = stats.entry(agent_id).or_insert(AgentStats {
            total_events: 0,
            resources_accessed: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
        });

        entry.total_events += 1;

        if event.resource_id.is_some() {
            entry.resources_accessed += 1;
        }

        if let Some(tokens) = &event.token_usage {
            entry.total_input_tokens += tokens.input_tokens.unwrap_or(0) as u64;
            entry.total_output_tokens += tokens.output_tokens.unwrap_or(0) as u64;
        }
    }

    stats
}
