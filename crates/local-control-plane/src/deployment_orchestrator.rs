// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_domain_schema::deployment_session::{
    DeploymentEvent, DeploymentPhase, DeploymentSession, DeploymentSessionStatus, EventStatus,
    LocalizedText,
};
use tokio::sync::mpsc;

pub trait DeploymentEventSink: Send + Sync {
    #[allow(async_fn_in_trait)]
    async fn emit(&self, event: DeploymentEvent) -> anyhow::Result<()>;
}

pub struct MemoryEventSink {
    sender: mpsc::Sender<DeploymentEvent>,
}

impl MemoryEventSink {
    pub fn new(sender: mpsc::Sender<DeploymentEvent>) -> Self {
        Self { sender }
    }
}

impl DeploymentEventSink for MemoryEventSink {
    async fn emit(&self, event: DeploymentEvent) -> anyhow::Result<()> {
        let _ = self.sender.send(event).await;
        Ok(())
    }
}

pub struct StoreEventSink {
    store: std::sync::Arc<dyn crate::store::DeploymentStore>,
}

impl StoreEventSink {
    pub fn new(store: std::sync::Arc<dyn crate::store::DeploymentStore>) -> Self {
        Self { store }
    }
}

impl DeploymentEventSink for StoreEventSink {
    async fn emit(&self, event: DeploymentEvent) -> anyhow::Result<()> {
        tracing::debug!("Emitting deployment event: {:?}", event.event_id);
        self.store.insert_deployment_event(event).await?;
        Ok(())
    }
}

pub struct DeploymentOrchestrator<T: DeploymentEventSink> {
    event_sink: std::sync::Arc<T>,
    store: std::sync::Arc<dyn crate::store::DeploymentStore>,
}

impl<T: DeploymentEventSink> DeploymentOrchestrator<T> {
    pub fn new(event_sink: std::sync::Arc<T>, store: std::sync::Arc<dyn crate::store::DeploymentStore>) -> Self {
        Self { event_sink, store }
    }

    pub async fn transition(
        &self,
        session: &mut DeploymentSession,
        new_status: DeploymentSessionStatus,
    ) -> anyhow::Result<()> {
        session.status = new_status.clone();
        session.updated_at = chrono::Utc::now();

        let phase = match new_status {
            DeploymentSessionStatus::ScanStarted
            | DeploymentSessionStatus::ScanCompleted
            | DeploymentSessionStatus::CapabilitySnapshotCreated => DeploymentPhase::AgentDiscovery,
            DeploymentSessionStatus::PolicyFeasibilityEvaluated
            | DeploymentSessionStatus::UserSelectedPolicy
            | DeploymentSessionStatus::DeploymentPlanCreated => DeploymentPhase::RoutePlanning,
            DeploymentSessionStatus::ApprovalRequired => DeploymentPhase::RoutePlanning,
            DeploymentSessionStatus::BundleCreated | DeploymentSessionStatus::BundleActivated => {
                DeploymentPhase::PepDeploy
            }
            DeploymentSessionStatus::WarmCheckPassed => DeploymentPhase::WarmCheck,
            DeploymentSessionStatus::Active
            | DeploymentSessionStatus::PartialActive
            | DeploymentSessionStatus::ObserveOnlyActive => DeploymentPhase::Enforcement,
            DeploymentSessionStatus::Failed => DeploymentPhase::Rollback,
            DeploymentSessionStatus::RolledBack => DeploymentPhase::Rollback,
        };

        let event = DeploymentEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            deployment_id: session.deployment_id.clone(),
            agent_id: None,
            entity_id: None,
            policy_id: session.policy_id.clone(),
            phase,
            status: EventStatus::Info,
            title: LocalizedText {
                en: format!("Transitioned to {:?}", new_status),
                th: format!("เปลี่ยนสถานะเป็น {:?}", new_status),
            },
            detail: LocalizedText {
                en: "".into(),
                th: "".into(),
            },
            technical_detail: None,
            user_action: None,
            created_at: chrono::Utc::now(),
            correlation_id: session.deployment_id.clone(),
        };

        self.event_sink.emit(event).await?;
        self.store.upsert_deployment_session(session.clone()).await?;
        Ok(())
    }
}
