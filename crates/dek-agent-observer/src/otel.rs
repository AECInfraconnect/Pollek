use crate::model::AgentObservationEvent;
use crate::usage_model::AiUsageEventV1;
use opentelemetry::{
    global,
    trace::{SpanKind, Tracer},
    KeyValue,
};

pub fn emit_span(event: &AgentObservationEvent) {
    let tracer = global::tracer("dek-agent-observer");
    let mut attrs = vec![
        KeyValue::new("gen_ai.operation.name", event.action.clone()),
        KeyValue::new(
            "pollen.agent_id",
            event.agent_id.clone().unwrap_or_default(),
        ),
        KeyValue::new("pollen.tenant_id", event.tenant_id.clone()),
    ];
    if let Some(p) = &event.provider {
        attrs.push(KeyValue::new("gen_ai.system", p.clone()));
    }
    if let Some(u) = &event.token_usage {
        if let Some(m) = &u.model {
            attrs.push(KeyValue::new("gen_ai.request.model", m.clone()));
        }
        attrs.push(KeyValue::new(
            "gen_ai.usage.input_tokens",
            u.input_tokens.unwrap_or(0),
        ));
        attrs.push(KeyValue::new(
            "gen_ai.usage.output_tokens",
            u.output_tokens.unwrap_or(0),
        ));
    }
    if let Some(t) = &event.tool_call {
        attrs.push(KeyValue::new("gen_ai.tool.name", t.tool_name.clone()));
    }
    tracer
        .span_builder(event.action.clone())
        .with_kind(SpanKind::Client)
        .with_attributes(attrs)
        .start(&tracer);
}

pub fn emit_usage_span(event: &AiUsageEventV1) {
    let tracer = global::tracer("dek-agent-observer");
    let mut attrs = vec![
        KeyValue::new("gen_ai.operation.name", format!("{:?}", event.event_kind)),
        KeyValue::new("pollen.tenant_id", event.tenant_id.clone()),
        KeyValue::new(
            "pollen.agent_id",
            event.agent_id.clone().unwrap_or_default(),
        ),
        KeyValue::new("pollen.agent_type", format!("{:?}", event.agent_type)),
        KeyValue::new("pollen.task_id", event.task_id.clone().unwrap_or_default()),
        KeyValue::new(
            "pollen.invocation_id",
            event.invocation_id.clone().unwrap_or_default(),
        ),
        KeyValue::new("gen_ai.usage.input_tokens", event.tokens.input_tokens),
        KeyValue::new("gen_ai.usage.output_tokens", event.tokens.output_tokens),
        KeyValue::new(
            "pollen.usage.cached_input_tokens",
            event.tokens.cached_input_tokens,
        ),
        KeyValue::new(
            "pollen.usage.reasoning_output_tokens",
            event.tokens.reasoning_output_tokens,
        ),
        KeyValue::new("pollen.cost.total", event.cost.total_cost),
    ];
    if let Some(provider) = &event.provider {
        attrs.push(KeyValue::new("gen_ai.system", provider.clone()));
    }
    if let Some(model) = &event.model {
        attrs.push(KeyValue::new("gen_ai.request.model", model.clone()));
    }
    if event.cost.currency == "USD" {
        attrs.push(KeyValue::new(
            "pollen.cost.total_usd",
            event.cost.total_cost,
        ));
    }

    tracer
        .span_builder("ai_usage_event")
        .with_kind(SpanKind::Client)
        .with_attributes(attrs)
        .start(&tracer);
}
