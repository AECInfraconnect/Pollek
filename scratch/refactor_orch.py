import os

# 1. Update state.rs
state_path = "crates/local-control-plane/src/state.rs"
with open(state_path, "r", encoding="utf-8") as f:
    state_content = f.read()

if "pub deployment_store: Arc<dyn store::DeploymentStore>," not in state_content:
    state_content = state_content.replace(
        "pub observability_store: Arc<dyn store::ObservabilityStore>,",
        "pub observability_store: Arc<dyn store::ObservabilityStore>,\n    pub deployment_store: Arc<dyn store::DeploymentStore>,"
    )
    with open(state_path, "w", encoding="utf-8") as f:
        f.write(state_content)
    print("Updated state.rs")


# 2. Update main.rs
main_path = "crates/local-control-plane/src/main.rs"
with open(main_path, "r", encoding="utf-8") as f:
    main_content = f.read()

if "deployment_store: store.clone()," not in main_content:
    main_content = main_content.replace(
        "observability_store: store.clone(),",
        "observability_store: store.clone(),\n        deployment_store: store.clone(),"
    )
    with open(main_path, "w", encoding="utf-8") as f:
        f.write(main_content)
    print("Updated main.rs")

# 3. Update deployment_orchestrator.rs
orch_path = "crates/local-control-plane/src/deployment_orchestrator.rs"
with open(orch_path, "r", encoding="utf-8") as f:
    orch_content = f.read()

if "pub struct StoreEventSink {" in orch_content:
    orch_content = orch_content.replace(
        """pub struct StoreEventSink {
    // In a real implementation, this would hold database pool and telemetry spool references.
}

impl Default for StoreEventSink {
    fn default() -> Self {
        Self::new()
    }
}

impl StoreEventSink {
    pub fn new() -> Self {
        Self {}
    }
}""",
        """pub struct StoreEventSink {
    store: std::sync::Arc<dyn crate::store::DeploymentStore>,
}

impl StoreEventSink {
    pub fn new(store: std::sync::Arc<dyn crate::store::DeploymentStore>) -> Self {
        Self { store }
    }
}"""
    )
    
    orch_content = orch_content.replace(
        """    async fn emit(&self, event: DeploymentEvent) -> anyhow::Result<()> {
        // Pseudo-code for secure telemetry and timeline integration:
        // 1. Write to local event store (SQLite) for the timeline view.
        // 2. Write to secure telemetry spool for cloud/admin sync.

        // Ensure correlation ID, policy ID, and agent/entity IDs are present.
        let _correlation_id = &event.correlation_id;
        let _policy_id = &event.policy_id;
        let _agent_id = &event.agent_id;

        // Emitting to local log
        tracing::debug!("Emitting deployment event: {:?}", event.event_id);

        Ok(())
    }""",
        """    async fn emit(&self, event: DeploymentEvent) -> anyhow::Result<()> {
        tracing::debug!("Emitting deployment event: {:?}", event.event_id);
        self.store.insert_deployment_event(event).await?;
        Ok(())
    }"""
    )

    orch_content = orch_content.replace(
        """        self.event_sink.emit(event).await?;
        Ok(())""",
        """        self.event_sink.emit(event).await?;
        // Now upsert the session into the database
        let store: &dyn crate::store::DeploymentStore = unsafe { std::mem::transmute(&*self.event_sink as *const _ as *const dyn crate::store::DeploymentStore) };
        // Wait, StoreEventSink implements DeploymentEventSink, but DeploymentOrchestrator doesn't know about store directly.
        // Let's add store to DeploymentOrchestrator.
        Ok(())"""
    )
    # Actually, the above comment is right. DeploymentOrchestrator needs the store.
    # Let's fix that.
    orch_content = orch_content.replace(
        """pub struct DeploymentOrchestrator<T: DeploymentEventSink> {
    event_sink: std::sync::Arc<T>,
}

impl<T: DeploymentEventSink> DeploymentOrchestrator<T> {
    pub fn new(event_sink: std::sync::Arc<T>) -> Self {
        Self { event_sink }
    }""",
        """pub struct DeploymentOrchestrator<T: DeploymentEventSink> {
    event_sink: std::sync::Arc<T>,
    store: std::sync::Arc<dyn crate::store::DeploymentStore>,
}

impl<T: DeploymentEventSink> DeploymentOrchestrator<T> {
    pub fn new(event_sink: std::sync::Arc<T>, store: std::sync::Arc<dyn crate::store::DeploymentStore>) -> Self {
        Self { event_sink, store }
    }"""
    )

    orch_content = orch_content.replace(
        """        self.event_sink.emit(event).await?;
        // Now upsert the session into the database
        let store: &dyn crate::store::DeploymentStore = unsafe { std::mem::transmute(&*self.event_sink as *const _ as *const dyn crate::store::DeploymentStore) };
        // Wait, StoreEventSink implements DeploymentEventSink, but DeploymentOrchestrator doesn't know about store directly.
        // Let's add store to DeploymentOrchestrator.
        Ok(())""",
        """        self.event_sink.emit(event).await?;
        self.store.upsert_deployment_session(session.clone()).await?;
        Ok(())"""
    )

    with open(orch_path, "w", encoding="utf-8") as f:
        f.write(orch_content)
    print("Updated deployment_orchestrator.rs")

