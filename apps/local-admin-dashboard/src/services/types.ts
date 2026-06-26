// Pollek Local Enforcement Kit Registry API Models

import type { components } from "../../../../contracts/generated/typescript/api";

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
    token_bindings?: {
      kind:
        | "oauth_access_token"
        | "oidc_id_token"
        | "oauth_refresh_token"
        | "jwt_svid"
        | "x509_svid"
        | "api_key_reference";
      provider: string;
      issuer?: string;
      subject?: string;
      audience: string[];
      scopes: string[];
      confirmation:
        | "mtls_certificate"
        | "dpop_key"
        | "spiffe_svid"
        | "none";
      token_hash?: string;
      expires_at?: string;
      last_rotated_at?: string;
    }[];
  };
  trust_level: "untrusted" | "low" | "medium" | "high" | "system";
  capabilities: string[];
  labels: Record<string, string>;
  enforcement_mode?: "Observe" | "Enforce" | "Shadow" | "NotEnforceable";
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

export type Tool = components["schemas"]["Tool"];
export type Resource = components["schemas"]["Resource"];
export type AgentObservationEvent =
  components["schemas"]["AgentObservationEvent"];
export type CostLedgerEntry = components["schemas"]["CostLedgerEntry"];
export type LegacyPolicySuggestion = components["schemas"]["PolicySuggestion"];
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
  | "remote_connector"
  | "pollen_cloud";

export type PdpKind =
  | "policy_router"
  | "cedar_local"
  | "opa_wasm"
  | "wasm_plugin"
  | "opa_server"
  | "openfga_server"
  | "cedar_http"
  | "custom_http"
  | "custom_grpc"
  | "pollen_cloud_pdp";

export type PdpRuntimeStatus =
  | "installed"
  | "not_configured"
  | "loading"
  | "ready"
  | "degraded"
  | "error"
  | "disabled";

export interface PdpProbeResult {
  ok: boolean;
  effect: string;
  reason: string;
  latency_ms: number;
  decision_id: string;
  details: any;
}

export interface PdpRuntime {
  id: string;
  name: string;
  category: PdpRuntimeCategory;
  kind: PdpKind;
  mode: string;
  system_managed: boolean;
  enabled: boolean;
  status: PdpRuntimeStatus;
  capabilities: string[];
  endpoint?: string;
  auth_ref?: string;
  config_source: string;
  active_bundle_id?: string;
  active_bundle_hash?: string;
  last_activated_at?: string;
  last_probe?: PdpProbeResult;
  health?: any;
  created_at: string;
  updated_at: string;
}

export interface CloudPdpProfile {
  tenant_id?: string;
  device_id?: string;
  pdp_endpoint?: string;
  contract_version?: string;
  auth_method?: string;
  status: string;
  manual_override_enabled: boolean;
  health?: any;
}

export type PdpRouteMode =
  | "local_only"
  | "local_primary_remote_fallback"
  | "remote_primary_local_fallback"
  | "cloud_primary_local_fallback"
  | "shadow_remote"
  | "mirror_audit_only"
  | "strict_remote";

export type PdpFailureBehavior =
  | "deny"
  | "fallback"
  | "last_known_good"
  | "allow"
  | "not_applicable";

export interface RouteMatch {
  agent_ids?: string[];
  tool_categories?: string[];
  resource_types?: string[];
  protocols?: string[];
  sensitivities?: string[];
  environments?: string[];
  risk_tiers?: string[];
}

export interface PdpRouteRule {
  id: string;
  name: string;
  enabled: boolean;
  priority: number;
  description?: string;
  match_cond: RouteMatch;
  mode: PdpRouteMode;
  primary_pdp_id: string;
  fallback_pdp_ids: string[];
  shadow_pdp_ids: string[];
  required_pdp_ids?: string[];
  merge_strategy: string;
  failure_behavior: PdpFailureBehavior;
  timeout_ms: number;
  max_retries: number;
  circuit_breaker_threshold?: number;
  cooldown_secs?: number;
  last_known_good_ttl_secs?: number;
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

export interface MatchedSignal {
  kind: string;
  detail: string;
  weight: number;
}

export interface DiscoveredAgentCandidateV2 {
  schema_version: string;
  candidate_id: string;
  tenant_id: string;
  device_id: string;
  status: string; // 'pending_approval' | 'registered' | etc
  display_name: string;
  vendor?: string;
  product?: string;
  inferred_agent_type: string;
  confidence: number;
  risk_score: number;
  first_seen: string;
  last_seen: string;
  scan_ids?: string[];
  last_scan_id?: string;
  evidence: DiscoveryEvidenceV2[];
  matched_signals?: MatchedSignal[];
  capability_tags?: string[];
  discovered_configs: any[];
  discovered_endpoints: any[];
  discovered_mcp_servers: any[];
  suggested_registration: any;
  suggested_observation_profile: any;
  suggested_control_bindings: ControlBindingPlan[];
  telemetry_plan: any;
  labels: Record<string, string>;
}

export interface IdentityConfirmation {
  candidate_id: string;
  confirmed_signature_id?: string;
  custom_display_name?: string;
  custom_vendor?: string;
  custom_product?: string;
  confirmed_agent_type: string;
  confirmed_capability_tags: string[];
  make_local_signature: boolean;
  confirmed_by: string;
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

export type PolicyFeasibilityStatus =
  | "can_enforce_now"
  | "can_enforce_after_approval"
  | "can_partially_enforce"
  | "can_observe_only"
  | "needs_setup"
  | "unsupported"
  | "unknown";

export type ProductMode =
  | "desktop_simple"
  | "desktop_advanced"
  | "enterprise_server";

export type PolicyIntent =
  | "observe_agent_activity"
  | "approve_risky_tool_calls"
  | "block_specific_tools"
  | "redact_sensitive_parameters"
  | "block_sensitive_file_upload"
  | "block_unknown_network_destinations"
  | "restrict_local_model_usage"
  | "limit_token_or_cost_usage"
  | "require_entity_relationship"
  | "detect_prompt_injection"
  | "kill_switch_on_anomaly";

export type ControlMethod =
  | "agent_tool_control"
  | "agent_config_wrapper"
  | "local_api_control"
  | "browser_activity_monitor"
  | "network_control"
  | "process_observation"
  | "observe_only";

export type InternalPep =
  | "mcp_proxy"
  | "mcp_stdio_wrapper"
  | "http_proxy"
  | "browser_extension"
  | "linux_ebpf"
  | "windows_wfp"
  | "macos_network_extension"
  | "secure_spool_observer"
  | "none";

export type InternalPdp =
  | "cedar"
  | "opa_wasm"
  | "open_fga"
  | "cloud"
  | "router_only";

export interface LocalizedText {
  en: string;
  th: string;
}

export interface RequiredUserAction {
  kind: string;
  label: LocalizedText;
}

export interface DiagnosticFinding {
  code: string;
  message: string;
}

export interface Enforceability {
  can_observe: boolean;
  can_warn: boolean;
  can_require_approval: boolean;
  can_enforce: boolean;
  can_strict_deny: boolean;
}

export interface LegacyControlMethodPlan {
  method: ControlMethod;
  internal_pep: InternalPep;
  internal_pdp: InternalPdp;
  enforceability: Enforceability;
  reason_code: string;
  explanation: LocalizedText;
  diagnostics: DiagnosticFinding[];
}

export interface PolicyFeasibilityRequest {
  policy_id?: string;
  policy_intent: PolicyIntent;
  requested_control_level: string;
  targets: any[];
  mode: ProductMode;
}

export interface LegacyPolicyFeasibilityResult {
  target: any;
  policy_intent: PolicyIntent;
  requested_control_level: string;
  effective_control_level: string;
  status: PolicyFeasibilityStatus;
  user_summary: LocalizedText;
  user_detail: LocalizedText;
  required_actions: RequiredUserAction[];
  technical_plan?: ControlMethodPlan;
  confidence: number;
}

export interface ControlMethodCapability {
  method: ControlMethod;
  internal_pep: InternalPep;
  status: string;
  can_observe: boolean;
  can_enforce: boolean;
  requires_admin: boolean;
  requires_user_approval: boolean;
  confidence: number;
  evidence: any[];
  user_message: LocalizedText;
  next_action?: RequiredUserAction;
}

export interface LegacyLocalCapabilitySnapshot {
  snapshot_id: string;
  device_id: string;
  os: any;
  agents: any[];
  methods: ControlMethodCapability[];
  generated_at: string;
}

export interface SuggestedPolicy {
  suggestion_id: string;
  policy_template_id: string;
  display_name: LocalizedText;
  description: LocalizedText;
  target_agent_ids: string[];
  recommended_control_level: string;
  feasibility: PolicyFeasibilityStatus;
  confidence: number;
  reason_codes: string[];
  setup_required: RequiredUserAction[];
}

export interface LegacyDeploymentSession {
  deployment_id: string;
  policy_id: string;
  policy_version: string;
  requested_control_level: string;
  target_scope: any;
  status: string;
  created_at: string;
  updated_at: string;
  created_by: string;
}

// V2 Policy-First Types
export type ControlLevel = "observe" | "warn" | "ask" | "enforce";
export type FeasibilityVerdict =
  | "fully_enforceable"
  | "partial_observe"
  | "observe_only"
  | "not_applicable";
export type MethodStatus =
  | "available"
  | "needs_install"
  | "needs_permission"
  | "unsupported";

export interface ControlMethodCap {
  id: string; // "mcp_stdio" | "linux_ebpf" | "windows_wfp_um" | ...
  domains: string[]; // ["network","file_system",...]
  max_level: ControlLevel;
  status: MethodStatus;
  requires: string[]; // ["admin","entitlement",...]
  source: string;
  maturity: string;
}
export interface CapabilityUpgrade {
  unlocks: string;
  method_id: string;
  how_th: string;
  how_en: string;
  download_url?: string;
  auto_installable: boolean;
  requires_restart: boolean;
}
export interface LocalCapabilitySnapshot {
  os: { name: string; version: string };
  captured_at: string;
  control_methods: ControlMethodCap[];
  install_suggestions: CapabilityUpgrade[];
  snapshot_hash: string;
}
export interface DomainFeasibility {
  domain: string;
  chosen_method?: string;
  level: ControlLevel;
  reason_th: string;
  reason_en: string;
}
export interface PolicyFeasibilityResult {
  policy_id: string;
  requested_level: ControlLevel;
  achievable_level: ControlLevel;
  verdict: FeasibilityVerdict;
  per_domain: DomainFeasibility[];
  gaps: CapabilityUpgrade[];
  friendly_th: string;
  friendly_en: string;
}
export interface MethodBinding {
  domain: string;
  method_id: string;
  effective_level: ControlLevel;
  maturity: string;
}
export interface ControlMethodPlan {
  policy_id: string;
  bindings: MethodBinding[];
  fallbacks: string[];
  auto_selected: boolean;
}
export interface PolicySuggestion {
  id: string;
  title_th: string;
  title_en: string;
  domains: string[];
  recommended_level: ControlLevel;
}
export interface DeploySession {
  id: string;
  feasibility: PolicyFeasibilityResult;
  plan?: ControlMethodPlan;
  status: string;
}

export type EventCategory =
  | "discovery"
  | "capability"
  | "policy_feasibility"
  | "deployment"
  | "approval"
  | "enforcement"
  | "observation"
  | "telemetry"
  | "health"
  | "rollback";

export type EventStatus = "pending" | "success" | "warning" | "error" | "info";

export interface UserVisibleEvent {
  event_id: string;
  correlation_id: string;
  scan_id?: string;
  deployment_id?: string;
  agent_id?: string;
  entity_id?: string;
  policy_id?: string;
  category: EventCategory;
  status: EventStatus;
  title: LocalizedText;
  detail: LocalizedText;
  next_action?: RequiredUserAction;
  advanced?: any;
  created_at: string;
}

export type FallbackBehavior =
  | "downgrade_to_observe"
  | "warn_then_observe"
  | "require_user_setup"
  | "none";

export interface RoutePreview {
  user_control_method: ControlMethod;
  advanced_pep?: InternalPep;
  advanced_pdp?: InternalPdp;
  fallback: FallbackBehavior;
  warm_check_required: boolean;
  explanation: LocalizedText;
}

export type EntityStatus =
  | "active"
  | "inactive"
  | "pending"
  | "error"
  | "observing";

export interface EntityCardModel {
  entity_id: string;
  kind: string;
  display_name: string;
  icon_url?: string;
  status: EntityStatus;
  primary_status_text: LocalizedText;
  secondary_status_text?: LocalizedText;
  tags: string[];
  last_updated_at: string;
}

export type ObservedResource = components["schemas"]["ObservedResource"];
export type ObservedTool = components["schemas"]["ObservedTool"];
export type ObservedIdentity = components["schemas"]["ObservedIdentity"];
