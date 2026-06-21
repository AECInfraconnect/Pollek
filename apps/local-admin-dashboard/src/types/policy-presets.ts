export type ControlMode =
  | "observe"
  | "warn"
  | "approval"
  | "enforce"
  | "strict_deny";

export type PresetCategory =
  | "content_guard"
  | "pii_and_secrets"
  | "file_system"
  | "personal_resources"
  | "mcp_tools"
  | "network_and_providers"
  | "cost_and_tokens"
  | "audit_and_compliance"
  | "approval_workflow";

export type RiskTag =
  | "prompt_injection"
  | "sensitive_info_disclosure"
  | "insecure_plugin_design"
  | "excessive_agency"
  | "model_dos_cost_spike"
  | "shadow_ai"
  | "data_exfiltration"
  | "secret_leakage"
  | "unsafe_file_access"
  | "unsafe_network_egress"
  | "tool_poisoning"
  | "unauthorized_access"
  | "financial_risk";

export type PepType =
  | "mcp_proxy"
  | "stdio_wrapper"
  | "http_gateway"
  | "linux_ebpf"
  | "windows_wfp"
  | "macos_network_extension"
  | "file_system_pep"
  | "browser_extension"
  | "local_model_proxy"
  | "cloud_connector_proxy"
  | "embedded_sdk"
  | "telemetry_only";

export type PolicyOutputKind =
  | "rego"
  | "cedar"
  | "open_fga_model"
  | "pep_config"
  | "router_rule"
  | "redaction_pipeline"
  | "approval_workflow"
  | "telemetry_rule";

export type ArtifactKind =
  | "policy_draft"
  | "signed_bundle"
  | "pep_binding"
  | "pdp_route_rule"
  | "resource_scope"
  | "approval_rule"
  | "telemetry_subscription"
  | "rollback_snapshot";

export type PresetValueType =
  | "string"
  | "integer"
  | "float"
  | "boolean"
  | "string_list"
  | "path_list"
  | "glob_list"
  | "provider_list"
  | "agent_selector"
  | "tool_selector"
  | "resource_selector"
  | "duration"
  | "money"
  | "json";

export interface PresetParameter {
  key: string;
  label: string;
  description: string;
  value_type: PresetValueType;
  required: boolean;
  default_value: any;
  examples: any[];
}

export type PiiHandling = "none" | "hash" | "redact" | "local_only";

export interface TelemetryRequirement {
  event_type: string;
  required_fields: string[];
  pii_handling: PiiHandling;
}

export type SimulationWindow =
  | "last_24_hours"
  | "last_7_days"
  | "last_30_days";

export interface PolicyPresetV2 {
  id: string;
  version: string;
  title: string;
  short_description: string;
  long_description: string;
  category: PresetCategory;
  risk_tags: RiskTag[];
  supported_pep_types: PepType[];
  recommended_pep_types: PepType[];
  supported_control_modes: ControlMode[];
  default_control_mode: ControlMode;
  supported_policy_outputs: PolicyOutputKind[];
  parameters: PresetParameter[];
  generated_artifacts: ArtifactKind[];
  telemetry_requirements: TelemetryRequirement[];
  default_simulation_window: SimulationWindow;
  safety_notes: string[];
}

export type FileOperation =
  | "read"
  | "write"
  | "create"
  | "delete"
  | "rename"
  | "execute"
  | "list";

export interface PathScope {
  root_path: string;
  include_globs: string[];
  exclude_globs: string[];
  operations: FileOperation[];
}

export interface AccountScope {
  provider: string;
  account_id: string;
  scopes: string[];
}

export interface PresetTargets {
  agent_ids: string[];
  tool_ids: string[];
  resource_ids: string[];
  provider_ids: string[];
  path_scopes: PathScope[];
  account_scopes: AccountScope[];
}

export interface DeployPresetRequest {
  preset_id: string;
  preset_version?: string;
  control_mode: ControlMode;
  selected_pep_types: PepType[];
  targets: PresetTargets;
  params: Record<string, any>;
  dry_run_first: boolean;
}

export interface RenderedArtifact {
  language: string;
  content: string;
  warnings: string[];
}

export interface PolicyPresetPreviewResponse {
  schema_version: string;
  preset_id: string;
  recommended_pep_types: PepType[];
  artifacts: RenderedArtifact[];
}

export interface PolicyPresetSimulationResponse {
  schema_version: string;
  preset_id: string;
  result: {
    allowed: boolean;
    decision: string;
    reason: string;
    obligations?: any[];
    deployment_test?: string;
    syntax_check?: string;
    recommended_pep?: string;
  };
}
