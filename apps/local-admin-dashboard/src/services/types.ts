// Pollen DEK Registry API Models

export interface ObjectMeta {
  schema_version: string;
  tenant_id: string;
  workspace_id: string;
  environment_id: string;
  created_at: string;
  updated_at: string;
  created_by: string;
  updated_by: string;
  source:
    | "manual"
    | "discovery"
    | "import"
    | "cloud_sync"
    | "agent_self_registration";
  status:
    | "discovered"
    | "pending_approval"
    | "registered"
    | "active"
    | "suspended"
    | "deleted"
    | "draft"
    | "published"
    | "compiled";
  tags: string[];
}

export interface AiAgent {
  meta: ObjectMeta;
  agent_id: string;
  name: string;
  agent_type:
    | "claude_desktop"
    | "openai_agent"
    | "langchain_agent"
    | "llama_index_agent"
    | "custom_mcp_client"
    | "browser_agent"
    | "cli_agent"
    | "unknown";
  vendor?: string;
  runtime: {
    runtime_name: string;
    version?: string;
  };
  entrypoints: {
    command: string;
    args: string[];
  }[];
  declared_tools: string[];
  declared_resources: string[];
  identity: {
    spiffe_id?: string;
    process_path?: string;
    user_subject?: string;
    signing_key_fingerprint?: string;
  };
  trust_level: "untrusted" | "low" | "medium" | "high" | "system";
  capabilities: string[];
  labels: Record<string, string>;
}

export interface McpServer {
  meta: ObjectMeta;
  server_id: string;
  name: string;
  transport: "stdio" | "http" | "sse" | "web_socket";
  endpoint: string;
  owner_agent_id?: string;
  tools: string[];
  resources: string[];
  risk_level: "low" | "medium" | "high" | "critical";
}

import type { components } from "../../../../contracts/generated/typescript/api";

export type Tool = components["schemas"]["Tool"];
export type Resource = components["schemas"]["Resource"];
export type AgentObservationEvent =
  components["schemas"]["AgentObservationEvent"];
export type CostLedgerEntry = components["schemas"]["CostLedgerEntry"];
export type PolicySuggestion = components["schemas"]["PolicySuggestion"];
export type DiscoveryCandidate = components["schemas"]["DiscoveryCandidate"];

export interface ConnectorConfig {
  id: string;
  kind?: string;
  endpoint?: string;
  health_interval_secs?: number;
  mtls_enabled?: boolean;
  [key: string]: any;
}

export interface SimulationRequest {
  action: string;
  resource: string;
  principal: string;
  context: any;
  target_pep?: string;
}

export interface SimulationResult {
  passed?: boolean;
  decision?: string;
  logs?: string[];
  [key: string]: any;
}

export interface Entity {
  meta: ObjectMeta;
  entity_id: string;
  entity_type:
    | "human_user"
    | "service_account"
    | "workload"
    | "ai_agent"
    | "organization"
    | "tenant"
    | "device";
  display_name: string;
  external_ids: { provider: string; id: string }[];
  roles: string[];
  attributes: Record<string, any>;
}

export interface Relationship {
  meta: ObjectMeta;
  relationship_id: string;
  subject: { object_type: string; object_id: string };
  relation: string;
  object: { object_type: string; object_id: string };
  conditions?: any;
}

// ---- Policy ----
export type PolicyType =
  | "rego"
  | "cedar"
  | "open_fga"
  | "pii_redaction"
  | "route"
  | "composite";

export type PolicyLifecycleStatus =
  | "draft"
  | "validated"
  | "simulation_passed"
  | "compiled"
  | "pending_approval"
  | "approved"
  | "published"
  | "active";

export interface PolicyTargets {
  agent_ids: string[];
  tool_ids: string[];
  resource_ids: string[];
  entity_ids: string[];
  route_ids: string[];
}

export type PolicySource =
  | { kind: "raw_text"; language: string; text: string }
  | { kind: "template"; template_id: string; params: any }
  | { kind: "structured"; ir: any };

export interface PolicyDraft {
  meta: ObjectMeta;
  policy_id: string;
  name: string;
  description?: string;
  policy_type: PolicyType;
  targets: PolicyTargets;
  source: PolicySource;
  compile_options: { optimization_level?: string; fail_on_warnings?: boolean };
}

// ---- Telemetry / Decision logs ----
export type TelemetryEventType =
  | "decision_log"
  | "policy_bundle_activated"
  | "policy_bundle_rejected"
  | "runtime_metric"
  | "security_event"
  | "pii_redaction_event"
  | "adapter_health"
  | "sync_health"
  | "os_guardrail_event";

export type DecisionEffect =
  | "allow"
  | "deny"
  | "redact"
  | "mask"
  | "warn"
  | "require_approval"
  | "break_glass_allow";

export interface TelemetryEventEnvelope {
  schema_version: string;
  event_id: string;
  event_type: TelemetryEventType;
  timestamp: string;
  tenant_id: string;
  workspace_id: string;
  environment_id: string;
  device_id: string;
  trace_id?: string;
  span_id?: string;
  payload: any; // DecisionResult for decision_log
  redaction_applied: boolean;
}

export interface DecisionResult {
  request_id: string;
  trace_id: string;
  decision: DecisionEffect;
  reason: string;
  matched_policy_ids: string[];
  matched_route_id?: string;
  adapter_results: {
    adapter_id: string;
    decision: DecisionEffect;
    reason?: string;
  }[];
  obligations: { obligation_type: string; fields: string[] }[];
  latency_ms: number;
}

export interface BlackboxAiProvider {
  meta: ObjectMeta;
  provider_id: string;
  name: string;
  provider_type: "openai" | "anthropic" | "google" | "azure_openai" | "custom";
  api_base_url?: string;
  auth_mechanism: {
    type: "api_key" | "bearer_token" | "mtls" | "none";
    secret_reference?: string;
  };
  supported_models: string[];
  default_model?: string;
  rate_limits: {
    requests_per_minute?: number;
    tokens_per_minute?: number;
  };
  trust_level: "untrusted" | "low" | "medium" | "high" | "system";
  data_processing_agreement_signed: boolean;
  requires_pii_redaction: boolean;
}

// ---- PDP Runtime & Routing ----
export type PdpRuntimeCategory =
  | "local_engine"
  | "external_connector"
  | "pollen_cloud";
export type PdpKind =
  | "opa_server"
  | "opa_wasm"
  | "cedar_http"
  | "cedar_local"
  | "openfga_server"
  | "wasm_plugin"
  | "custom_http";
export type PdpStatus =
  | "initializing"
  | "ready"
  | "unreachable"
  | "degraded"
  | "disabled";

export interface PdpRuntime {
  id: string;
  name: string;
  category: PdpRuntimeCategory;
  kind: PdpKind;
  enabled: boolean;
  status: PdpStatus;
  endpoint?: string;
  auth_ref?: string;
  capabilities: string[];
  health?: any;
  created_at: string;
  updated_at: string;
}

export type PdpRouteMode =
  | "local_primary_remote_fallback"
  | "remote_primary_local_fallback"
  | "load_balanced"
  | "shadow"
  | "first_match_wins"
  | "broadcast";
export type PdpFailureBehavior = "deny" | "allow" | "passthrough";

export interface RouteMatch {
  agent_ids?: string[];
  resource_ids?: string[];
  protocols?: string[];
  policy_tags?: string[];
  sensitivity?: string;
  environment?: string;
}

export interface PdpRouteRule {
  id: string;
  name: string;
  enabled: boolean;
  priority: number;
  match_cond: RouteMatch;
  mode: PdpRouteMode;
  primary_pdp_id: string;
  fallback_pdp_ids: string[];
  shadow_pdp_ids: string[];
  merge_strategy: string;
  failure_behavior: PdpFailureBehavior;
  timeout_ms: number;
  max_retries: number;
}

export interface DiscoveryEvidenceV2 {
  evidence_id: string;
  source: string;
  confidence: number;
  observed_at: string;
  privacy_class: string;
  redacted: boolean;
  data: any;
  merge_key?: string;
  source_path_hash?: string;
  source_path_redacted?: string;
}

export interface ControlBindingPlan {
  binding_id: string;
  kind: string;
  target_candidate_id: string;
  target_config_hash?: string;
  action: string;
  requires_user_approval: boolean;
  risk: string;
  reversible: boolean;
  backup_path_hash?: string;
  summary: string;
}

export interface DiscoveredAgentCandidateV2 {
  schema_version: string;
  candidate_id: string;
  tenant_id: string;
  device_id: string;
  status: string;
  display_name: string;
  vendor?: string;
  product?: string;
  inferred_agent_type: string;
  confidence: number;
  risk_score: number;
  first_seen: string;
  last_seen: string;
  evidence: DiscoveryEvidenceV2[];
  discovered_configs: any[];
  discovered_endpoints: any[];
  discovered_mcp_servers: any[];
  suggested_registration: any;
  suggested_observation_profile: any;
  suggested_control_bindings: ControlBindingPlan[];
  telemetry_plan: any;
  labels: Record<string, string>;
}

export interface DiscoveryScanJob {
  scan_id: string;
  tenant_id: string;
  status:
    | "queued"
    | "running"
    | "completed"
    | "partial"
    | "failed"
    | "cancelled";
  started_at?: string;
  finished_at?: string;
  sources: string[];
  error?: string;
  candidates_found: number;
}
