export type LocalizedText = {
  en: string;
  th: string;
};

export type DeploymentPhase =
  | 'agent_discovery'
  | 'route_planning'
  | 'pep_deploy'
  | 'enforcement'
  | 'rollback';

export type EventStatus =
  | 'info'
  | 'success'
  | 'warning'
  | 'error';

export type UserActionKind = 'RequireAuth' | 'RequireApproval' | 'RequireConfig';

export type UserAction = {
  kind: UserActionKind;
  action_url: string;
  expires_at?: string;
};

export type DeploymentEvent = {
  event_id: string;
  deployment_id: string;
  agent_id?: string;
  entity_id?: string;
  policy_id: string;
  phase: DeploymentPhase;
  status: EventStatus;
  title: LocalizedText;
  detail: LocalizedText;
  technical_detail?: string;
  user_action?: UserAction;
  created_at: string;
  correlation_id: string;
};

export type DeploymentSessionStatus =
  | 'draft'
  | 'planning'
  | 'deploying'
  | 'waiting_for_user_action'
  | 'active'
  | 'partially_active'
  | 'active_observe_only'
  | 'failed'
  | 'rolled_back';

export type EnforcementLayer =
  | 'browser_extension'
  | 'macos_network_extension'
  | 'windows_wfp'
  | 'ebpf_network'
  | 'mcp_proxy'
  | 'mcp_stdio_wrapper'
  | 'http_proxy'
  | 'observe_only';

export type PdpEngine = 'OpenFga' | 'Cedar' | 'CloudAuthz';

export type RoutingPlan = {
  selected_pep: {
    layer: EnforcementLayer;
    name: LocalizedText;
  };
  selected_pdp: {
    engine: PdpEngine;
  };
  fallback_pep?: {
    layer: EnforcementLayer;
    name: LocalizedText;
  };
};

export type DeploymentSession = {
  deployment_id: string;
  policy_id: string;
  status: DeploymentSessionStatus;
  routing_plan?: RoutingPlan;
  created_at: string;
  updated_at: string;
};
