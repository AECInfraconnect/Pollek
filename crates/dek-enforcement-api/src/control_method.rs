use async_trait::async_trait;
use pollek_contract::{
    AgentObservationPayload, AgentObservationPayloadControlMethod, EnforcementResultPayload,
    IdentityAccessPayload, PollekTelemetryEnvelopeV1, ResourceAccessPayload, ToolUsagePayload,
};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct AgentRef {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct CompiledRules {
    // Level parameter can be customized or retrieved from the Enforcer context
}

pub struct EmitCtx {
    pub tenant_id: String,
    pub device_id: String,
    // Add other envelope fields as needed
}

impl EmitCtx {
    pub fn envelope(&self, event_type: &str, payload: Value) -> PollekTelemetryEnvelopeV1 {
        let mut map = serde_json::Map::new();
        if let Value::Object(obj) = payload {
            map = obj;
        }
        PollekTelemetryEnvelopeV1 {
            schema_version: "telemetry-envelope.v1".to_string(),
            event_id: uuid::Uuid::new_v4().to_string(),
            event_type: event_type.to_string(),
            timestamp: chrono::Utc::now(),
            tenant_id: self.tenant_id.clone(),
            workspace_id: None,
            environment_id: None,
            device_id: self.device_id.clone(),
            trace_id: None,
            span_id: None,
            redaction_applied: false,
            payload: map,
        }
    }
}

#[derive(Clone)]
pub struct TelemetrySink {
    pub tx: tokio::sync::mpsc::Sender<PollekTelemetryEnvelopeV1>,
    pub ctx: std::sync::Arc<EmitCtx>,
}

impl TelemetrySink {
    pub async fn observe(&self, p: AgentObservationPayload) {
        let env = self.ctx.envelope(
            "agent_observation",
            serde_json::to_value(p).unwrap_or_default(),
        );
        let _ = self.tx.send(env).await;
    }

    pub async fn enforcement(&self, p: EnforcementResultPayload) {
        let env = self.ctx.envelope(
            "enforcement_result",
            serde_json::to_value(p).unwrap_or_default(),
        );
        let _ = self.tx.send(env).await;
    }

    pub async fn resource(&self, p: ResourceAccessPayload) {
        let env = self.ctx.envelope(
            "resource_access",
            serde_json::to_value(p).unwrap_or_default(),
        );
        let _ = self.tx.send(env).await;
    }

    pub async fn tool(&self, p: ToolUsagePayload) {
        let env = self
            .ctx
            .envelope("tool_usage", serde_json::to_value(p).unwrap_or_default());
        let _ = self.tx.send(env).await;
    }

    pub async fn identity(&self, p: IdentityAccessPayload) {
        let env = self.ctx.envelope(
            "identity_access",
            serde_json::to_value(p).unwrap_or_default(),
        );
        let _ = self.tx.send(env).await;
    }
}

#[async_trait]
pub trait ControlMethod: Send + Sync {
    fn get_method_id(&self) -> AgentObservationPayloadControlMethod;

    /// Binds an agent to this control method
    async fn bind(&self, agent: &AgentRef) -> Result<(), anyhow::Error>;

    /// บังคับใช้ policy กับ agent หนึ่งตัว — คืนผลที่ emit เป็น EnforcementResult
    async fn apply(&self, agent: &AgentRef, rules: &CompiledRules) -> EnforcementResultPayload;

    /// สังเกตการณ์ -> ส่ง AgentObservationPayload เข้า sink ทุกครั้งที่เกิด decision/กิจกรรม
    async fn observe(&self, sink: TelemetrySink) -> anyhow::Result<()>;
}
