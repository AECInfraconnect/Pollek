import "@typespec/http";
import "@typespec/rest";
using TypeSpec.Http;
using TypeSpec.Rest;

namespace Pollen.Contract;

enum AiUsageEventKind {
  agent_run_started,
  agent_run_completed,
  agent_step_started,
  agent_step_completed,
  model_call_started,
  model_call_chunk,
  model_call_completed,
  tool_call_started,
  tool_call_completed,
  resource_access,
  budget_preflight,
  budget_alert,
  budget_blocked,
  usage_reconciled,
}

enum AiAgentType {
  local_agent,
  browser_ai,
  coding_agent,
  claude_code,
  codex_cli,
  cursor,
  mcp_client,
  mcp_server_agent,
  a2a_agent,
  managed_agent,
  gateway_agent,
  shadow_ai,
  unknown_value: "unknown",
}

enum AiUsageSource {
  provider_response,
  provider_streaming_final,
  provider_billing_export,
  sdk_instrumentation,
  proxy_observation,
  browser_estimate,
  local_tokenizer_estimate,
  manual_import,
  reconciled,
  unknown_value: "unknown",
}

enum AiCostSource {
  provider_reported,
  price_catalog_exact,
  price_catalog_tiered,
  estimated_tokenizer,
  billing_reconciled,
  unknown_value: "unknown",
}

model CanonicalTokenUsageV1 {
  input_tokens: int64;
  output_tokens: int64;
  total_tokens: int64;
  cached_input_tokens: int64;
  cache_write_input_tokens: int64;
  cache_write_input_tokens_5m?: int64;
  cache_write_input_tokens_1h?: int64;
  reasoning_output_tokens: int64;
  reasoning_input_tokens?: int64;
  tool_prompt_tokens: int64;
  tool_result_tokens: int64;
  image_input_tokens: int64;
  image_output_tokens: int64;
  audio_input_tokens: int64;
  audio_output_tokens: int64;
  video_input_tokens: int64;
  by_modality: Record<int64>;
  usage_details_ext: Record<int64>;
  estimated: boolean;
  source: AiUsageSource;
}

model CanonicalCostBreakdownV1 {
  currency: string;
  input_cost: float32;
  output_cost: float32;
  cached_input_cost: float32;
  cache_write_input_cost: float32;
  reasoning_output_cost: float32;
  tool_cost: float32;
  image_cost: float32;
  audio_cost: float32;
  total_cost: float32;
  price_catalog_version?: string;
  pricing_tier_id?: string;
  cost_source: AiCostSource;
  estimated: boolean;
  cost_details_ext: Record<float32>;
}

model AiUsageEventV1 {
  schema_version: "ai-usage-event.v1";
  event_id: string;
  event_kind: AiUsageEventKind;
  occurred_at: utcDateTime;
  received_at: utcDateTime;
  tenant_id: string;
  workspace_id?: string;
  device_id?: string;
  actor_id_hash?: string;
  actor_kind?: string;
  trace_id: string;
  span_id: string;
  parent_span_id?: string;
  session_id?: string;
  task_id?: string;
  agent_run_id?: string;
  agent_step_id?: string;
  invocation_id?: string;
  agent_id?: string;
  agent_instance_id?: string;
  agent_type: AiAgentType;
  parent_agent_id?: string;
  subagent_id?: string;
  shadow_candidate_id?: string;
  provider?: string;
  provider_api?: string;
  provider_request_id?: string;
  "model"?: string;
  model_version?: string;
  service_tier?: string;
  inference_region?: string;
  surface: string;
  pep_type?: string;
  control_mode?: string;
  policy_ids: string[];
  tokens: CanonicalTokenUsageV1;
  cost: CanonicalCostBreakdownV1;
  tool_id?: string;
  tool_name?: string;
  mcp_server_id?: string;
  resource_id?: string;
  resource_type?: string;
  latency_ms?: int64;
  status: string;
  error_code?: string;
  provider_usage_raw: Record<unknown>;
  metadata: Record<unknown>;
  local_sequence?: int64;
  cloud_sync_status?: string;
  idempotency_key: string;
}

model AiUsageEventPageV1 {
  schema_version: "ai-usage-event-page.v1";
  items: AiUsageEventV1[];
  next_cursor?: string;
}

model AiUsageLedgerV1 {
  schema_version: "ai-usage-ledger.v1";
  items: AiUsageEventV1[];
}

model AiUsageTotalsV1 {
  request_count: int64;
  input_tokens: int64;
  output_tokens: int64;
  cached_input_tokens: int64;
  cache_write_input_tokens: int64;
  reasoning_output_tokens: int64;
  tool_tokens: int64;
  multimodal_tokens: int64;
  total_tokens: int64;
  total_cost: float32;
}

model AiUsageBudgetStatusV1 {
  window: string;
  hard_cost_limit?: float32;
  hard_token_limit?: int64;
  remaining_cost?: float32;
  remaining_tokens?: int64;
  status: string;
}

model AiUsageBreakdownV1 {
  key: string;
  label: string;
  agent_type?: string;
  request_count: int64;
  total_tokens: int64;
  total_cost: float32;
  budget?: AiUsageBudgetStatusV1;
}

model AiUsageSeriesPointV1 {
  bucket_start: utcDateTime;
  request_count: int64;
  input_tokens: int64;
  output_tokens: int64;
  cached_input_tokens: int64;
  reasoning_output_tokens: int64;
  total_tokens: int64;
  total_cost: float32;
}

model AiBudgetLimitV1 {
  schema_version: "ai-budget-limit.v1";
  budget_id: string;
  tenant_id: string;
  scope_type: string;
  scope_id: string;
  window: string;
  currency: string;
  soft_cost_limit?: float32;
  hard_cost_limit?: float32;
  soft_token_limit?: int64;
  hard_token_limit?: int64;
  action_on_soft: string;
  action_on_hard: string;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

model AiBudgetLimitListV1 {
  schema_version: "ai-budget-limit-list.v1";
  items: AiBudgetLimitV1[];
}

model AiUsageSummaryV1 {
  schema_version: "ai-usage-summary.v1";
  tenant_id: string;
  from?: utcDateTime;
  to?: utcDateTime;
  bucket: string;
  currency: string;
  totals: AiUsageTotalsV1;
  by_agent: AiUsageBreakdownV1[];
  by_provider: AiUsageBreakdownV1[];
  by_model: AiUsageBreakdownV1[];
  series: AiUsageSeriesPointV1[];
  budgets: AiBudgetLimitV1[];
}

model AiUsageBatchIngestRequestV1 {
  events: AiUsageEventV1[];
}

model AiUsageBatchIngestResponseV1 {
  schema_version: "ai-usage-batch-ingest-response.v1";
  accepted: int64;
  rejected: int64;
}

model AiUsageReconcileResponseV1 {
  schema_version: "ai-usage-reconcile-response.v1";
  tenant_id: string;
  status: string;
  received: unknown;
}

model AiUsageQueryParams {
  @query from?: utcDateTime;
  @query to?: utcDateTime;
  @query agent_id?: string;
  @query agent_type?: string;
  @query provider?: string;
  @query "model"?: string;
  @query task_id?: string;
  @query session_id?: string;
  @query surface?: string;
  @query sync_status?: string;
  @query limit?: int64;
  @query cursor?: string;
}

model AiUsageSummaryQueryParams {
  @query from?: utcDateTime;
  @query to?: utcDateTime;
  @query bucket?: string;
  @query group_by?: string;
  @query agent_id?: string;
  @query agent_type?: string;
  @query provider?: string;
  @query "model"?: string;
  @query task_id?: string;
  @query session_id?: string;
  @query surface?: string;
}

model AiUsageAgentSummaryQueryParams {
  @query from?: utcDateTime;
  @query to?: utcDateTime;
  @query bucket?: string;
  @query group_by?: string;
  @query agent_type?: string;
  @query provider?: string;
  @query "model"?: string;
  @query task_id?: string;
  @query session_id?: string;
  @query surface?: string;
}

@tag("AI Usage")
@route("/v1/tenants/{tenant_id}/usage")
interface AiUsageApi {
  @doc("Ingest one canonical AI usage event")
  @route("events")
  @post ingestUsageEvent(@path tenant_id: string, ...PollenHeaders, @body body: AiUsageEventV1): { item: AiUsageEventV1 } | PollenError;

  @doc("Ingest canonical AI usage events in batch")
  @route("events/batch")
  @post ingestUsageEventsBatch(@path tenant_id: string, ...PollenHeaders, @body body: AiUsageBatchIngestRequestV1): AiUsageBatchIngestResponseV1 | PollenError;

  @doc("List canonical AI usage events")
  @route("events")
  @get listUsageEvents(@path tenant_id: string, ...PollenHeaders, ...AiUsageQueryParams): AiUsageEventPageV1 | PollenError;

  @doc("Get canonical AI usage summary")
  @route("summary")
  @get getUsageSummary(@path tenant_id: string, ...PollenHeaders, ...AiUsageSummaryQueryParams): AiUsageSummaryV1 | PollenError;

  @doc("Get canonical AI usage summary for one agent")
  @route("agents/{agent_id}")
  @get getAgentUsageSummary(@path tenant_id: string, @path agent_id: string, ...PollenHeaders, ...AiUsageAgentSummaryQueryParams): AiUsageSummaryV1 | PollenError;

  @doc("List canonical AI usage ledger events")
  @route("ledger")
  @get getUsageLedger(@path tenant_id: string, ...PollenHeaders, ...AiUsageQueryParams): AiUsageLedgerV1 | PollenError;

  @doc("List AI budget policies")
  @route("budgets")
  @get listUsageBudgets(@path tenant_id: string, ...PollenHeaders): AiBudgetLimitListV1 | PollenError;

  @doc("Upsert an AI budget policy")
  @route("budgets/{budget_id}")
  @put upsertUsageBudget(@path tenant_id: string, @path budget_id: string, ...PollenHeaders, @body body: AiBudgetLimitV1): { item: AiBudgetLimitV1 } | PollenError;

  @doc("Accept provider billing reconciliation metadata")
  @route("reconcile")
  @post reconcileUsage(@path tenant_id: string, ...PollenHeaders, @body body: unknown): AiUsageReconcileResponseV1 | PollenError;
}
