import "@typespec/http";
using TypeSpec.Http;
namespace Pollek.Contract;

model TelemetryEnvelope<T> {
  schema_version: "telemetry-envelope.v1";
  event_id: string;
  event_type: TelemetryEventType;
  timestamp: utcDateTime;
  tenant_id: string;
  workspace_id?: string;
  environment_id?: string;
  device_id: string;
  trace_id?: string;
  span_id?: string;
  redaction_applied: boolean;
  payload: T;
}

enum TelemetryEventType { agent_observation, enforcement_result, control_binding_changed, health, resource_access, tool_usage, identity_access }

enum ControlLevel { observe, warn, ask, enforce }
enum ControlDomain { network, file_system, process, mcp_tool, dns }
enum ControlMethod {
  mcp_stdio, mcp_http,
  linux_landlock, linux_ebpf, linux_seccomp,
  windows_wfp_um, windows_wfp_callout, windows_etw,
  macos_netext, macos_endpoint_security,
}
enum DecisionOutcome { allow, deny, redact, warn, observed }
enum EnforcePlaneState { enforcing, observing, degraded, failed }

enum ResourceKind { file, folder, database_local, os_syscall, config, process, clipboard, web, email, cloud_drive, database_cloud, data_source, api, saas }
enum ToolKind { mcp_tool, function_call, http_api, a2a_skill, shell_command, browser_action }
enum AccessMode { read, write, execute, delete, connect, invoke }
enum AccessScope { local, cloud }
enum IdentityKind { user, service_account, workload, device, oauth_client, spiffe_id, api_key, browser_profile, cloud_account }
enum IdentityAction { authenticate, authorize, assume, delegate, token_issue, token_refresh, access }

model ResourceTraceDetails {
  trace_source?: string;
  capture_quality?: string;
  trace_granularity?: string;
  raw_content_stored?: boolean;
  source_path_hash?: string;
  source_path_redacted?: string;
  path_kind?: string;
  resource_name?: string;
  file_name?: string;
  file_extension?: string;
  folder_name?: string;
  folder_path?: string;
  host?: string;
  port?: int32;
  db_system?: string;
  db_namespace?: string;
  db_table?: string;
  db_collection?: string;
  db_operation?: string;
  query_summary?: string;
  query_fingerprint?: string;
  os_observer?: string;
  control_method?: ControlMethod;
  limitations?: string[];
}

model AgentObservationPayload {
  agent_id: string;
  agent_label: string;
  domain: ControlDomain;
  action: string;
  target?: string;
  decision: DecisionOutcome;
  control_method?: ControlMethod;
  enforced_for_real: boolean;
  reason_th?: string; reason_en?: string;
}

model EnforcementResultPayload {
  agent_id: string;
  policy_id: string;
  control_method: ControlMethod;
  domain: ControlDomain;
  requested_level: ControlLevel;
  effective_level: ControlLevel;
  plane_state: EnforcePlaneState;
  success: boolean;
  os: string;
  message_th?: string; message_en?: string;
  user_action_th?: string; user_action_en?: string;
}

model ControlBindingChangedPayload {
  agent_id: string; policy_id: string;
  bindings: { domain: ControlDomain, method: ControlMethod, level: ControlLevel }[];
}

model TelemetryBatchRequest {
  schema_version: "telemetry-batch.v1";
  tenant_id: string; device_id: string; batch_id: string;
  events: TelemetryEnvelope<unknown>[];
}

model TelemetryIngestResponse {
  schema_version: "telemetry-ingest-response.v1";
  accepted: int32; rejected: int32; retry_after_seconds?: int32;
}

model ObservationQuery { agent_id?: string; since?: utcDateTime; limit?: int32; }

model ObservationPage {
  schema_version: "observation-page.v1";
  items: TelemetryEnvelope<AgentObservationPayload>[];
  next_cursor?: string;
}

model EnforcementStatusList {
  schema_version: "enforcement-status.v1";
  items: TelemetryEnvelope<EnforcementResultPayload>[];
}

model ResourceAccessPayload {
  agent_id: string; agent_label: string;
  scope: AccessScope; kind: ResourceKind;
  target_redacted: string; target_hash: string;
  mode: AccessMode;
  decision: DecisionOutcome;
  control_method?: ControlMethod;
  enforced_for_real: boolean;
  bytes?: int64; count?: int32; classification?: string;
  details?: ResourceTraceDetails;
  observed_at: utcDateTime;
}

model ToolUsagePayload {
  agent_id: string; agent_label: string;
  tool_kind: ToolKind; tool_name: string; server?: string;
  decision: DecisionOutcome; enforced_for_real: boolean;
  args_redacted?: string; observed_at: utcDateTime;
}

model IdentityAccessPayload {
  agent_id: string; agent_label: string;
  scope: AccessScope; identity_kind: IdentityKind;
  identity_id: string; identity_label: string; provider?: string; spiffe_id?: string;
  action: IdentityAction;
  decision: DecisionOutcome;
  control_method?: ControlMethod;
  enforced_for_real: boolean;
  observed_at: utcDateTime;
}

model ObservedResource {
  resource_id: string; scope: AccessScope; kind: ResourceKind;
  target_redacted: string; classification?: string;
  details?: ResourceTraceDetails;
  agents: string[]; modes: AccessMode[];
  last_access: utcDateTime; access_count: int64;
  governed: boolean; registered: boolean;
}

model ObservedTool {
  tool_id: string; tool_kind: ToolKind; tool_name: string; server?: string;
  agents: string[]; last_used: utcDateTime; use_count: int64;
  governed: boolean; registered: boolean;
}

model ObservedIdentity {
  identity_id: string; identity_label: string; identity_kind: IdentityKind;
  scope: AccessScope; provider?: string; spiffe_id?: string;
  agents: string[]; actions: IdentityAction[];
  last_seen: utcDateTime; access_count: int64;
  governed: boolean; registered: boolean;
}

model ResourceInventoryPage { schema_version: "resource-inventory.v1"; items: ObservedResource[]; next_cursor?: string; }
model ToolInventoryPage     { schema_version: "tool-inventory.v1";     items: ObservedTool[];     next_cursor?: string; }
model IdentityInventoryPage { schema_version: "identity-inventory.v1"; items: ObservedIdentity[]; next_cursor?: string; }

@tag("Telemetry")
interface TelemetryApi {
  @route("/v1/telemetry/batches") @post
  ingestBatch(...PollekHeaders, @body body: TelemetryBatchRequest): TelemetryIngestResponse | PollekError;

  @route("/v1/telemetry/observations") @get
  listObservations(...PollekHeaders, ...ObservationQuery): ObservationPage | PollekError;

  @route("/v1/telemetry/enforcement-status") @get
  enforcementStatus(...PollekHeaders, @query agent_id?: string): EnforcementStatusList | PollekError;

  @route("/v1/telemetry/resources") @get
  resources(...PollekHeaders, @query agent_id?: string, @query scope?: AccessScope): ResourceInventoryPage | PollekError;

  @route("/v1/telemetry/tools") @get
  tools(...PollekHeaders, @query agent_id?: string): ToolInventoryPage | PollekError;

  @route("/v1/telemetry/identities") @get
  identities(...PollekHeaders, @query agent_id?: string, @query scope?: AccessScope): IdentityInventoryPage | PollekError;
}
