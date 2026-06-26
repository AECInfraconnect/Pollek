import "@typespec/http";
using TypeSpec.Http;
namespace Pollen.Contract;

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

enum TelemetryEventType { agent_observation, enforcement_result, control_binding_changed, health }

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

@tag("Telemetry")
interface TelemetryApi {
  @route("/v1/telemetry/batches") @post
  ingestBatch(...PollenHeaders, @body body: TelemetryBatchRequest): TelemetryIngestResponse | PollenError;

  @route("/v1/telemetry/observations") @get
  listObservations(...PollenHeaders, ...ObservationQuery): ObservationPage | PollenError;

  @route("/v1/telemetry/enforcement-status") @get
  enforcementStatus(...PollenHeaders, @query agent_id?: string): EnforcementStatusList | PollenError;
}
