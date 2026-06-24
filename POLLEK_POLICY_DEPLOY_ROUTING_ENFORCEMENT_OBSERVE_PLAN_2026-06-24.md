# POLLEK Policy Deploy, Auto Routing, Enforcement, and Observe Improvement Plan

Date: 2026-06-24

Repository reviewed: https://github.com/AECInfraconnect/AntiG_Pollen_DEK

Primary focus: make policy deployment, automatic PDP/PEP selection, local enforcement, local observation, telemetry, and user-facing status messages work reliably across different local user environments.

## 1. Executive Summary

POLLEK already has the right product direction: local-first AI Agent Governance Runtime, agent discovery, policy routing, local/cloud PDP, MCP proxy, control modes, secure spool, dashboard, and platform-specific enforcement modules. The current gap is not the idea. The gap is the missing operational layer that turns many separate capabilities into a predictable end-to-end user flow.

The highest priority improvement is to add a first-class Policy Deployment Orchestrator and Routing Explanation Timeline.

The system should not only deploy a policy. It must explain:

1. Which agent was found.
2. Which capabilities the agent exposes.
3. Which enforcement layers are available on this machine.
4. Which PEP was selected and why.
5. Which PDP engine was selected and why.
6. Whether deployment succeeded.
7. Whether enforcement is actually active.
8. Whether telemetry is being observed and synced.
9. What the user must do when a local permission, driver, extension, or config change is required.

Recommended product direction:

- Move from "policy deploy button" to "guided deployment session".
- Show each agent/entity as its own timeline.
- Show every deploy/enforce/observe action as an event with friendly messages.
- Treat unsupported or partially supported environments as expected states, not silent failures.
- Always provide a next action: "working", "active", "observe only", "approval required", "driver required", "cloud offline", "rollback applied", or "manual setup required".

## 2. Current Repo Signals

The repository README states that POLLEK is a local-first AI Agent Governance Runtime that discovers agents, deploys enforceable policies to the right PEP, evaluates decisions through local/cloud PDPs, records telemetry, and gives users a dashboard to observe/control/prove agent behavior.

The repo also states these intended capabilities:

- Auto-select policy engine: Cedar, OPA/Rego, or OpenFGA.
- Policy presets with dynamic PEP capability targeting.
- Control modes: Observe, Warn, Approval, Enforce, StrictDeny.
- Shadow AI Discovery using eBPF/WFP and fingerprinting.
- Secure Telemetry Spool.
- Agent Binding Governance.
- Trust scoring and kill switch.
- Kernel-grade network control with Linux eBPF and in-progress Windows WFP/macOS System Extension.
- Local Admin Dashboard pages for registry, policy, observability, operations, and settings.

The crate landscape also shows the right architectural pieces:

- Control: `dek-core`, `dek-policy-syncer`, `dek-bundle-sync`, `dek-secure-spool`
- Decision: `dek-mcp-proxy`, `dek-policy-router`, `dek-policy-runtime`, `dek-cedar`, `dek-openfga`, `dek-opa-wasm`
- Observability: `dek-agent-discovery`, `dek-agent-observer`, `dek-policy-suggester`, `dek-telemetry`
- Network: `dek-ebpfd`, `dek-windows-wfp`, `dek-macos-nefilter`
- Interop: `dek-agent-binding`, `dek-agent-connector`, `dek-mcp-stdio-wrapper`

Key finding: the repo has the right module boundaries, but the flow needs a canonical deployment state machine, capability probing status, routing explanation, and user-facing event stream.

## 3. Product Goal

The user experience must answer one question clearly:

> "What is POLLEK doing to control this agent on my machine, and is it actually working?"

The dashboard should expose a session-based story, not only raw logs.

Example:

```text
Deployment Session: DEP-2026-06-24-00041
Policy: Prevent sensitive file upload
Target Agent: Claude Desktop
Entity: Local User / Marketing Team

09:41:02 Agent detected: Claude Desktop
09:41:03 Agent capability mapped: MCP stdio server config found
09:41:04 Local capability checked: macOS Network Extension not approved
09:41:05 PEP selected: MCP Stdio Wrapper
09:41:05 PDP selected: Cedar local engine
09:41:06 Policy compiled and signed
09:41:08 User approval required: allow POLLEK to update Claude Desktop MCP config
09:42:13 User approved config change
09:42:14 PEP deployed
09:42:16 Warm check passed
09:42:17 Enforcement active
09:42:20 Telemetry spool active
```

## 4. Required New Core Concept: Deployment Session

Add a deployment session as the canonical unit of work.

Every click on "Deploy Policy" should create one `DeploymentSession`. A session owns all child events for one deployment attempt.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentSession {
    pub deployment_id: String,
    pub policy_id: String,
    pub policy_version: String,
    pub requested_control_level: ControlLevel,
    pub target_scope: DeploymentScope,
    pub status: DeploymentStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentStatus {
    Planning,
    WaitingForUserAction,
    Deploying,
    Active,
    ActiveObserveOnly,
    PartiallyActive,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentScope {
    Agent { agent_id: String },
    Entity { entity_id: String },
    AgentGroup { group_id: String },
    Device { device_id: String },
}
```

Implementation location:

- Add shared schema to `crates/dek-domain-schema`.
- Persist session state in `local-control-plane` SQLite.
- Expose through `dek-control-plane-api`.
- Mirror a summarized version to cloud telemetry when cloud mode is enabled.

## 5. Required New Event Model

Add a structured timeline event model. This becomes the source for dashboard UX, local logs, secure spool, and cloud dashboard.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentEvent {
    pub event_id: String,
    pub deployment_id: String,
    pub agent_id: Option<String>,
    pub entity_id: Option<String>,
    pub policy_id: String,
    pub phase: DeploymentPhase,
    pub status: EventStatus,
    pub title: LocalizedText,
    pub detail: LocalizedText,
    pub technical_detail: Option<serde_json::Value>,
    pub user_action: Option<UserAction>,
    pub created_at: DateTime<Utc>,
    pub correlation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentPhase {
    AgentDiscovery,
    CapabilityCheck,
    RoutePlanning,
    PolicyCompile,
    BundleSign,
    PepDeploy,
    PdpHealthCheck,
    WarmCheck,
    Enforcement,
    Observe,
    TelemetrySync,
    Rollback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    Info,
    Success,
    Warning,
    Error,
    ActionRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalizedText {
    pub en: String,
    pub th: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAction {
    pub action_id: String,
    pub kind: UserActionKind,
    pub label: LocalizedText,
    pub help: LocalizedText,
    pub safe_to_retry: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserActionKind {
    ApproveConfigPatch,
    ApproveSystemExtension,
    InstallDriver,
    StartLocalService,
    ConnectCloud,
    RebuildPolicyBundle,
    RetryDeployment,
}
```

Rules:

- Every non-trivial deploy step must emit an event.
- Every downgrade must emit a warning event with reason.
- Every failed capability must emit an actionable message.
- Every selected PEP/PDP must emit "selected because" metadata.
- Timeline event IDs must be stable and searchable.

## 6. Policy Deploy State Machine

The deployment orchestrator should implement this state machine:

```text
draft_created
  -> target_resolved
  -> agents_scanned
  -> capabilities_checked
  -> route_planned
  -> policy_compiled
  -> bundle_signed
  -> pep_staged
  -> user_action_required?
  -> pep_deployed
  -> pdp_health_checked
  -> warm_check_passed
  -> active
  -> observed
  -> telemetry_synced
```

Failure states:

```text
capability_missing
pdp_unavailable
pep_not_supported
user_approval_required
policy_compile_failed
signature_failed
warm_check_failed
rollback_started
rollback_completed
rollback_failed
```

Each state transition must create a `DeploymentEvent`.

## 7. Auto Select and Auto Routing Model

The router must not choose only a PDP. It must choose a full route:

- Agent surface
- PEP layer
- PDP engine
- Control level
- Fallback behavior
- Observability path
- Required user action
- Friendly explanation

### Routing Inputs

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePlanningInput {
    pub deployment_id: String,
    pub policy_id: String,
    pub policy_intent: PolicyIntent,
    pub requested_control_level: ControlLevel,
    pub target_agent: AgentProfile,
    pub device_capabilities: DeviceCapabilityReport,
    pub pdp_health: Vec<PdpHealth>,
    pub policy_bundle_fresh: bool,
    pub cloud_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyIntent {
    ToolCallAllowDeny,
    ParameterRedaction,
    NetworkEgressControl,
    RelationshipAuthorization,
    PromptContentGuard,
    CostLimit,
    KillSwitch,
    ObserveOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    pub agent_id: String,
    pub display_name: String,
    pub surfaces: Vec<AgentSurface>,
    pub risk_score: u8,
    pub trust_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSurface {
    McpStdio { config_path: String },
    McpHttp { base_url: String },
    OpenAiCompatibleHttp { base_url: String },
    BrowserExtension { browser: String, extension_id: String },
    LocalModelServer { base_url: String },
    Container { runtime: String, container_id: String },
    ProcessOnly { pid: u32 },
    NetworkOnly { pid: Option<u32>, host: String },
}
```

### Routing Output

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingPlan {
    pub deployment_id: String,
    pub agent_id: String,
    pub selected_pep: PepSelection,
    pub selected_pdp: PdpSelection,
    pub effective_control_level: ControlLevel,
    pub observability_path: ObservabilityPath,
    pub fallback: FallbackPlan,
    pub user_messages: Vec<UserMessage>,
    pub required_actions: Vec<UserAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepSelection {
    pub pep_id: String,
    pub layer: EnforcementLayer,
    pub status: CapabilityStatus,
    pub reason_code: String,
    pub reason: LocalizedText,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdpSelection {
    pub pdp_id: String,
    pub engine: PdpEngine,
    pub mode: PdpRouteMode,
    pub status: CapabilityStatus,
    pub reason_code: String,
    pub reason: LocalizedText,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnforcementLayer {
    McpProxy,
    McpStdioWrapper,
    HttpProxy,
    EbpfNetwork,
    WindowsWfp,
    MacosNetworkExtension,
    BrowserExtension,
    ObserveOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PdpEngine {
    Cedar,
    OpaWasm,
    OpenFga,
    Cloud,
    RouterOnly,
}
```

## 8. Capability Registry Must Store Status, Not Just Boolean Support

Current capability detection should evolve from "supported/not supported" to explainable statuses.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilityReport {
    pub device_id: String,
    pub os: OsProfile,
    pub peps: Vec<PepCapabilityStatus>,
    pub pdps: Vec<PdpCapabilityStatus>,
    pub scanned_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PepCapabilityStatus {
    pub layer: EnforcementLayer,
    pub status: CapabilityStatus,
    pub confidence: f32,
    pub detected_version: Option<String>,
    pub reason_code: String,
    pub user_message: LocalizedText,
    pub next_action: Option<UserActionKind>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Ready,
    ReadyRequiresApproval,
    InstalledInactive,
    MissingPermission,
    MissingDriver,
    MissingBinary,
    UnsupportedOs,
    Unhealthy,
    Unknown,
}
```

Example capability checks:

```rust
pub async fn probe_mcp_stdio(agent: &AgentProfile) -> PepCapabilityStatus {
    let found = agent.surfaces.iter().any(|s| matches!(s, AgentSurface::McpStdio { .. }));

    if found {
        return PepCapabilityStatus {
            layer: EnforcementLayer::McpStdioWrapper,
            status: CapabilityStatus::ReadyRequiresApproval,
            confidence: 0.95,
            detected_version: None,
            reason_code: "mcp_stdio_config_found".into(),
            user_message: LocalizedText {
                en: "POLLEK found an MCP stdio configuration for this agent. It can enforce tool calls by wrapping the stdio server after you approve the config change.".into(),
                th: "พบการตั้งค่า MCP stdio ของ Agent นี้ ระบบสามารถควบคุม tool call ผ่าน stdio wrapper ได้ หลังจากคุณอนุมัติการแก้ไข config".into(),
            },
            next_action: Some(UserActionKind::ApproveConfigPatch),
        };
    }

    PepCapabilityStatus {
        layer: EnforcementLayer::McpStdioWrapper,
        status: CapabilityStatus::Unknown,
        confidence: 0.0,
        detected_version: None,
        reason_code: "mcp_stdio_not_detected".into(),
        user_message: LocalizedText {
            en: "No MCP stdio configuration was found for this agent.".into(),
            th: "ไม่พบการตั้งค่า MCP stdio สำหรับ Agent นี้".into(),
        },
        next_action: None,
    }
}
```

## 9. PEP Selection Rules

The route planner should select the least invasive layer that can actually enforce the requested policy.

Recommended priority:

1. MCP Proxy or MCP Stdio Wrapper for MCP tool calls and parameter-level policies.
2. HTTP Proxy for OpenAI-compatible local servers and API egress.
3. Browser extension for browser-hosted AI workflows.
4. eBPF/WFP/NetworkExtension for network egress enforcement when app-layer integration is unavailable.
5. ObserveOnly when no safe enforcement layer is ready.

Example selector:

```rust
pub fn select_pep(
    policy_intent: &PolicyIntent,
    agent: &AgentProfile,
    caps: &DeviceCapabilityReport,
) -> PepSelection {
    let has_surface = |predicate: fn(&AgentSurface) -> bool| agent.surfaces.iter().any(predicate);
    let pep_ready = |layer: EnforcementLayer| {
        caps.peps
            .iter()
            .find(|p| p.layer == layer)
            .map(|p| p.status.clone())
    };

    match policy_intent {
        PolicyIntent::ToolCallAllowDeny | PolicyIntent::ParameterRedaction => {
            if has_surface(|s| matches!(s, AgentSurface::McpHttp { .. })) {
                return explain_pep(
                    "mcp_http_best_fit",
                    EnforcementLayer::McpProxy,
                    CapabilityStatus::Ready,
                    "Selected MCP Proxy because this agent exposes MCP over HTTP and the policy controls tool calls.",
                    "เลือก MCP Proxy เพราะ Agent นี้ใช้ MCP ผ่าน HTTP และ policy นี้ควบคุม tool call",
                );
            }

            if has_surface(|s| matches!(s, AgentSurface::McpStdio { .. })) {
                return explain_pep(
                    "mcp_stdio_best_fit",
                    EnforcementLayer::McpStdioWrapper,
                    CapabilityStatus::ReadyRequiresApproval,
                    "Selected MCP Stdio Wrapper because this agent uses MCP stdio. User approval is required before patching the agent config.",
                    "เลือก MCP Stdio Wrapper เพราะ Agent นี้ใช้ MCP stdio ต้องได้รับอนุมัติก่อนแก้ไข config",
                );
            }
        }
        PolicyIntent::NetworkEgressControl => {
            for layer in [
                EnforcementLayer::EbpfNetwork,
                EnforcementLayer::WindowsWfp,
                EnforcementLayer::MacosNetworkExtension,
            ] {
                if matches!(pep_ready(layer.clone()), Some(CapabilityStatus::Ready)) {
                    return explain_pep(
                        "network_layer_ready",
                        layer,
                        CapabilityStatus::Ready,
                        "Selected the OS network enforcement layer because this policy controls network egress.",
                        "เลือกชั้นควบคุม network ของ OS เพราะ policy นี้ควบคุมการออก network",
                    );
                }
            }
        }
        PolicyIntent::RelationshipAuthorization => {
            return explain_pep(
                "relationship_policy_router_only",
                EnforcementLayer::McpProxy,
                CapabilityStatus::Ready,
                "Selected MCP Proxy because relationship checks can be enforced before tool execution.",
                "เลือก MCP Proxy เพราะสามารถตรวจสิทธิ์ความสัมพันธ์ก่อนเรียก tool ได้",
            );
        }
        _ => {}
    }

    explain_pep(
        "fallback_observe_only",
        EnforcementLayer::ObserveOnly,
        CapabilityStatus::Ready,
        "No safe enforcement layer is ready for this agent. POLLEK will observe activity and suggest setup steps.",
        "ยังไม่มีชั้น enforcement ที่พร้อมและปลอดภัยสำหรับ Agent นี้ ระบบจะ Observe ก่อนและแนะนำขั้นตอนตั้งค่า",
    )
}

fn explain_pep(
    reason_code: &str,
    layer: EnforcementLayer,
    status: CapabilityStatus,
    en: &str,
    th: &str,
) -> PepSelection {
    PepSelection {
        pep_id: format!("{:?}", layer).to_lowercase(),
        layer,
        status,
        reason_code: reason_code.into(),
        reason: LocalizedText {
            en: en.into(),
            th: th.into(),
        },
    }
}
```

## 10. PDP Selection Rules

PDP selection should follow policy semantics:

- Cedar: ABAC/RBAC, local allow/deny, principal/action/resource/context.
- OPA WASM: complex rule logic, transformations, risk scoring, content conditions.
- OpenFGA: relationship-based authorization.
- Cloud PDP: tenant-wide policy, managed policy, central audit, remote decisioning.
- RouterOnly: emergency deny, break-glass, simple observe-only.

Example:

```rust
pub fn select_pdp(
    input: &RoutePlanningInput,
    selected_pep: &PepSelection,
) -> PdpSelection {
    if input.policy_intent == PolicyIntent::RelationshipAuthorization {
        return pdp(
            "openfga_relationship",
            PdpEngine::OpenFga,
            PdpRouteMode::LocalPrimaryRemoteFallback,
            "Selected OpenFGA because this policy depends on entity relationships.",
            "เลือก OpenFGA เพราะ policy นี้ตรวจสิทธิ์จากความสัมพันธ์ของ entity",
        );
    }

    if matches!(input.policy_intent, PolicyIntent::PromptContentGuard | PolicyIntent::CostLimit) {
        return pdp(
            "opa_complex_logic",
            PdpEngine::OpaWasm,
            PdpRouteMode::LocalPrimaryRemoteFallback,
            "Selected OPA WASM because this policy needs complex conditional logic.",
            "เลือก OPA WASM เพราะ policy นี้ต้องใช้ logic เงื่อนไขซับซ้อน",
        );
    }

    if input.cloud_connected && input.requested_control_level == ControlLevel::StrictDeny {
        return pdp(
            "cloud_strict_deny",
            PdpEngine::Cloud,
            PdpRouteMode::CloudPrimaryLocalFallback,
            "Selected Cloud PDP with local fallback because this strict policy should stay aligned with central governance.",
            "เลือก Cloud PDP พร้อม local fallback เพราะ policy ระดับ Strict ควรตรงกับ governance ส่วนกลาง",
        );
    }

    if selected_pep.layer == EnforcementLayer::ObserveOnly {
        return pdp(
            "router_observe_only",
            PdpEngine::RouterOnly,
            PdpRouteMode::MirrorAuditOnly,
            "Selected observe-only routing because no active enforcement layer is ready.",
            "เลือก observe-only routing เพราะยังไม่มี enforcement layer ที่พร้อมใช้งาน",
        );
    }

    pdp(
        "cedar_default_local",
        PdpEngine::Cedar,
        PdpRouteMode::LocalOnly,
        "Selected Cedar local engine because this policy is a standard local allow/deny rule.",
        "เลือก Cedar local engine เพราะ policy นี้เป็นกฎ allow/deny แบบ local มาตรฐาน",
    )
}

fn pdp(
    reason_code: &str,
    engine: PdpEngine,
    mode: PdpRouteMode,
    en: &str,
    th: &str,
) -> PdpSelection {
    PdpSelection {
        pdp_id: format!("{:?}", engine).to_lowercase(),
        engine,
        mode,
        status: CapabilityStatus::Ready,
        reason_code: reason_code.into(),
        reason: LocalizedText { en: en.into(), th: th.into() },
    }
}
```

## 11. Full Route Planner

```rust
pub async fn build_routing_plan(
    input: RoutePlanningInput,
    event_sink: &dyn DeploymentEventSink,
) -> anyhow::Result<RoutingPlan> {
    event_sink.emit(info(
        &input.deployment_id,
        DeploymentPhase::CapabilityCheck,
        "Checking local enforcement capabilities",
        "กำลังตรวจสอบความพร้อมของ enforcement ในเครื่องนี้",
    )).await?;

    let pep = select_pep(&input.policy_intent, &input.target_agent, &input.device_capabilities);
    event_sink.emit(selection_event(
        &input.deployment_id,
        DeploymentPhase::RoutePlanning,
        "Selected enforcement layer",
        "เลือกชั้น enforcement แล้ว",
        &pep.reason,
        serde_json::json!({
            "pep_id": pep.pep_id,
            "layer": pep.layer,
            "status": pep.status,
            "reason_code": pep.reason_code,
        }),
    )).await?;

    let pdp = select_pdp(&input, &pep);
    event_sink.emit(selection_event(
        &input.deployment_id,
        DeploymentPhase::RoutePlanning,
        "Selected policy decision point",
        "เลือก PDP แล้ว",
        &pdp.reason,
        serde_json::json!({
            "pdp_id": pdp.pdp_id,
            "engine": pdp.engine,
            "mode": pdp.mode,
            "reason_code": pdp.reason_code,
        }),
    )).await?;

    let required_actions = required_actions_for(&pep, &pdp);
    let effective_control_level = effective_control_level(
        input.requested_control_level,
        &pep,
        &pdp,
    );

    Ok(RoutingPlan {
        deployment_id: input.deployment_id,
        agent_id: input.target_agent.agent_id,
        selected_pep: pep,
        selected_pdp: pdp,
        effective_control_level,
        observability_path: ObservabilityPath::SecureSpoolAndOtel,
        fallback: FallbackPlan::default(),
        user_messages: vec![],
        required_actions,
    })
}
```

## 12. Friendly Message Catalog

Create a message catalog instead of hardcoding UI text in many modules.

```rust
pub struct MessageCatalog;

impl MessageCatalog {
    pub fn pep_selected_mcp_stdio(agent_name: &str) -> LocalizedText {
        LocalizedText {
            en: format!(
                "POLLEK selected MCP Stdio Wrapper for {agent_name} because this agent uses MCP over stdio. You need to approve a config update before enforcement starts."
            ),
            th: format!(
                "ระบบเลือก MCP Stdio Wrapper สำหรับ {agent_name} เพราะ Agent นี้ใช้ MCP ผ่าน stdio คุณต้องอนุมัติการแก้ไข config ก่อนเริ่ม enforcement"
            ),
        }
    }

    pub fn pep_fallback_observe_only(agent_name: &str) -> LocalizedText {
        LocalizedText {
            en: format!(
                "POLLEK cannot safely enforce {agent_name} yet. It will observe activity and show setup recommendations."
            ),
            th: format!(
                "ระบบยังไม่สามารถ enforce {agent_name} ได้อย่างปลอดภัย จึงจะ Observe ก่อนและแสดงคำแนะนำการตั้งค่า"
            ),
        }
    }

    pub fn enforcement_active(agent_name: &str, layer: &str) -> LocalizedText {
        LocalizedText {
            en: format!("Enforcement is active for {agent_name} through {layer}."),
            th: format!("Enforcement สำหรับ {agent_name} เริ่มทำงานแล้วผ่าน {layer}"),
        }
    }
}
```

Message quality rules:

- Use "Selected X because Y".
- Never show only internal enum names.
- Always include "what happens now".
- For warnings, include "what the user can do".
- Avoid blaming the user or OS.

## 13. UX Layout Recommendation

Add or modify dashboard screens:

### Policy Deployment Wizard

Steps:

1. Select policy or preset.
2. Select target agents/entities.
3. Preview routing plan.
4. Review required permissions/config changes.
5. Deploy.
6. Watch timeline.

### Routing Preview Panel

Show:

- Target agent.
- Agent surface detected.
- Selected PEP.
- Selected PDP.
- Effective control level.
- Fallback behavior.
- Required user action.

Example:

```text
Agent: Cursor
Detected Surface: MCP HTTP + OpenAI-compatible HTTP
Selected PEP: MCP Proxy
Selected PDP: OPA WASM
Control Level: Enforce
Fallback: Local allow cache for 60s, then fail closed
User Action: None
Status: Ready to deploy
```

### Agent Timeline

Each agent gets a timeline grouped by deployment session.

Fields:

- Time
- Event title
- Friendly explanation
- Status
- Technical details expandable
- Retry/fix button when action is required

### Enforcement Layer Status

Show per layer:

```text
MCP Proxy: Ready
MCP Stdio Wrapper: Ready, approval required per app
Linux eBPF: Not available on this OS
Windows WFP: Not available on this OS
macOS Network Extension: Installed but not approved
Secure Spool: Active
Cloud Sync: Offline, local queue active
```

## 14. Frontend Types and Timeline Component

```ts
export type EventStatus =
  | "info"
  | "success"
  | "warning"
  | "error"
  | "action_required";

export type DeploymentPhase =
  | "agent_discovery"
  | "capability_check"
  | "route_planning"
  | "policy_compile"
  | "bundle_sign"
  | "pep_deploy"
  | "pdp_health_check"
  | "warm_check"
  | "enforcement"
  | "observe"
  | "telemetry_sync"
  | "rollback";

export interface LocalizedText {
  en: string;
  th: string;
}

export interface DeploymentEvent {
  event_id: string;
  deployment_id: string;
  agent_id?: string;
  entity_id?: string;
  policy_id: string;
  phase: DeploymentPhase;
  status: EventStatus;
  title: LocalizedText;
  detail: LocalizedText;
  technical_detail?: Record<string, unknown>;
  user_action?: {
    action_id: string;
    kind: string;
    label: LocalizedText;
    help: LocalizedText;
    safe_to_retry: boolean;
  };
  created_at: string;
}
```

```tsx
type Props = {
  events: DeploymentEvent[];
  locale: "en" | "th";
  onAction: (actionId: string) => void;
};

export function AgentDeploymentTimeline({ events, locale, onAction }: Props) {
  return (
    <section className="timeline">
      {events.map((event) => (
        <article key={event.event_id} className={`timeline-item ${event.status}`}>
          <div className="timeline-time">
            {new Date(event.created_at).toLocaleTimeString()}
          </div>
          <div className="timeline-body">
            <div className="timeline-title">{event.title[locale]}</div>
            <div className="timeline-detail">{event.detail[locale]}</div>
            {event.user_action && (
              <button onClick={() => onAction(event.user_action!.action_id)}>
                {event.user_action.label[locale]}
              </button>
            )}
          </div>
        </article>
      ))}
    </section>
  );
}
```

## 15. API Contract

Add these endpoints to `local-control-plane` and mirror to cloud where needed:

```http
POST /v1/policies/{policy_id}/deploy-plan
POST /v1/deployments
GET  /v1/deployments/{deployment_id}
GET  /v1/deployments/{deployment_id}/events
GET  /v1/agents/{agent_id}/timeline
POST /v1/deployments/{deployment_id}/actions/{action_id}/approve
POST /v1/deployments/{deployment_id}/retry
POST /v1/deployments/{deployment_id}/rollback
GET  /v1/capabilities/local
GET  /v1/enforcement-layers/status
```

Example deploy-plan request:

```json
{
  "policy_id": "pol_sensitive_file_upload",
  "target": { "type": "agent", "agent_id": "agent_claude_desktop" },
  "requested_control_level": "enforce",
  "dry_run": true
}
```

Example deploy-plan response:

```json
{
  "deployment_id": "dep_01JZPOLLEK001",
  "status": "waiting_for_user_action",
  "selected_pep": {
    "layer": "mcp_stdio_wrapper",
    "status": "ready_requires_approval",
    "reason_code": "mcp_stdio_best_fit",
    "reason": {
      "en": "Selected MCP Stdio Wrapper because this agent uses MCP stdio.",
      "th": "เลือก MCP Stdio Wrapper เพราะ Agent นี้ใช้ MCP stdio"
    }
  },
  "selected_pdp": {
    "engine": "cedar",
    "mode": "local_only",
    "status": "ready"
  },
  "required_actions": [
    {
      "action_id": "act_approve_claude_config_patch",
      "kind": "approve_config_patch",
      "label": {
        "en": "Approve config update",
        "th": "อนุมัติการแก้ไข config"
      }
    }
  ]
}
```

## 16. Enforcement Confirmation

Deployment is not complete when a config file is patched or a route is saved. It is complete only when a warm check proves the selected PEP and PDP path works.

Warm check examples:

- MCP HTTP: send a safe `tools/list` or dry-run tool call through the proxy.
- MCP stdio: start wrapper in dry-run and verify process handshake.
- HTTP proxy: send a local health request and verify routing metadata.
- eBPF/WFP/NetworkExtension: install a test deny rule for a local synthetic endpoint and verify event capture without breaking traffic.
- ObserveOnly: verify telemetry event reaches local store/spool.

Code contract:

```rust
#[async_trait::async_trait]
pub trait PepWarmCheck {
    async fn warm_check(&self, plan: &RoutingPlan) -> anyhow::Result<WarmCheckResult>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmCheckResult {
    pub ok: bool,
    pub layer: EnforcementLayer,
    pub checked_at: DateTime<Utc>,
    pub latency_ms: u64,
    pub message: LocalizedText,
    pub technical_detail: serde_json::Value,
}
```

## 17. Observe and Telemetry Flow

All enforcement and observation events should share the same correlation model.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceTelemetryEnvelope {
    pub event_id: String,
    pub correlation_id: String,
    pub deployment_id: Option<String>,
    pub policy_id: Option<String>,
    pub agent_id: Option<String>,
    pub entity_id: Option<String>,
    pub source_layer: EnforcementLayer,
    pub event_kind: TelemetryEventKind,
    pub occurred_at: DateTime<Utc>,
    pub redaction_state: RedactionState,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEventKind {
    AgentDiscovered,
    PolicyRouteSelected,
    PolicyDecision,
    ToolCallObserved,
    ToolCallBlocked,
    NetworkEgressObserved,
    NetworkEgressBlocked,
    PdpUnavailable,
    PepUnhealthy,
    UserActionRequired,
    UserActionCompleted,
    TelemetrySynced,
}
```

Required behavior:

- Local dashboard reads from local event store first.
- Secure spool handles durable async export.
- Cloud dashboard receives summarized deployment and enforcement events.
- Offline mode shows "Cloud sync paused. Local evidence is still being recorded."
- Reconnect should sync in order with backpressure.

## 18. Platform-Specific Routing Guidance

### Windows

Minimum target recommendation:

- Windows 10 22H2 or Windows 11 for production support.
- Use WFP only when driver/service installation is successful and health check passes.
- For regular users without admin rights, downgrade to app-layer PEP or ObserveOnly with clear message.

Friendly message:

```text
EN: Windows network enforcement is not active because the WFP driver is not installed. POLLEK will use MCP/HTTP enforcement where possible and observe the rest.
TH: Network enforcement บน Windows ยังไม่ทำงานเพราะยังไม่ได้ติดตั้ง WFP driver ระบบจะใช้ MCP/HTTP enforcement เท่าที่ทำได้ และ Observe ส่วนที่เหลือ
```

### macOS

Minimum target recommendation:

- macOS 13+ for modern NetworkExtension/System Extension support.
- NetworkExtension must be treated as "approval required" until the OS reports active status.
- MCP stdio/http should be preferred when available because it is less invasive and easier to explain.

Friendly message:

```text
EN: macOS Network Extension is installed but not approved. Open System Settings to allow it, or continue with MCP-level enforcement.
TH: ติดตั้ง macOS Network Extension แล้วแต่ยังไม่ได้อนุมัติ ให้เปิด System Settings เพื่ออนุญาต หรือใช้ MCP-level enforcement ต่อไป
```

### Linux

Minimum target recommendation:

- Prefer modern LTS distributions with kernel features needed by eBPF.
- If eBPF cannot load due to kernel, capability, or privilege limits, downgrade to HTTP/MCP PEP or ObserveOnly.

Friendly message:

```text
EN: Linux eBPF enforcement is unavailable in this environment. POLLEK will use application-level enforcement and keep observing network activity where possible.
TH: eBPF enforcement บน Linux ใช้งานไม่ได้ใน environment นี้ ระบบจะใช้ application-level enforcement และ Observe network เท่าที่ทำได้
```

## 19. Agent and Entity Event Grouping

The dashboard should group events in two dimensions:

1. By deployment session.
2. By target agent/entity.

Example groups:

```text
Entity: Finance Team
  Policy: Block external upload of financial files
    Agent: Claude Desktop
      DEP-001 active via MCP Stdio Wrapper
    Agent: Cursor
      DEP-002 active via MCP Proxy
    Agent: Browser AI
      DEP-003 observe-only, extension not installed
```

This is important because one policy may produce different enforcement routes on the same machine.

## 20. Stub Replacement Priorities

### Priority 1: Capability status must be real

Replace optimistic OS checks with probes that report:

- ready
- installed but inactive
- missing permission
- missing driver/binary
- unsupported OS
- unhealthy
- unknown

### Priority 2: Agent registration must preserve discovered capabilities

When converting discovered candidates into registered agents, do not drop:

- MCP config paths
- HTTP endpoints
- container IDs
- process identity
- confidence signals
- suggested PEP
- discovered tools/resources

### Priority 3: Router must emit explanation events

`dek-policy-router` should keep fast authorization, but route planning and explanation should be separated:

- `plan_route()` produces explainable route.
- `authorize()` executes already planned route.
- `authorize_dry_run()` returns route + decision + warnings.

### Priority 4: Secure spool facade must be wired

Deployment/enforcement events should be persisted through a real durable path, not a no-op facade.

### Priority 5: Warm check before active status

No dashboard should show "Active" until warm check succeeds.

## 21. Suggested PR Backlog

### PR-001: Domain schema for deployment sessions and timeline events

Files:

- `crates/dek-domain-schema`
- `schemas`
- `contracts`

Deliverables:

- `DeploymentSession`
- `DeploymentEvent`
- `RoutingPlan`
- JSON Schema
- TypeSpec/OpenAPI updates
- Serialization tests

### PR-002: Capability probe status model

Files:

- `crates/dek-capability-registry`
- `crates/dek-ebpfd`
- `crates/dek-windows-wfp`
- `crates/dek-macos-nefilter`

Deliverables:

- real capability probes
- status reason codes
- friendly message mapping
- cross-platform tests with mocked probes

### PR-003: Deployment orchestrator

Files:

- `crates/dek-core`
- `crates/local-control-plane`
- `crates/dek-policy-syncer`

Deliverables:

- session state machine
- event emission
- deployment plan endpoint
- action approval endpoint
- rollback/retry endpoint

### PR-004: Explainable route planner

Files:

- `crates/dek-policy-router`
- `crates/dek-agent-binding`
- `crates/dek-policy-presets`

Deliverables:

- `RoutePlanningInput`
- `RoutingPlan`
- PEP/PDP selector
- dry-run route preview
- route explanation tests

### PR-005: PEP warm checks

Files:

- `crates/dek-mcp-proxy`
- `crates/dek-mcp-stdio-wrapper`
- `crates/dek-ebpfd`
- `crates/dek-windows-wfp`
- `crates/dek-macos-nefilter`

Deliverables:

- `PepWarmCheck` trait
- warm check implementations
- "active only after verified" behavior

### PR-006: Timeline UI

Files:

- `apps/local-admin-dashboard`

Deliverables:

- Deployment Wizard
- Routing Preview Panel
- Agent Timeline
- Enforcement Layer Status
- Action Required panel
- Thai/English messages

### PR-007: Telemetry correlation and secure spool integration

Files:

- `crates/dek-telemetry`
- `crates/dek-secure-spool`
- `crates/dek-agent-observer`

Deliverables:

- correlated event envelope
- durable local event storage
- ordered cloud sync
- offline/online status messages

### PR-008: End-to-end scenario tests

Files:

- `tests`
- `load_tests`
- platform-specific test harnesses

Scenarios:

- Claude Desktop MCP stdio config approval.
- Cursor MCP HTTP enforcement.
- Local Ollama observe-only fallback.
- Windows missing WFP driver.
- macOS NetworkExtension approval required.
- Linux eBPF unavailable due permission.
- Cloud offline but local enforcement active.

## 22. Acceptance Criteria

Policy deployment is production-ready only when all criteria pass:

- A user can deploy a policy to a detected agent without reading technical documentation.
- The system can explain why it selected a PEP and PDP.
- The dashboard shows per-agent and per-entity event history.
- Unsupported local environments downgrade gracefully.
- Every downgrade has a friendly message and next action.
- "Active" means warm check passed.
- ObserveOnly is explicit, not hidden.
- Telemetry persists locally when cloud is offline.
- Cloud sync resumes without duplicate or out-of-order critical events.
- Agent registration preserves discovered capabilities.
- Stub/no-op paths are removed from deploy, enforce, observe, and telemetry flows.

## 23. Deep Research References

Repository:

- https://github.com/AECInfraconnect/AntiG_Pollen_DEK

MCP:

- https://modelcontextprotocol.io/docs/getting-started/intro
- https://modelcontextprotocol.io/specification/draft
- https://modelcontextprotocol.io/specification/2025-03-26/server/tools

Observability:

- https://opentelemetry.io/docs/specs/semconv/gen-ai/
- https://opentelemetry.io/blog/2024/otel-generative-ai/

Windows WFP:

- https://learn.microsoft.com/en-us/windows-hardware/drivers/network/introduction-to-windows-filtering-platform-callout-drivers
- https://learn.microsoft.com/en-us/windows-hardware/drivers/network/roadmap-for-developing-wfp-callout-drivers

macOS NetworkExtension:

- https://developer.apple.com/documentation/networkextension
- https://developer.apple.com/documentation/networkextension/content-filter-providers
- https://developer.apple.com/documentation/networkextension/nefilterprovider

Policy engines:

- Cedar: https://www.cedarpolicy.com/
- OPA WASM: https://www.openpolicyagent.org/docs/latest/wasm/
- OpenFGA: https://openfga.dev/docs/interacting/relationship-queries

## 24. Instruction Prompt for Next AI Agent

Use this prompt to continue development:

```text
You are implementing production-grade policy deployment and explainable routing for POLLEK in the AntiG_Pollen_DEK repository.

Focus on these outcomes:
1. Add DeploymentSession, DeploymentEvent, RoutingPlan, CapabilityStatus schemas.
2. Implement a Deployment Orchestrator that emits events for every state transition.
3. Implement explainable PEP/PDP auto-selection with friendly Thai/English messages.
4. Replace optimistic capability booleans with real probe statuses.
5. Add warm checks so the dashboard only shows Active after a verified enforcement path.
6. Preserve discovered agent capabilities during registration.
7. Add dashboard timeline grouped by deployment session, agent, and entity.
8. Wire observe/enforce events into local telemetry and secure spool.

Do not only add UI labels. The backend must produce the event model and the UI must render it.

Prioritize PR-001 to PR-004 first. Add tests for route selection:
- MCP stdio -> MCP Stdio Wrapper + user approval
- MCP HTTP -> MCP Proxy
- relationship policy -> OpenFGA
- complex prompt/content policy -> OPA WASM
- standard allow/deny -> Cedar
- missing PEP -> ObserveOnly with warning and next action
```
