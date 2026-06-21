use crate::model::AgentObservationEvent;

pub fn extract_spans(event: &AgentObservationEvent) {
    if let Some(tokens) = &event.token_usage {
        let _span_name = "gen_ai.usage";
        let _model = tokens.model.clone().unwrap_or_default();
        let _total_tokens = tokens.total_tokens.unwrap_or(0);

        // Emitting an OTel span
        // Tracer::span_builder(_span_name).with_attributes(...).start();
    }
}
