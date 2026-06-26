export type EntityType =
  | "agent"
  | "tool"
  | "resource"
  | "policy"
  | "identity"
  | "provider"
  | "model"
  | string;

export interface GraphMetric {
  label: string;
  value: string;
}

export interface GraphNode {
  id: string;
  type: EntityType;
  entity_id: string;
  label: string;
  subtitle?: string | null;
  status: string;
  risk?: string | null;
  mode?: string | null;
  badges: string[];
  metrics: GraphMetric[];
  href?: string | null;
  raw?: unknown;
}

export interface GraphEdge {
  id: string;
  source: string;
  target: string;
  relation: string;
  label: string;
  evidence: string;
  observed: boolean;
  enforced: boolean;
}

export interface RelationshipSummary {
  kind: string;
  label: string;
  count: number;
  tone: "neutral" | "info" | "success" | "warning" | "danger" | string;
}

export interface GraphWarning {
  code: string;
  message: string;
  entity_id?: string | null;
}

export interface EntityGraphResponse {
  schema_version: "entity-graph.v1";
  tenant_id: string;
  generated_at: string;
  center?: GraphNode | null;
  nodes: GraphNode[];
  edges: GraphEdge[];
  summaries: RelationshipSummary[];
  warnings: GraphWarning[];
}

export interface GraphRef {
  id: string;
  type: EntityType;
  entity_id: string;
  label: string;
}

export interface ActivityCost {
  total_cost_usd?: number | null;
  total_tokens?: number | null;
  provider?: string | null;
  model?: string | null;
}

export interface ActivityTimelineItem {
  event_id: string;
  timestamp: string;
  actor?: GraphRef | null;
  action: string;
  tool?: GraphRef | null;
  resource?: GraphRef | null;
  policies: GraphRef[];
  decision: string;
  enforcement_mode: string;
  pep_plane?: string | null;
  pdp_engine?: string | null;
  trace_id?: string | null;
  cost?: ActivityCost | null;
  explanation?: string | null;
  raw?: unknown;
}

export interface ActivityTimelineResponse {
  schema_version: "activity-timeline.v1";
  tenant_id: string;
  generated_at: string;
  items: ActivityTimelineItem[];
  next_cursor?: string | null;
}

export interface Entity360Response {
  schema_version: "entity-360.v1";
  tenant_id: string;
  generated_at: string;
  entity: GraphNode;
  graph: EntityGraphResponse;
  summaries: RelationshipSummary[];
  activity: ActivityTimelineItem[];
  warnings: GraphWarning[];
}

export interface EntityGraphQuery {
  types?: string;
  status?: string;
  q?: string;
  limit?: number;
}

export interface ActivityTimelineQuery {
  entity_type?: string;
  entity_id?: string;
  agent_id?: string;
  policy_id?: string;
  resource_id?: string;
  tool_id?: string;
  decision?: string;
  mode?: string;
  limit?: number;
}
