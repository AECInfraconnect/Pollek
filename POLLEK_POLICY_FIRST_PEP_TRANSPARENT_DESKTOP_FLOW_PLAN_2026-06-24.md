# POLLEK Policy-First and PEP-Transparent Desktop Flow Plan

Date: 2026-06-24

Repository reviewed: https://github.com/AECInfraconnect/AntiG_Pollen_DEK

Purpose: improve POLLEK so general Windows/macOS/Linux users do not need to understand or select PEP types. Users should choose policies and control levels. POLLEK should auto-detect local capabilities, auto-route to the best enforcement/observe method, explain what is possible, and only expose low-level PEP configuration in Advanced or Enterprise mode.

## 1. Core Decision

For normal desktop users, POLLEK should be Policy-first and PEP-transparent.

The user should not be asked:

```text
Which PEP do you want to deploy?
```

The user should be asked:

```text
What do you want POLLEK to control or observe?
How strongly should POLLEK control it?
Which agents or users should this apply to?
```

Then POLLEK should:

1. Scan local agents.
2. Scan local capability.
3. Match policies to what can actually be enforced or observed.
4. Suggest policies that fit the user's local environment.
5. Auto-select the control method internally.
6. Tell the user in friendly terms what will happen.
7. Show whether enforcement is full, partial, observe-only, or requires setup.

PEP, PDP, WFP, eBPF, NetworkExtension, MCP proxy, stdio wrapper, and HTTP proxy should remain technical details behind "How POLLEK will control this agent".

## 2. Why This Direction Is Correct

General desktop machines are not managed servers. They usually do not have kernel/network enforcement components pre-installed, and OS-level enforcement often requires administrator rights, system extension approval, driver installation, signing, or MDM.

Therefore, a PEP-selection UI is the wrong default for normal users.

Correct user-facing model:

```text
Policy -> Feasibility -> Control Level -> Deploy
```

Internal model:

```text
PolicyIntent -> CapabilityMatch -> ControlMethodPlan -> PEP/PDP route -> Bundle -> Runtime enforcement
```

This matches POLLEK's existing product direction. The README already positions POLLEK as a local-first AI Agent Governance Runtime that discovers agents, deploys policies to the right PEP, evaluates through local/cloud PDPs, records telemetry, and gives users a dashboard. The README also states dynamic PEP targeting, control modes, policy suggestions, and a local dashboard. The improvement here is to make those capabilities usable without requiring the user to know the term PEP.

## 3. Product Modes

POLLEK should support three modes.

### 3.1 Desktop Simple Mode

Default mode for personal devices and SMB users.

Visible concepts:

- Agents
- Policies
- Control Level
- Status
- What POLLEK can do on this machine
- What setup is required

Hidden concepts:

- PEP type
- PDP engine
- route failover
- OS driver details
- policy bundle internals

Example UI:

```text
Claude Desktop
Detected: Yes
Recommended Policies:
  - Approve risky tool calls: Can enforce after config approval
  - Redact PII before tool use: Can enforce after config approval
  - Block unknown network destinations: Needs system setup
  - Observe token/cost usage: Can observe now
```

### 3.2 Desktop Advanced Mode

For technical users.

Additional visible concepts:

- Control method
- Selected PDP
- fallback behavior
- health checks
- diagnostics

Use friendly labels:

```text
Control Method: MCP Stdio Wrapper
Decision Engine: Cedar local
Status: Waiting for config approval
Fallback: Observe only
```

Do not make the advanced mode required for success.

### 3.3 Enterprise / Server Mode

For managed servers, VDI, MDM, fleet deployments, and SOC/security teams.

Visible concepts:

- PEP selection
- PDP route selection
- fail-open/fail-closed
- WFP/eBPF/NetworkExtension deployment
- driver health
- policy bundle signing
- tenant-wide rollout
- rollback strategy

This mode can expose PEP explicitly because the operator is expected to understand infrastructure consequences.

## 4. New UX Flow

Replace a PEP deploy flow with a Policy Feasibility flow.

```text
1. Scan
   -> Find agents, surfaces, local model servers, MCP configs, browser/IDE hints

2. Capability Analysis
   -> Determine what POLLEK can observe/enforce now
   -> Determine what needs setup
   -> Determine what is unsupported on this machine

3. Policy Suggestions
   -> Suggest policies that are feasible for the detected agents
   -> Show impact and setup requirements

4. User Selects Policy and Control Level
   -> User chooses desired policy and level
   -> User does not choose PEP

5. Deploy Plan Preview
   -> Show "Can enforce", "Partial", "Observe only", or "Needs setup"
   -> Show friendly reason

6. Deploy
   -> Orchestrator creates session
   -> Builds route internally
   -> Compiles/signs bundle
   -> Deploys or observes

7. Timeline
   -> Show events per agent/entity/session
   -> Show required actions
```

## 5. User-Facing Status Levels

Use a status vocabulary that non-technical users can understand.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyFeasibilityStatus {
    CanEnforceNow,
    CanEnforceAfterApproval,
    CanPartiallyEnforce,
    CanObserveOnly,
    NeedsSetup,
    Unsupported,
    Unknown,
}
```

User-facing meanings:

| Status | Meaning | User message style |
|---|---|---|
| `CanEnforceNow` | POLLEK can enforce this policy immediately | "Ready to enforce" |
| `CanEnforceAfterApproval` | A safe local change needs user approval | "Approve setup to enforce" |
| `CanPartiallyEnforce` | Some parts can enforce, some will observe | "Partially enforceable" |
| `CanObserveOnly` | POLLEK can record and alert but cannot block | "Observe only on this machine" |
| `NeedsSetup` | Install/enable driver, extension, browser extension, or config | "Setup required" |
| `Unsupported` | This machine/agent cannot support this policy | "Not supported here" |
| `Unknown` | Scan incomplete or insufficient evidence | "Run scan again" |

## 6. Policy Feasibility Model

Add a first-class model that sits above PEP/PDP routing.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyFeasibilityRequest {
    pub policy_id: Option<String>,
    pub policy_intent: PolicyIntent,
    pub requested_control_level: ControlLevel,
    pub targets: Vec<PolicyTarget>,
    pub mode: ProductMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductMode {
    DesktopSimple,
    DesktopAdvanced,
    EnterpriseServer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyTarget {
    Agent { agent_id: String },
    Entity { entity_id: String },
    AgentGroup { group_id: String },
    Device,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyFeasibilityResult {
    pub target: PolicyTarget,
    pub policy_intent: PolicyIntent,
    pub requested_control_level: ControlLevel,
    pub effective_control_level: ControlLevel,
    pub status: PolicyFeasibilityStatus,
    pub user_summary: LocalizedText,
    pub user_detail: LocalizedText,
    pub required_actions: Vec<RequiredUserAction>,
    pub technical_plan: Option<ControlMethodPlan>,
    pub confidence: f32,
}
```

In `DesktopSimple`, `technical_plan` should be hidden by default in the API response or only returned when `include_technical_details=true`.

## 7. Policy Intent Taxonomy

Policies should be expressed as user goals before they become PEP/PDP routes.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyIntent {
    ObserveAgentActivity,
    ApproveRiskyToolCalls,
    BlockSpecificTools,
    RedactSensitiveParameters,
    BlockSensitiveFileUpload,
    BlockUnknownNetworkDestinations,
    RestrictLocalModelUsage,
    LimitTokenOrCostUsage,
    RequireEntityRelationship,
    DetectPromptInjection,
    KillSwitchOnAnomaly,
}
```

Mapping examples:

| Policy Intent | Best common desktop control method | Fallback |
|---|---|---|
| Observe agent activity | process/MCP/HTTP/browser observation | event-only |
| Approve risky tool calls | MCP proxy or stdio wrapper | warn/observe |
| Block specific tools | MCP proxy or stdio wrapper | warn/observe |
| Redact sensitive parameters | MCP proxy or stdio wrapper | warn/observe |
| Block sensitive file upload | MCP layer if tool-mediated, OS/network only if not | observe + alert |
| Block unknown network destinations | OS network layer or HTTP proxy | observe-only |
| Restrict local model usage | HTTP proxy/local model endpoint control | observe |
| Limit token or cost usage | HTTP/MCP observation + rate limit | observe |
| Require entity relationship | OpenFGA/Cedar at app layer | observe |
| Detect prompt injection | content guard before PDP | warn/observe |
| Kill switch on anomaly | app-layer where possible, OS-layer if available | require approval |

## 8. Control Method Abstraction

Use `ControlMethod` as the user-facing abstraction. It maps to PEP internally.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ControlMethod {
    AgentToolControl,
    AgentConfigWrapper,
    LocalApiControl,
    BrowserActivityMonitor,
    NetworkControl,
    ProcessObservation,
    ObserveOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlMethodPlan {
    pub method: ControlMethod,
    pub internal_pep: InternalPep,
    pub internal_pdp: InternalPdp,
    pub enforceability: Enforceability,
    pub reason_code: String,
    pub explanation: LocalizedText,
    pub diagnostics: Vec<DiagnosticFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InternalPep {
    McpProxy,
    McpStdioWrapper,
    HttpProxy,
    BrowserExtension,
    LinuxEbpf,
    WindowsWfp,
    MacosNetworkExtension,
    SecureSpoolObserver,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InternalPdp {
    Cedar,
    OpaWasm,
    OpenFga,
    Cloud,
    RouterOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enforceability {
    pub can_observe: bool,
    pub can_warn: bool,
    pub can_require_approval: bool,
    pub can_enforce: bool,
    pub can_strict_deny: bool,
}
```

User-facing names:

| Internal PEP | User-facing label |
|---|---|
| `McpProxy` | Agent tool control |
| `McpStdioWrapper` | Agent config wrapper |
| `HttpProxy` | Local API control |
| `BrowserExtension` | Browser activity monitor |
| `LinuxEbpf` | System network control |
| `WindowsWfp` | System network control |
| `MacosNetworkExtension` | System network control |
| `SecureSpoolObserver` | Activity observation |
| `None` | Not available |

## 9. Capability Scan Must Produce User-Actionable Findings

The current capability registry should evolve from "does this OS support X" to "what is actually usable now".

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalCapabilitySnapshot {
    pub snapshot_id: String,
    pub device_id: String,
    pub os: OsProfile,
    pub agents: Vec<DetectedAgent>,
    pub methods: Vec<ControlMethodCapability>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlMethodCapability {
    pub method: ControlMethod,
    pub internal_pep: InternalPep,
    pub status: CapabilityStatus,
    pub can_observe: bool,
    pub can_enforce: bool,
    pub requires_admin: bool,
    pub requires_user_approval: bool,
    pub confidence: f32,
    pub evidence: Vec<CapabilityEvidence>,
    pub user_message: LocalizedText,
    pub next_action: Option<RequiredUserAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityStatus {
    Ready,
    ReadyAfterApproval,
    InstalledInactive,
    MissingPermission,
    MissingComponent,
    UnsupportedOnThisOs,
    UnsupportedForThisAgent,
    Unhealthy,
    Unknown,
}
```

Examples:

```rust
pub fn capability_to_user_status(cap: &ControlMethodCapability) -> PolicyFeasibilityStatus {
    match cap.status {
        CapabilityStatus::Ready if cap.can_enforce => PolicyFeasibilityStatus::CanEnforceNow,
        CapabilityStatus::ReadyAfterApproval if cap.can_enforce => {
            PolicyFeasibilityStatus::CanEnforceAfterApproval
        }
        CapabilityStatus::Ready if cap.can_observe => PolicyFeasibilityStatus::CanObserveOnly,
        CapabilityStatus::MissingPermission | CapabilityStatus::MissingComponent => {
            PolicyFeasibilityStatus::NeedsSetup
        }
        CapabilityStatus::UnsupportedOnThisOs | CapabilityStatus::UnsupportedForThisAgent => {
            PolicyFeasibilityStatus::Unsupported
        }
        _ => PolicyFeasibilityStatus::Unknown,
    }
}
```

## 10. Policy Suggestion Engine

Add a policy suggestion step immediately after agent and capability scan.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedPolicy {
    pub suggestion_id: String,
    pub policy_template_id: String,
    pub display_name: LocalizedText,
    pub description: LocalizedText,
    pub target_agent_ids: Vec<String>,
    pub recommended_control_level: ControlLevel,
    pub feasibility: PolicyFeasibilityStatus,
    pub confidence: f32,
    pub reason_codes: Vec<String>,
    pub setup_required: Vec<RequiredUserAction>,
}

pub trait PolicySuggestionEngine {
    fn suggest(
        &self,
        snapshot: &LocalCapabilitySnapshot,
        templates: &[PolicyTemplate],
    ) -> Vec<SuggestedPolicy>;
}
```

Suggestion rules:

```rust
pub fn suggest_for_agent(agent: &DetectedAgent, caps: &[ControlMethodCapability]) -> Vec<SuggestedPolicy> {
    let mut suggestions = Vec::new();

    if agent.has_mcp_surface() {
        suggestions.push(template(
            "approve_risky_tool_calls",
            ControlLevel::Approval,
            PolicyFeasibilityStatus::CanEnforceAfterApproval,
            "Agent exposes MCP tools, so POLLEK can review tool calls before execution.",
            "Agent นี้มี MCP tools ระบบจึงสามารถให้ตรวจและอนุมัติ tool call ก่อนทำงานได้",
        ));

        suggestions.push(template(
            "redact_sensitive_parameters",
            ControlLevel::Enforce,
            PolicyFeasibilityStatus::CanEnforceAfterApproval,
            "Agent tool parameters can be inspected and redacted before execution.",
            "ระบบสามารถตรวจและ redact parameter ของ tool ก่อน execution ได้",
        ));
    }

    if agent.has_openai_compatible_http_surface() {
        suggestions.push(template(
            "limit_token_or_cost_usage",
            ControlLevel::Warn,
            PolicyFeasibilityStatus::CanEnforceNow,
            "Local API traffic can be measured and rate-limited.",
            "ระบบสามารถวัดและจำกัดการใช้งานผ่าน local API ได้",
        ));
    }

    if has_network_control_ready(caps) {
        suggestions.push(template(
            "block_unknown_network_destinations",
            ControlLevel::Enforce,
            PolicyFeasibilityStatus::CanEnforceNow,
            "System network control is ready on this device.",
            "เครื่องนี้พร้อมใช้ system network control",
        ));
    } else {
        suggestions.push(template(
            "observe_unknown_network_destinations",
            ControlLevel::Observe,
            PolicyFeasibilityStatus::CanObserveOnly,
            "System network control is not ready, but POLLEK can still observe known agent activity.",
            "system network control ยังไม่พร้อม แต่ระบบยัง Observe activity ของ Agent ที่รู้จักได้",
        ));
    }

    suggestions
}
```

## 11. Feasibility Planner Algorithm

The planner should produce an honest answer before deployment.

```rust
pub fn evaluate_policy_feasibility(
    req: PolicyFeasibilityRequest,
    snapshot: &LocalCapabilitySnapshot,
) -> Vec<PolicyFeasibilityResult> {
    req.targets
        .iter()
        .map(|target| {
            let agent = resolve_agent(target, snapshot);
            let candidates = candidate_methods_for_intent(&req.policy_intent, agent.as_ref(), snapshot);
            let best = select_best_control_method(&req, candidates);
            build_feasibility_result(&req, target.clone(), best)
        })
        .collect()
}

fn select_best_control_method(
    req: &PolicyFeasibilityRequest,
    candidates: Vec<ControlMethodPlan>,
) -> ControlMethodPlan {
    let mut sorted = candidates;
    sorted.sort_by_key(|plan| score_plan(req, plan));
    sorted.pop().unwrap_or_else(observe_only_plan)
}

fn score_plan(req: &PolicyFeasibilityRequest, plan: &ControlMethodPlan) -> i32 {
    let mut score = 0;

    if plan.enforceability.can_enforce {
        score += 100;
    }
    if plan.enforceability.can_require_approval {
        score += 70;
    }
    if plan.enforceability.can_warn {
        score += 50;
    }
    if plan.enforceability.can_observe {
        score += 20;
    }

    // Prefer least invasive methods for desktop simple mode.
    if matches!(req.mode, ProductMode::DesktopSimple) {
        score += match plan.method {
            ControlMethod::AgentToolControl => 40,
            ControlMethod::AgentConfigWrapper => 30,
            ControlMethod::LocalApiControl => 25,
            ControlMethod::BrowserActivityMonitor => 15,
            ControlMethod::NetworkControl => -10,
            ControlMethod::ProcessObservation => 10,
            ControlMethod::ObserveOnly => 0,
        };
    }

    // Enterprise/server mode can prefer stronger OS/network controls.
    if matches!(req.mode, ProductMode::EnterpriseServer) {
        if matches!(plan.method, ControlMethod::NetworkControl) {
            score += 40;
        }
    }

    score
}
```

Key rule:

```text
DesktopSimple should prefer app-layer control over OS network control when both can satisfy the policy.
```

Rationale:

- MCP/HTTP/browser integration is easier to explain.
- OS network control has more installation/approval friction.
- Least invasive control is better for desktop trust.

## 12. Control Level Negotiation

If the user requests `Enforce`, but the machine can only `Observe`, POLLEK should not fail silently. It should create a downgraded plan and ask for confirmation.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlLevelNegotiation {
    pub requested: ControlLevel,
    pub effective: ControlLevel,
    pub downgraded: bool,
    pub reason: LocalizedText,
    pub requires_user_confirmation: bool,
}

pub fn negotiate_control_level(
    requested: ControlLevel,
    enforceability: &Enforceability,
) -> ControlLevelNegotiation {
    let effective = match requested {
        ControlLevel::StrictDeny if enforceability.can_strict_deny => ControlLevel::StrictDeny,
        ControlLevel::StrictDeny if enforceability.can_enforce => ControlLevel::Enforce,
        ControlLevel::Enforce if enforceability.can_enforce => ControlLevel::Enforce,
        ControlLevel::Enforce if enforceability.can_require_approval => ControlLevel::Approval,
        ControlLevel::Approval if enforceability.can_require_approval => ControlLevel::Approval,
        ControlLevel::Warn if enforceability.can_warn => ControlLevel::Warn,
        _ if enforceability.can_observe => ControlLevel::Observe,
        _ => ControlLevel::Observe,
    };

    let downgraded = effective != requested;

    ControlLevelNegotiation {
        requested,
        effective,
        downgraded,
        reason: if downgraded {
            LocalizedText {
                en: "This device cannot fully enforce the requested level yet. POLLEK will use the strongest available safe mode.".into(),
                th: "เครื่องนี้ยัง enforce ตามระดับที่ขอได้ไม่เต็มที่ ระบบจะใช้ระดับที่ปลอดภัยและทำได้จริงที่สุด".into(),
            }
        } else {
            LocalizedText {
                en: "The requested control level is supported on this device.".into(),
                th: "เครื่องนี้รองรับ control level ที่เลือก".into(),
            }
        },
        requires_user_confirmation: downgraded,
    }
}
```

## 13. Deployment Orchestrator Placement

Keep the earlier architecture decision:

- `local-control-plane` owns the deployment session, UI API, DB writes, user approval, event timeline, feasibility preview, policy suggestion flow, and deploy/rollback/retry state machine.
- Route planning logic should be a reusable pure module, not embedded in React/UI code.
- `dek-core` should load active bundles and enforce/observe at runtime.

Recommended crate/module split:

```text
crates/dek-domain-schema
  - PolicyFeasibilityResult
  - ControlMethodPlan
  - DeploymentSession
  - DeploymentEvent

crates/dek-deployment-planner   (new)
  - policy feasibility planner
  - capability-to-policy matcher
  - control method selector
  - PDP selector
  - no DB, no HTTP, no UI

crates/local-control-plane
  - HTTP API
  - SQLite persistence
  - deployment session state machine
  - user actions and approvals
  - calls dek-deployment-planner
  - builds policy bundle

crates/dek-core
  - active bundle loading
  - runtime enforcement
  - telemetry emission

apps/local-admin-dashboard
  - Policy-first UX
  - feasibility cards
  - timeline
  - advanced diagnostics
```

## 14. API Design

Add policy-first endpoints.

```http
POST /v1/local/scan
GET  /v1/local/capability-snapshot/latest
GET  /v1/policy-suggestions
POST /v1/policies/feasibility
POST /v1/deployment-sessions
GET  /v1/deployment-sessions/{deployment_id}
GET  /v1/deployment-sessions/{deployment_id}/events
POST /v1/deployment-sessions/{deployment_id}/actions/{action_id}/approve
POST /v1/deployment-sessions/{deployment_id}/retry
POST /v1/deployment-sessions/{deployment_id}/rollback
```

Example request:

```json
{
  "policy_intent": "redact_sensitive_parameters",
  "requested_control_level": "enforce",
  "targets": [
    { "agent": { "agent_id": "agent_claude_desktop" } }
  ],
  "mode": "desktop_simple"
}
```

Example response:

```json
{
  "results": [
    {
      "target": { "agent": { "agent_id": "agent_claude_desktop" } },
      "policy_intent": "redact_sensitive_parameters",
      "requested_control_level": "enforce",
      "effective_control_level": "approval",
      "status": "can_enforce_after_approval",
      "user_summary": {
        "en": "POLLEK can enforce this policy after you approve one local config change.",
        "th": "ระบบ enforce policy นี้ได้หลังจากคุณอนุมัติการแก้ไข config ในเครื่องหนึ่งรายการ"
      },
      "user_detail": {
        "en": "Claude Desktop uses MCP over stdio, so POLLEK will wrap the MCP server and inspect tool parameters before execution.",
        "th": "Claude Desktop ใช้ MCP ผ่าน stdio ระบบจะ wrap MCP server เพื่อตรวจ parameter ของ tool ก่อน execution"
      },
      "required_actions": [
        {
          "kind": "approve_config_patch",
          "label": {
            "en": "Approve config update",
            "th": "อนุมัติการแก้ไข config"
          }
        }
      ]
    }
  ]
}
```

In `DesktopSimple`, do not include `internal_pep` unless explicitly requested.

## 15. Dashboard Flow

### 15.1 Home Screen

Show:

```text
POLLEK scanned this device

Agents found: 5
Ready to observe: 5
Ready to enforce: 2
Needs setup for stronger control: 3

Recommended next action:
Enable approval for risky tool calls on Claude Desktop and Cursor.
```

### 15.2 Agent Detail

Show policy feasibility, not PEP.

```text
Agent: Claude Desktop

What POLLEK can do:
  - Observe activity: Ready
  - Approve risky tool calls: Needs your approval
  - Redact sensitive parameters: Needs your approval
  - Block network destinations: Needs system setup

Recommended policies:
  1. Approve risky tool calls
  2. Redact sensitive parameters
  3. Observe token/cost usage
```

### 15.3 Deploy Preview

```text
Policy: Redact sensitive parameters
Target: Claude Desktop
Selected Level: Enforce

Result:
POLLEK can enforce this after one approval.

What will happen:
1. POLLEK will update the agent's local MCP config.
2. POLLEK will route tool calls through a local control layer.
3. Sensitive fields will be redacted before the tool runs.
4. Events will be recorded locally.

Action needed:
Approve the config update.
```

### 15.4 Advanced Details Collapsible

```text
Control Method: Agent config wrapper
Internal PEP: MCP Stdio Wrapper
Decision Engine: Cedar local
Fallback: Observe only if wrapper cannot start
Warm check: Required before active
```

## 16. Friendly Message Catalog

```rust
pub struct PolicyFirstMessages;

impl PolicyFirstMessages {
    pub fn can_enforce_now(policy: &str, agent: &str) -> LocalizedText {
        LocalizedText {
            en: format!("POLLEK can enforce '{policy}' for {agent} now."),
            th: format!("ระบบสามารถ enforce '{policy}' กับ {agent} ได้ทันที"),
        }
    }

    pub fn approval_required(policy: &str, agent: &str) -> LocalizedText {
        LocalizedText {
            en: format!(
                "POLLEK can enforce '{policy}' for {agent} after you approve a local setup change."
            ),
            th: format!(
                "ระบบสามารถ enforce '{policy}' กับ {agent} ได้หลังจากคุณอนุมัติการตั้งค่าในเครื่อง"
            ),
        }
    }

    pub fn observe_only(policy: &str, agent: &str) -> LocalizedText {
        LocalizedText {
            en: format!(
                "POLLEK can observe '{policy}' for {agent}, but cannot block it on this device yet."
            ),
            th: format!(
                "ระบบสามารถ Observe '{policy}' กับ {agent} ได้ แต่ยัง block บนเครื่องนี้ไม่ได้"
            ),
        }
    }

    pub fn needs_system_setup(policy: &str) -> LocalizedText {
        LocalizedText {
            en: format!(
                "'{policy}' needs additional system setup before enforcement can start."
            ),
            th: format!(
                "'{policy}' ต้องตั้งค่าระบบเพิ่มเติมก่อนเริ่ม enforcement"
            ),
        }
    }
}
```

Message rules:

- Say what POLLEK can do.
- Say what POLLEK cannot do yet.
- Say why in user language.
- Say what action is required.
- Put technical terms in Advanced Details only.

## 17. Deployment Session Events

Events should also use policy-first wording.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserVisibleDeploymentEvent {
    pub event_id: String,
    pub deployment_id: String,
    pub agent_id: Option<String>,
    pub policy_id: String,
    pub stage: DeploymentStage,
    pub status: EventStatus,
    pub title: LocalizedText,
    pub detail: LocalizedText,
    pub action: Option<RequiredUserAction>,
    pub advanced: Option<AdvancedDeploymentDetail>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedDeploymentDetail {
    pub internal_pep: InternalPep,
    pub internal_pdp: InternalPdp,
    pub route_id: String,
    pub bundle_id: Option<String>,
    pub diagnostics: serde_json::Value,
}
```

Example event:

```json
{
  "stage": "capability_check",
  "status": "success",
  "title": {
    "en": "Control capability checked",
    "th": "ตรวจสอบความสามารถในการควบคุมแล้ว"
  },
  "detail": {
    "en": "POLLEK can control this agent's tool calls after one approval.",
    "th": "ระบบสามารถควบคุม tool call ของ Agent นี้ได้หลังจากอนุมัติหนึ่งรายการ"
  },
  "advanced": {
    "internal_pep": "mcp_stdio_wrapper",
    "internal_pdp": "cedar",
    "route_id": "route_01"
  }
}
```

## 18. Desktop Capability Probing Strategy

### 18.1 Windows

Default desktop behavior:

- Prefer MCP/HTTP/app-layer control.
- Treat WFP as advanced/system network control.
- Do not ask normal users to choose WFP.
- Show WFP only as "System network control".
- If not installed/active, show "Needs system setup".

Probe outputs:

```rust
ControlMethodCapability {
    method: ControlMethod::NetworkControl,
    internal_pep: InternalPep::WindowsWfp,
    status: CapabilityStatus::MissingComponent,
    can_observe: false,
    can_enforce: false,
    requires_admin: true,
    requires_user_approval: true,
    confidence: 0.9,
    evidence: vec![],
    user_message: LocalizedText {
        en: "System network control is not installed on this Windows device.".into(),
        th: "เครื่อง Windows นี้ยังไม่ได้ติดตั้ง system network control".into(),
    },
    next_action: Some(RequiredUserAction::install_system_component("windows_network_control")),
}
```

### 18.2 macOS

Default desktop behavior:

- Prefer MCP/HTTP/app-layer control.
- Treat NetworkExtension as advanced/system network control.
- Show it as approval-required if installed but inactive.

### 18.3 Linux

Default desktop behavior:

- Prefer MCP/HTTP/app-layer control.
- Use eBPF only when capability, kernel, permissions, and probe health are ready.
- If not ready, fall back to observe/app-layer.

## 19. Policy Template Metadata

Policy presets need feasibility metadata.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyTemplate {
    pub template_id: String,
    pub display_name: LocalizedText,
    pub description: LocalizedText,
    pub intent: PolicyIntent,
    pub supported_control_levels: Vec<ControlLevel>,
    pub required_capabilities: Vec<CapabilityRequirement>,
    pub preferred_methods: Vec<ControlMethod>,
    pub fallback_allowed: bool,
    pub default_for_desktop: bool,
    pub default_for_enterprise: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequirement {
    pub method: ControlMethod,
    pub minimum: RequiredCapabilityLevel,
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredCapabilityLevel {
    Observe,
    Warn,
    Approval,
    Enforce,
    StrictDeny,
}
```

Example:

```json
{
  "template_id": "redact_sensitive_tool_parameters",
  "intent": "redact_sensitive_parameters",
  "supported_control_levels": ["warn", "approval", "enforce"],
  "preferred_methods": ["agent_tool_control", "agent_config_wrapper"],
  "required_capabilities": [
    {
      "method": "agent_tool_control",
      "minimum": "enforce",
      "optional": true
    },
    {
      "method": "agent_config_wrapper",
      "minimum": "approval",
      "optional": true
    }
  ],
  "fallback_allowed": true,
  "default_for_desktop": true,
  "default_for_enterprise": true
}
```

## 20. Implementation Backlog

### PR-001: Policy-first domain schema

Add to `crates/dek-domain-schema`:

- `PolicyIntent`
- `ProductMode`
- `PolicyFeasibilityStatus`
- `PolicyFeasibilityRequest`
- `PolicyFeasibilityResult`
- `ControlMethod`
- `ControlMethodPlan`
- `InternalPep`
- `InternalPdp`

Tests:

- serde roundtrip
- OpenAPI/TypeSpec generation
- compatibility with current `ControlLevel`

### PR-002: Capability snapshot and control method mapping

Add to `crates/dek-capability-registry`:

- `LocalCapabilitySnapshot`
- `ControlMethodCapability`
- method-to-PEP mapping
- status reason codes
- user-actionable messages

Important: do not report `NetworkControl` ready only because OS is Windows/macOS/Linux. It must be ready only after an actual probe.

### PR-003: New `dek-deployment-planner` crate

Responsibilities:

- evaluate policy feasibility
- suggest policies from capability snapshot
- select best control method
- negotiate control level
- produce technical route for deployment

Non-responsibilities:

- no HTTP server
- no SQLite
- no React state
- no direct file writes

### PR-004: local-control-plane policy-first APIs

Add endpoints:

- scan
- latest capability snapshot
- policy suggestions
- feasibility preview
- deployment session create
- action approve
- retry/rollback

### PR-005: Dashboard Policy-first UX

Change UI hierarchy:

```text
Agents -> What POLLEK can do -> Recommended Policies -> Deploy Preview -> Timeline
```

Keep PEP/PDP in Advanced Details.

### PR-006: Deployment session state machine

States:

```text
scan_started
scan_completed
capability_snapshot_created
policy_feasibility_evaluated
user_selected_policy
deployment_plan_created
approval_required
bundle_created
bundle_activated
warm_check_passed
active
observe_only_active
partial_active
failed
rolled_back
```

### PR-007: Agent registration must preserve control surfaces

Ensure discovered candidates preserve:

- MCP stdio config path
- MCP HTTP URL
- local model endpoint
- browser extension evidence
- container/process evidence
- confidence
- suggested control methods

### PR-008: Secure telemetry and timeline integration

Every feasibility, deployment, enforcement, and observe event should:

- write local event store
- write secure spool
- include correlation ID
- include policy ID
- include agent/entity ID
- include internal technical detail for diagnostics

### PR-009: Desktop fallback E2E tests

Test scenarios:

1. Claude Desktop with MCP stdio: show approval-required enforceability.
2. Cursor with MCP HTTP: show enforce-now for tool policies.
3. Local Ollama HTTP: show observe/cost/rate policies.
4. Browser AI without extension: show observe-only or setup-required.
5. Windows without WFP: network block shows setup-required, not failure.
6. macOS NetworkExtension inactive: network block shows approval/setup-required.
7. Linux without eBPF permission: network block downgrades to observe-only.

## 21. Acceptance Criteria

POLLEK is ready for this flow when:

- A desktop user can deploy a policy without seeing the term PEP.
- Policy suggestions appear after local scan.
- Each suggestion states whether it can enforce, partially enforce, observe, or needs setup.
- Enforce is never shown as active until a warm check passes.
- A user can open Advanced Details to see selected PEP/PDP.
- Enterprise/server mode can still manually choose PEP/PDP.
- Capability registry does not overclaim readiness.
- Deployment events are grouped by agent/entity/session.
- Downgrades require user confirmation when requested control level cannot be met.
- Observe-only is explicit and not disguised as enforcement.

## 22. AI Agent Implementation Prompt

Use this prompt for the next development agent:

```text
Implement POLLEK's Policy-first, PEP-transparent desktop flow.

Do not ask normal desktop users to choose PEPs. Add a policy feasibility layer above route planning.

Required work:
1. Add PolicyIntent, ProductMode, PolicyFeasibilityStatus, PolicyFeasibilityRequest, PolicyFeasibilityResult, ControlMethod, ControlMethodPlan, InternalPep, InternalPdp to dek-domain-schema.
2. Add LocalCapabilitySnapshot and ControlMethodCapability to dek-capability-registry.
3. Create dek-deployment-planner as a pure crate for feasibility evaluation, policy suggestions, control method selection, and control level negotiation.
4. Update local-control-plane APIs:
   - POST /v1/local/scan
   - GET /v1/local/capability-snapshot/latest
   - GET /v1/policy-suggestions
   - POST /v1/policies/feasibility
   - POST /v1/deployment-sessions
5. Update dashboard UX:
   - show "What POLLEK can do"
   - show recommended policies
   - show feasibility preview
   - hide PEP/PDP in normal mode
   - expose PEP/PDP only in Advanced Details or Enterprise mode
6. Add tests:
   - MCP stdio -> approval required
   - MCP HTTP -> enforce now
   - no OS network control -> setup required or observe only
   - requested Enforce but only Observe available -> downgraded with confirmation

Keep dek-core focused on runtime enforcement and active bundle loading. Keep deployment orchestration in local-control-plane. Keep route planning reusable and UI-independent.
```

## 23. Deep Research References

Repository:

- https://github.com/AECInfraconnect/AntiG_Pollen_DEK

MCP:

- https://modelcontextprotocol.io/docs/getting-started/intro
- https://modelcontextprotocol.io/specification/2025-03-26/server/tools

Windows WFP:

- https://learn.microsoft.com/en-us/windows-hardware/drivers/network/introduction-to-windows-filtering-platform-callout-drivers
- https://learn.microsoft.com/en-us/samples/microsoft/windows-driver-samples/windows-filtering-platform-sample/

macOS NetworkExtension:

- https://developer.apple.com/documentation/networkextension
- https://developer.apple.com/documentation/networkextension/content-filter-providers

Policy engines:

- https://docs.cedarpolicy.com/
- https://openpolicyagent.org/docs/wasm
- https://openfga.dev/docs/interacting/relationship-queries

Observability:

- https://opentelemetry.io/blog/2024/otel-generative-ai/

