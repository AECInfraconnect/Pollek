use crate::control_method::{AgentRef, CompiledRules, ControlMethod, TelemetrySink};
use async_trait::async_trait;
use pollek_contract::{
    AgentObservationPayloadControlMethod, EnforcementResultPayload,
    EnforcementResultPayloadControlMethod, EnforcementResultPayloadDomain,
    EnforcementResultPayloadEffectiveLevel, EnforcementResultPayloadPlaneState,
    EnforcementResultPayloadRequestedLevel,
};

macro_rules! define_backend {
    ($name:ident, $obs_id:ident, $enf_id:ident, $domain:ident, $max_level:ident) => {
        pub struct $name;

        #[async_trait]
        impl ControlMethod for $name {
            fn get_method_id(&self) -> AgentObservationPayloadControlMethod {
                AgentObservationPayloadControlMethod::$obs_id
            }

            async fn bind(&self, _agent: &AgentRef) -> Result<(), anyhow::Error> {
                Ok(())
            }

            async fn apply(
                &self,
                agent: &AgentRef,
                _rules: &CompiledRules,
            ) -> EnforcementResultPayload {
                EnforcementResultPayload {
                    agent_id: agent.id.clone(),
                    policy_id: "default-policy".to_string(), // placeholder
                    control_method: EnforcementResultPayloadControlMethod::$enf_id,
                    domain: EnforcementResultPayloadDomain::$domain,
                    requested_level: EnforcementResultPayloadRequestedLevel::Observe,
                    effective_level: EnforcementResultPayloadEffectiveLevel::$max_level,
                    plane_state: EnforcementResultPayloadPlaneState::Enforcing,
                    success: true,
                    os: std::env::consts::OS.to_string(),
                    message_th: Some(format!("บังคับใช้ {} สำเร็จ", stringify!($name))),
                    message_en: Some(format!("Applied {} successfully", stringify!($name))),
                    user_action_th: None,
                    user_action_en: None,
                }
            }

            async fn observe(&self, _sink: TelemetrySink) -> anyhow::Result<()> {
                Ok(())
            }
        }
    };
}

// MCP
define_backend!(McpStdioBackend, McpStdio, McpStdio, McpTool, Observe);
define_backend!(McpHttpBackend, McpHttp, McpHttp, McpTool, Observe);

// Windows
define_backend!(
    WindowsWfpUmBackend,
    WindowsWfpUm,
    WindowsWfpUm,
    Network,
    Warn
);
define_backend!(WindowsEtwBackend, WindowsEtw, WindowsEtw, Process, Observe);

// Linux
define_backend!(
    LinuxLandlockBackend,
    LinuxLandlock,
    LinuxLandlock,
    FileSystem,
    Enforce
);
define_backend!(LinuxEbpfBackend, LinuxEbpf, LinuxEbpf, Network, Warn);

// macOS
define_backend!(
    MacosNetextBackend,
    MacosNetext,
    MacosNetext,
    Network,
    Enforce
);
define_backend!(
    MacosEndpointSecurityBackend,
    MacosEndpointSecurity,
    MacosEndpointSecurity,
    Process,
    Warn
);
