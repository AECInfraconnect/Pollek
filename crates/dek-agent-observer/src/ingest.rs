use crate::model::AgentObservationEvent;

pub fn ingest_event(event: AgentObservationEvent) -> Result<(), String> {
    // Basic validation
    if event.event_id.is_empty() {
        return Err("event_id is required".to_string());
    }

    // In a real implementation, this would save to SQLite or send to a message queue
    println!("Ingested event: {}", event.event_id);

    Ok(())
}
