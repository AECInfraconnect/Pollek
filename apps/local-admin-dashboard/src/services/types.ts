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
  source: 'manual' | 'discovery' | 'import' | 'cloud_sync' | 'agent_self_registration';
  status: 'discovered' | 'pending_approval' | 'registered' | 'active' | 'suspended' | 'deleted' | 'draft' | 'published' | 'compiled';
  tags: string[];
}

export interface AiAgent {
  meta: ObjectMeta;
  agent_id: string;
  name: string;
  agent_type: 'claude_desktop' | 'openai_agent' | 'langchain_agent' | 'llama_index_agent' | 'custom_mcp_client' | 'browser_agent' | 'cli_agent' | 'unknown';
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
  trust_level: 'untrusted' | 'low' | 'medium' | 'high' | 'system';
  capabilities: string[];
  labels: Record<string, string>;
}

export interface McpServer {
  meta: ObjectMeta;
  server_id: string;
  name: string;
  transport: 'stdio' | 'http' | 'sse' | 'web_socket';
  endpoint: string;
  owner_agent_id?: string;
  tools: string[];
  resources: string[];
  risk_level: 'low' | 'medium' | 'high' | 'critical';
}

import type { components } from '../../../../contracts/generated/typescript/api';

export type Tool = components['schemas']['Tool'];
export type Resource = components['schemas']['Resource'];
export type AgentObservationEvent = components['schemas']['AgentObservationEvent'];
export type CostLedgerEntry = components['schemas']['CostLedgerEntry'];
export type PolicySuggestion = components['schemas']['PolicySuggestion'];
export type DiscoveryCandidate = components['schemas']['DiscoveryCandidate'];

export interface Entity {
  meta: ObjectMeta;
  entity_id: string;
  entity_type: 'human_user' | 'service_account' | 'workload' | 'ai_agent' | 'organization' | 'tenant' | 'device';
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
export type PolicyType = 'rego' | 'cedar' | 'open_fga' | 'pii_redaction' | 'route' | 'composite';

export type PolicyLifecycleStatus =
  | 'draft' | 'validated' | 'simulation_passed' | 'compiled'
  | 'pending_approval' | 'approved' | 'published' | 'active';

export interface PolicyTargets {
  agent_ids: string[];
  tool_ids: string[];
  resource_ids: string[];
  entity_ids: string[];
  route_ids: string[];
}

export type PolicySource =
  | { kind: 'raw_text'; language: string; text: string }
  | { kind: 'template'; template_id: string; params: any }
  | { kind: 'structured'; ir: any };

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
  | 'decision_log' | 'policy_bundle_activated' | 'policy_bundle_rejected'
  | 'runtime_metric' | 'security_event' | 'pii_redaction_event'
  | 'adapter_health' | 'sync_health' | 'os_guardrail_event';

export type DecisionEffect =
  | 'allow' | 'deny' | 'redact' | 'mask' | 'warn' | 'require_approval' | 'break_glass_allow';

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
  adapter_results: { adapter_id: string; decision: DecisionEffect; reason?: string }[];
  obligations: { obligation_type: string; fields: string[] }[];
  latency_ms: number;
}

export interface BlackboxAiProvider {
  meta: ObjectMeta;
  provider_id: string;
  name: string;
  provider_type: 'openai' | 'anthropic' | 'google' | 'azure_openai' | 'custom';
  api_base_url?: string;
  auth_mechanism: {
    type: 'api_key' | 'bearer_token' | 'mtls' | 'none';
    secret_reference?: string;
  };
  supported_models: string[];
  default_model?: string;
  rate_limits: {
    requests_per_minute?: number;
    tokens_per_minute?: number;
  };
  trust_level: 'untrusted' | 'low' | 'medium' | 'high' | 'system';
  data_processing_agreement_signed: boolean;
  requires_pii_redaction: boolean;
}
