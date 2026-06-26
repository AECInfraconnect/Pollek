use async_trait::async_trait;
use dek_agent_observer::egress_parser::classify_cloud_egress;
use dek_enforcement_api::control_method::TelemetrySink;
use dek_enforcement_api::egress_observer::EgressEventSource;
use pollen_contract::{
    IdentityAccessPayload, IdentityAccessPayloadAction, IdentityAccessPayloadDecision,
    IdentityAccessPayloadIdentityKind, IdentityAccessPayloadScope, ResourceAccessPayload,
    ResourceAccessPayloadDecision, ResourceAccessPayloadKind, ResourceAccessPayloadMode,
    ResourceAccessPayloadScope,
};

pub struct SimulatorEgressSource {
    pub deterministic: bool,
}

#[async_trait]
impl EgressEventSource for SimulatorEgressSource {
    fn id(&self) -> &str {
        "simulator_egress"
    }

    async fn start_observing(&self, sink: TelemetrySink) -> anyhow::Result<()> {
        let fixtures = [
            "api.openai.com",
            "api.anthropic.com",
            "huggingface.co",
            "drive.google.com",
        ];
        let mut idx = 0;

        loop {
            // Sleep duration depends on whether we need deterministic rapid testing or a realistic demo.
            tokio::time::sleep(std::time::Duration::from_secs(if self.deterministic {
                5
            } else {
                15
            }))
            .await;

            let host = fixtures[idx % fixtures.len()];
            idx += 1;

            if let Some((kind_str, name)) = classify_cloud_egress(host) {
                let kind = match kind_str.as_str() {
                    "api" => ResourceAccessPayloadKind::Api,
                    "cloud_drive" => ResourceAccessPayloadKind::CloudDrive,
                    "saas" => ResourceAccessPayloadKind::Saas,
                    "email" => ResourceAccessPayloadKind::Email,
                    _ => ResourceAccessPayloadKind::Web,
                };

                let payload = ResourceAccessPayload {
                    agent_id: "agent-simulator".into(),
                    agent_label: "Simulator Agent".into(),
                    scope: ResourceAccessPayloadScope::Cloud,
                    kind,
                    target_redacted: host.into(),
                    target_hash: host.into(),
                    mode: ResourceAccessPayloadMode::Connect,
                    decision: ResourceAccessPayloadDecision::Allow,
                    control_method: None,
                    enforced_for_real: false,
                    bytes: Some(1024),
                    count: Some(1),
                    classification: Some(name.clone()),
                    observed_at: chrono::Utc::now(),
                };

                sink.resource(payload).await;

                let identity_payload = IdentityAccessPayload {
                    agent_id: "agent-simulator".into(),
                    agent_label: "Simulator Agent".into(),
                    scope: IdentityAccessPayloadScope::Cloud,
                    identity_kind: IdentityAccessPayloadIdentityKind::ServiceAccount,
                    identity_id: format!("cloud-service-account:{}", host),
                    identity_label: format!("{} service identity", name),
                    provider: Some(name),
                    spiffe_id: Some("spiffe://local/pollek/agent/agent-simulator".into()),
                    action: IdentityAccessPayloadAction::Access,
                    decision: IdentityAccessPayloadDecision::Allow,
                    control_method: None,
                    enforced_for_real: false,
                    observed_at: chrono::Utc::now(),
                };

                sink.identity(identity_payload).await;
            }
        }
    }
}
