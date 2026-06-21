import "@typespec/http";
import "@typespec/rest";
using TypeSpec.Http;
using TypeSpec.Rest;

namespace Pollen.Contract;

model ObjectMeta {
  schema_version: string;
  tenant_id: string;
  workspace_id: string;
  environment_id: string;
  created_at: string;
  updated_at: string;
  created_by: string;
  updated_by: string;
  source: string;
  status: string;
  tags: string[];
}

model TokenUsage {
  input_tokens?: int64;
  output_tokens?: int64;
  total_tokens?: int64;
  "model"?: string;
}

model AgentObservationEvent {
  event_id: string;
  tenant_id: string;
  trace_id: string;
  agent_id?: string;
  shadow_candidate_id?: string;
  tool_id?: string;
  resource_id?: string;
  surface: string;
  action: string;
  pep_type?: string;
  risk_level?: string;
  timestamp: string;
  payload_json: string;
  token_usage?: TokenUsage;
}

enum SuggestionType {
  RestrictMcpTool,
  RequireApprovalForSensitiveResource,
  RegisterShadowAgent,
  EnforceCostBudget,
  RestrictExternalLlmProvider,
}

enum SuggestedPolicyLanguage {
  Rego,
  Cedar,
  OpenFga,
}

enum SuggestionSeverity {
  Low,
  Medium,
  High,
  Critical,
}

enum SuggestionStatus {
  Draft,
  Suggested,
  Approved,
  Dismissed,
  Applied,
}

model PolicyArtifact {
  name: string;
  content: string;
  language: SuggestedPolicyLanguage;
}

model PolicySuggestion {
  suggestion_id: string;
  tenant_id: string;
  target_agent_id?: string;
  target_resource_id?: string;
  target_tool_id?: string;
  suggestion_type: SuggestionType;
  title: string;
  summary: string;
  severity: SuggestionSeverity;
  confidence: float32;
  recommended_policy_type: SuggestedPolicyLanguage;
  recommended_pep_type: string;
  artifacts: PolicyArtifact[];
  status: SuggestionStatus;
  created_at: string;
}

model DiscoveryCandidate {
  candidate_id: string;
  tenant_id: string;
  surface: string;
  first_seen_at: string;
  last_seen_at: string;
  observation_count: int64;
  inferred_name?: string;
  inferred_provider?: string;
  status: string;
}

model AgentObservationEventListResponse {
  items: AgentObservationEvent[];
}

model CostLedgerEntry {
  event_id: string;
  agent_id: string;
  provider: string;
  "model"?: string;
  input_tokens: int64;
  output_tokens: int64;
  total_tokens: int64;
  input_cost: float32;
  output_cost: float32;
  total_cost: float32;
  currency: string;
  estimated: boolean;
  timestamp: string;
}

model AgentTrustScore {
  schema_version: "agent-trust-score.v1";
  agent_id: string;
  score: float32;
  reasons: string[];
}

model BudgetPolicy {
  schema_version: "budget-policy.v1";
  agent_id: string;
  daily_cost_cap_usd: float32;
  daily_token_cap: int64;
}

model SandboxConfig {
  schema_version: "sandbox-config.v1";
  timeout_ms: int64;
  max_memory_mb: int32;
  enable_network: boolean;
}

model CostLedgerListResponse {
  items: CostLedgerEntry[];
}

model CostSummaryResponse {
  schema_version: "cost-summary.v1";
  tenant_id: string;
  period: string;
  total_estimated_cost_usd: float32;
  total_tokens: int64;
  provider_breakdown: Record<float32>;
  agent_breakdown: Record<float32>;
}

model PolicySuggestionListResponse {
  items: PolicySuggestion[];
}

model DiscoveryCandidateListResponse {
  items: DiscoveryCandidate[];
}

model PolicyCoverageSummary {
  status: string;
  policy_ids: string[];
  missing_controls: MissingControl[];
  last_simulated_at?: string;
  last_enforced_at?: string;
  confidence: float32;
}

model MissingControl {
  control_type: string;
  reason: string;
  recommended_policy_type: string;
  recommended_pep_type: string;
}

model PepBinding {
  pep_type: string;
  deployment_status: string;
  rule_ids: string[];
  capabilities: string[];
  limitations: string[];
}

model SchemaFingerprint {
  input_schema_hash: string;
  output_schema_hash?: string;
  descriptor_hash?: string;
  previous_descriptor_hash?: string;
  drift_status: string;
}

model Tool {
  meta: ObjectMeta;
  tool_id: string;
  mcp_server_id?: string;
  name: string;
  description?: string;
  input_schema: unknown;
  output_schema?: unknown;
  side_effect_level: string;
  data_access_level: string;
  risk_level: string;
  category: string;
  source: string;
  declared_by_agent_ids: string[];
  observed_by_agent_ids: string[];
  reachable_resource_ids: string[];
  required_entitlements: string[];
  schema_fingerprint: SchemaFingerprint;
  policy_coverage: PolicyCoverageSummary;
  pep_bindings: PepBinding[];
  last_seen_at?: string;
  first_seen_at?: string;
  observation_count_24h: int64;
  deny_count_24h: int64;
  allow_count_24h: int64;
}

model Resource {
  meta: ObjectMeta;
  resource_id: string;
  resource_type: string;
  name: string;
  uri: string;
  classification: string;
  owner_entity_id?: string;
  attributes: Record<unknown>;
  data_boundary: string;
  data_tags: string[];
  pii_types: string[];
  secret_types: string[];
  allowed_actions: string[];
  observed_actions: string[];
  observed_by_agent_ids: string[];
  reachable_via_tool_ids: string[];
  policy_coverage: PolicyCoverageSummary;
  pep_bindings: PepBinding[];
  first_seen_at?: string;
  last_seen_at?: string;
  access_count_24h: int64;
  violation_count_24h: int64;
  cost_relevant: boolean;
}

model ToolListResponse {
  items: Tool[];
}

model ResourceListResponse {
  items: Resource[];
}

@tag("Registry")
@route("/v1/tenants/{tenant_id}/registry/tools")
interface ToolsApi {
  @doc("List tools")
  @get listTools(@path tenant_id: string, ...PollenHeaders): ToolListResponse | PollenError;

  @doc("Create a tool")
  @post createTool(@path tenant_id: string, ...PollenHeaders, @body body: Tool): Tool | PollenError;
}

@tag("Registry")
@route("/v1/tenants/{tenant_id}/registry/resources")
interface ResourcesApi {
  @doc("List resources")
  @get listResources(@path tenant_id: string, ...PollenHeaders): ResourceListResponse | PollenError;

  @doc("Create a resource")
  @post createResource(@path tenant_id: string, ...PollenHeaders, @body body: Resource): Resource | PollenError;
}

@tag("Observation")
@route("/v1/tenants/{tenant_id}/observations")
interface ObservationApi {
  @doc("Ingest observation events")
  @route("events")
  @post ingestEvents(@path tenant_id: string, ...PollenHeaders, @body body: AgentObservationEvent[]): void | PollenError;

  @doc("Get observations for an agent")
  @route("agents/{agent_id}")
  @get getAgentObservations(@path tenant_id: string, @path agent_id: string, ...PollenHeaders): AgentObservationEventListResponse | PollenError;

  @doc("Get observations for a tool")
  @route("tools/{tool_id}")
  @get getToolObservations(@path tenant_id: string, @path tool_id: string, ...PollenHeaders): AgentObservationEventListResponse | PollenError;

  @doc("Get observations for a resource")
  @route("resources/{resource_id}")
  @get getResourceObservations(@path tenant_id: string, @path resource_id: string, ...PollenHeaders): AgentObservationEventListResponse | PollenError;

  @doc("Get cost summary")
  @route("costs")
  @get getCosts(@path tenant_id: string, ...PollenHeaders): CostSummaryResponse | PollenError;

  @doc("Get policy coverage summary")
  @route("coverage")
  @get getCoverage(@path tenant_id: string, ...PollenHeaders): PolicyCoverageSummary | PollenError;

  @doc("Get trust score for an agent")
  @route("trust/{agent_id}")
  @get getAgentTrustScore(@path tenant_id: string, @path agent_id: string, ...PollenHeaders): AgentTrustScore | PollenError;
}

@tag("Discovery")
@route("/v1/tenants/{tenant_id}/agent-discovery")
interface DiscoveryApi {
  @doc("Start a discovery scan")
  @route("scan")
  @post scan(@path tenant_id: string, ...PollenHeaders): void | PollenError;

  @doc("List discovery candidates")
  @route("candidates")
  @get listCandidates(@path tenant_id: string, ...PollenHeaders): DiscoveryCandidateListResponse | PollenError;

  @doc("Register a candidate")
  @route("candidates/{candidate_id}/register")
  @post registerCandidate(@path tenant_id: string, @path candidate_id: string, ...PollenHeaders): void | PollenError;

  @doc("Ignore a candidate")
  @route("candidates/{candidate_id}/ignore")
  @post ignoreCandidate(@path tenant_id: string, @path candidate_id: string, ...PollenHeaders): void | PollenError;

  @doc("Observe a candidate only")
  @route("candidates/{candidate_id}/observe-only")
  @post observeCandidate(@path tenant_id: string, @path candidate_id: string, ...PollenHeaders): void | PollenError;

  @doc("Block a candidate")
  @route("candidates/{candidate_id}/block")
  @post blockCandidate(@path tenant_id: string, @path candidate_id: string, ...PollenHeaders): void | PollenError;
}

@tag("PolicySuggestions")
@route("/v1/tenants/{tenant_id}/policy-suggestions")
interface PolicySuggestionsApi {
  @doc("Generate policy suggestions")
  @route("generate")
  @post generate(@path tenant_id: string, ...PollenHeaders): void | PollenError;

  @doc("List policy suggestions")
  @get listSuggestions(@path tenant_id: string, ...PollenHeaders): PolicySuggestionListResponse | PollenError;

  @doc("Get a specific policy suggestion")
  @route("{suggestion_id}")
  @get getSuggestion(@path tenant_id: string, @path suggestion_id: string, ...PollenHeaders): PolicySuggestion | PollenError;

  @doc("Create a draft from a suggestion")
  @route("{suggestion_id}/create-draft")
  @post createDraft(@path tenant_id: string, @path suggestion_id: string, ...PollenHeaders): void | PollenError;

  @doc("Dismiss a policy suggestion")
  @route("{suggestion_id}/dismiss")
  @post dismiss(@path tenant_id: string, @path suggestion_id: string, ...PollenHeaders): void | PollenError;

  @doc("Simulate a policy suggestion")
  @route("{suggestion_id}/simulate")
  @post simulate(@path tenant_id: string, @path suggestion_id: string, ...PollenHeaders): void | PollenError;

  @doc("Approve a policy suggestion")
  @route("{suggestion_id}/approve")
  @post approve(@path tenant_id: string, @path suggestion_id: string, ...PollenHeaders): void | PollenError;
}

@tag("PolicyPresets")
@route("/v1/tenants/{tenant_id}/policy-presets")
interface PolicyPresetsApi {
  @doc("List policy presets")
  @get listPresets(@path tenant_id: string, ...PollenHeaders): unknown | PollenError;

  @doc("Get a specific policy preset")
  @route("{preset_id}")
  @get getPreset(@path tenant_id: string, @path preset_id: string, ...PollenHeaders): unknown | PollenError;

  @doc("Preview a policy preset")
  @route("{preset_id}/preview")
  @post previewPreset(@path tenant_id: string, @path preset_id: string, ...PollenHeaders, @body body: unknown): unknown | PollenError;

  @doc("Create a draft from a preset")
  @route("{preset_id}/create-draft")
  @post createDraft(@path tenant_id: string, @path preset_id: string, ...PollenHeaders, @body body: unknown): unknown | PollenError;

  @doc("Simulate a policy preset")
  @route("{preset_id}/simulate")
  @post simulatePreset(@path tenant_id: string, @path preset_id: string, ...PollenHeaders, @body body: unknown): unknown | PollenError;
}

@tag("PepCapabilities")
@route("/v1/tenants/{tenant_id}/pep-capabilities")
interface PepCapabilitiesApi {
  @doc("List PEP capabilities")
  @get listCapabilities(@path tenant_id: string, ...PollenHeaders): unknown | PollenError;

  @doc("Check PEP capabilities")
  @route("check")
  @post checkCapabilities(@path tenant_id: string, ...PollenHeaders, @body body: unknown): unknown | PollenError;
}
