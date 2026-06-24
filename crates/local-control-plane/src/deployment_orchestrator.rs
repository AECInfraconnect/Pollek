// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use dek_domain_schema::deployment_session::{
    DeploymentEvent, DeploymentPhase, DeploymentSession, DeploymentSessionStatus, EventStatus,
    LocalizedText,
};
use tokio::sync::mpsc;

pub trait DeploymentEventSink: Send + Sync {
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

pub struct DeploymentOrchestrator<T: DeploymentEventSink> {
    event_sink: std::sync::Arc<T>,
}

impl<T: DeploymentEventSink> DeploymentOrchestrator<T> {
    pub fn new(event_sink: std::sync::Arc<T>) -> Self {
        Self { event_sink }
    }

    pub async fn transition(
        &self,
        session: &mut DeploymentSession,
        new_status: DeploymentSessionStatus,
    ) -> anyhow::Result<()> {
        session.status = new_status.clone();
        session.updated_at = chrono::Utc::now();

        let phase = match new_status {
            DeploymentSessionStatus::Planning => DeploymentPhase::RoutePlanning,
            DeploymentSessionStatus::Deploying => DeploymentPhase::PepDeploy,
            DeploymentSessionStatus::WaitingForUserAction => DeploymentPhase::RoutePlanning,
            DeploymentSessionStatus::Active
            | DeploymentSessionStatus::PartiallyActive
            | DeploymentSessionStatus::ActiveObserveOnly => DeploymentPhase::Enforcement,
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
        Ok(())
    }
}
