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

model PolicyArtifact {
  name: string;
  content: string;
  language: string;
}

model PolicySuggestion {
  suggestion_id: string;
  tenant_id: string;
  target_agent_id?: string;
  target_resource_id?: string;
  target_tool_id?: string;
  suggestion_type: string;
  title: string;
  summary: string;
  severity: string;
  confidence: float32;
  recommended_policy_type: string;
  recommended_pep_type: string;
  artifacts: PolicyArtifact[];
  status: string;
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

model CostLedgerListResponse {
  items: CostLedgerEntry[];
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
  @get listTools(@path tenant_id: string): ToolListResponse | PollenError;
  @post createTool(@path tenant_id: string, @body body: Tool): Tool | PollenError;
}

@tag("Registry")
@route("/v1/tenants/{tenant_id}/registry/resources")
interface ResourcesApi {
  @get listResources(@path tenant_id: string): ResourceListResponse | PollenError;
  @post createResource(@path tenant_id: string, @body body: Resource): Resource | PollenError;
}

@tag("Observation")
@route("/v1/tenants/{tenant_id}/observations")
interface ObservationApi {
  @route("events")
  @post ingestEvents(@path tenant_id: string, @body body: AgentObservationEvent[]): void | PollenError;

  @route("agents/{agent_id}")
  @get getAgentObservations(@path tenant_id: string, @path agent_id: string): AgentObservationEventListResponse | PollenError;

  @route("tools/{tool_id}")
  @get getToolObservations(@path tenant_id: string, @path tool_id: string): AgentObservationEventListResponse | PollenError;

  @route("resources/{resource_id}")
  @get getResourceObservations(@path tenant_id: string, @path resource_id: string): AgentObservationEventListResponse | PollenError;

  @route("costs")
  @get getCosts(@path tenant_id: string): CostLedgerListResponse | PollenError;

  @route("coverage")
  @get getCoverage(@path tenant_id: string): PolicyCoverageSummary | PollenError;
}

@tag("Discovery")
@route("/v1/tenants/{tenant_id}/agent-discovery")
interface DiscoveryApi {
  @route("scan")
  @post scan(@path tenant_id: string): void | PollenError;

  @route("candidates")
  @get listCandidates(@path tenant_id: string): DiscoveryCandidateListResponse | PollenError;

  @route("candidates/{candidate_id}/register")
  @post registerCandidate(@path tenant_id: string, @path candidate_id: string): void | PollenError;

  @route("candidates/{candidate_id}/ignore")
  @post ignoreCandidate(@path tenant_id: string, @path candidate_id: string): void | PollenError;

  @route("candidates/{candidate_id}/observe-only")
  @post observeCandidate(@path tenant_id: string, @path candidate_id: string): void | PollenError;

  @route("candidates/{candidate_id}/block")
  @post blockCandidate(@path tenant_id: string, @path candidate_id: string): void | PollenError;
}

@tag("PolicySuggestions")
@route("/v1/tenants/{tenant_id}/policy-suggestions")
interface PolicySuggestionsApi {
  @route("generate")
  @post generate(@path tenant_id: string): void | PollenError;

  @get listSuggestions(@path tenant_id: string): PolicySuggestionListResponse | PollenError;

  @route("{suggestion_id}")
  @get getSuggestion(@path tenant_id: string, @path suggestion_id: string): PolicySuggestion | PollenError;

  @route("{suggestion_id}/create-draft")
  @post createDraft(@path tenant_id: string, @path suggestion_id: string): void | PollenError;

  @route("{suggestion_id}/dismiss")
  @post dismiss(@path tenant_id: string, @path suggestion_id: string): void | PollenError;

  @route("{suggestion_id}/simulate")
  @post simulate(@path tenant_id: string, @path suggestion_id: string): void | PollenError;
}
