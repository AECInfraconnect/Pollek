import type {
  AgentObserveActivity,
  AiAgent,
  McpServer,
  Tool,
  Resource,
  Entity,
  Relationship,
  PolicyDraft,
  TelemetryEventEnvelope,
  BlackboxAiProvider,
  PolicySuggestion,
  SimulationRequest,
  SimulationResult,
  PdpRuntime,
  PdpRouteRule,
  CloudPdpProfile,
  DiscoveryCapabilityInventory,
  DiscoveryEnrichmentSession,
  DiscoveryEntityCandidate,
  DiscoveryScanJob,
  DiscoveredAgentCandidateV2,
  IdentityConfirmation,
  LocalCapabilitySnapshot,
  PolicyFeasibilityResult,
  DeploySession,
  ControlMethodPlan,
  ControlLevel,
  LocalCapabilitySnapshotV2,
  RuntimeModeV2,
  PluginMarketItem,
  InstalledPlugin,
  PluginInstallRequest,
} from "./types";
export type * from "./types";
import type { components } from "../../../../contracts/generated/typescript/api";
import type { GuardEvent, GuardIncidentEnvelope } from "../types/guard";

export type AiUsageSummary = components["schemas"]["AiUsageSummaryV1"];
export type AiUsageEventPage = components["schemas"]["AiUsageEventPageV1"];
export type AiBudgetLimit = components["schemas"]["AiBudgetLimitV1"];

export type GuardEventPage = {
  schema_version?: string;
  count?: number;
  items: Array<GuardEvent | GuardIncidentEnvelope>;
  unavailable?: boolean;
};

export type PromptGuardCheckRequest = {
  text: string;
  direction?: "request" | "response";
  agent_id?: string;
  source?: string;
  surface?: string;
  session_id?: string;
  url?: string;
  persist?: boolean;
};

export type PromptGuardCheckResponse = {
  schema_version: "pollek.prompt_guard.check.v1";
  event_id: string;
  action: GuardEvent["action"];
  severity: GuardEvent["severity"];
  persisted: boolean;
  raw_prompt_or_response_stored: false;
  storage_error?: string | null;
  guard_event: GuardEvent;
  recommended_actions: string[];
  message: string;
};

export type BrowserExtensionStatusItem =
  components["schemas"]["BrowserExtensionStatusItem"];
export type BrowserExtensionStatusResponse =
  components["schemas"]["BrowserExtensionStatusResponse"];

export type LocalObserveRefreshRequest = {
  include_estimates?: boolean;
  sources?: string[];
};

export type LocalObserveRefreshResponse = {
  schema_version: "local-observe-refresh.v1";
  tenant_id: string;
  scan_id: string;
  candidates_found: number;
  resource_events: number;
  identity_events: number;
  tool_events: number;
  usage_events: number;
  exact_usage_events: number;
  estimated_usage_events: number;
  capture_quality: string[];
  limitations: string[];
  next_steps: Array<{
    action_id: string;
    title: string;
    reason: string;
    route: string;
  }>;
};

export type ObserveInputKind =
  | "provider_usage_key"
  | "local_usage_log_path"
  | "cloud_read_role"
  | "oauth_read_token"
  | "proxy_ca_trust"
  | "provider_admin_write";

export type ObserveCredentialRequest = {
  kind: ObserveInputKind;
  title: string;
  what_we_ask: string;
  why: string;
  unlocks: string[];
  risk_level: "low" | "medium" | "high" | string;
  required_scope: string;
  least_privilege_tip: string;
  data_handling: string[];
  supported_now: boolean;
};

export type ObserveAccuracyInput = {
  input_id: string;
  kind: ObserveInputKind;
  label: string;
  provider?: string | null;
  redacted_preview: string;
  fingerprint: string;
  scope_note?: string | null;
  connected_at: string;
  updated_at: string;
  consent_statement: string;
  status: string;
  unlocks: string[];
};

export type ObserveAccuracyResponse = {
  schema_version: "pollek.observe_accuracy.v1";
  tenant_id: string;
  generated_at: string;
  active_level: string;
  active_level_label: string;
  ladder: Array<{
    level: string;
    label: string;
    status: string;
    description: string;
  }>;
  inputs: ObserveAccuracyInput[];
  available_requests: ObserveCredentialRequest[];
  suggested_local_log_paths: Array<{
    label: string;
    path: string;
    redacted_path: string;
    exists: boolean;
    reason: string;
  }>;
  data_handling: string[];
  next_steps: string[];
};

export type StoreObserveInputRequest = {
  kind: ObserveInputKind;
  input_value: string;
  input_id?: string;
  label?: string;
  provider?: string;
  scope_note?: string;
  consent_ack: boolean;
  consent_statement?: string;
};

export type DetectionRuleSummary = {
  id: string;
  name: string;
  severity: string;
  confidence: string;
  maturity: string;
  detect_type: string;
  default_response: string;
  enforce_if_capable?: string | null;
  observe_only_fallback: boolean;
  user_message: string;
  maps: Record<string, string[]>;
  setup_requirements: string[];
  can_stop_next_time: boolean;
  privacy_note: string;
};

export type ObserveSensor = {
  id: string;
  title: string;
  os: string[];
  domains: string[];
  layer: string;
  status: string;
  achieved_level?: string;
  achievable_level?: string;
  deterministic_decision?: string;
  evidence_sources?: string[];
  missing_requirements?: Array<Record<string, unknown>>;
  remediation?: Array<Record<string, unknown>>;
  can_observe: boolean;
  can_enforce: boolean;
  requires_admin: boolean;
  user_consent_required: boolean;
  setup_action: string;
  reason: string;
  fallback: string;
  package_path?: string | null;
  setup_state?: Record<string, unknown> | null;
};

export type DetectionCoverageResponse = {
  schema_version: string;
  tenant_id: string;
  generated_at: string;
  pack_id: string;
  pack_version: string;
  manifest_integrity: string;
  rule_count: number;
  coverage: {
    schema_version?: string;
    rule_count: number;
    frameworks: Record<string, Record<string, string[]>>;
  };
  rules: DetectionRuleSummary[];
  sensors: ObserveSensor[];
  research_basis: Array<{
    framework: string;
    source: string;
    implementation_use: string;
  }>;
  privacy_guards: string[];
  limitations: string[];
};

export const LOCAL_CONTROL_PLANE_DEFAULT_ORIGIN = "http://127.0.0.1:43891";

const htmlFallbackMessage = (url: string) =>
  `Local Control Plane API returned dashboard HTML instead of JSON for ${url}. ` +
  "This usually means the dashboard is running without the Local Control Plane API, " +
  "an old backend binary is still running, or the dev proxy points at the wrong port. " +
  "Restart local-control-plane and verify the API is available on 127.0.0.1:43891.";

export function isHtmlResponse(text: string, contentType?: string | null) {
  const normalizedContentType = (contentType ?? "").toLowerCase();
  if (normalizedContentType.includes("text/html")) return true;
  const sample = text.trimStart().slice(0, 128).toLowerCase();
  return sample.startsWith("<!doctype html") || sample.startsWith("<html");
}

export function tenantBaseUrl(originOrBase: string, tenantId = "local") {
  const trimmed = originOrBase.trim().replace(/\/+$/, "");
  if (!trimmed) return `/v1/tenants/${tenantId}`;
  if (trimmed.endsWith(`/v1/tenants/${tenantId}`)) return trimmed;
  if (trimmed.endsWith("/v1")) return `${trimmed}/tenants/${tenantId}`;
  return `${trimmed}/v1/tenants/${tenantId}`;
}

function envValue(name: string) {
  return (import.meta.env[name] as string | undefined)?.trim();
}

function configuredLocalOrigin() {
  return (
    envValue("VITE_POLLEK_LCP_ORIGIN") ??
    envValue("VITE_POLLEK_API_ORIGIN") ??
    ""
  );
}

async function parseJsonResponse<T>(res: Response, url: string): Promise<T> {
  const text = await res.text();
  const contentType = res.headers.get("content-type");
  if (isHtmlResponse(text, contentType)) {
    throw new Error(htmlFallbackMessage(url));
  }

  let parsed: unknown = undefined;
  if (text.trim()) {
    try {
      parsed = JSON.parse(text);
    } catch {
      if (!res.ok) {
        throw new Error(
          `HTTP Error ${res.status}: ${res.statusText || "non-JSON error response"}`,
        );
      }
      throw new Error(
        `Local Control Plane API returned a non-JSON response for ${url}.`,
      );
    }
  }

  if (!res.ok) {
    let errText = "";
    if (parsed && typeof parsed === "object") {
      const payload = parsed as { message?: unknown; error?: unknown };
      if (typeof payload.message === "string") errText = payload.message;
      else if (typeof payload.error === "string") errText = payload.error;
    }
    throw new Error(errText || `HTTP Error ${res.status}: ${res.statusText}`);
  }

  return parsed as T;
}

export interface ConnectorConfig {
  id: string;
  kind: string;
  endpoint: string;
  store_id?: string;
  health_interval_secs: number;
  mtls_enabled: boolean;
}

export type ContractDiscoveryResponse =
  components["schemas"]["ContractDiscoveryResponse"];

export class ControlPlaneClient {
  public baseUrl: string;
  public tenantId: string;

  constructor() {
    this.tenantId = "local";
    const origin = configuredLocalOrigin();
    this.baseUrl = origin
      ? tenantBaseUrl(origin, this.tenantId)
      : `/v1/tenants/${this.tenantId}`;
  }

  get rootUrl(): string {
    if (this.baseUrl.startsWith("http")) {
      const url = new URL(this.baseUrl);
      return `${url.protocol}//${url.host}`;
    }
    return "";
  }

  async getContractDiscovery(): Promise<ContractDiscoveryResponse> {
    const url = `${this.rootUrl}/.well-known/pollek-contract`;
    const res = await fetch(url);
    return parseJsonResponse<ContractDiscoveryResponse>(res, url);
  }

  public async fetchRootApi<T = any>(
    path: string,
    options?: RequestInit,
  ): Promise<T> {
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
    };

    const url = `${this.rootUrl}${path}`;
    const res = await fetch(url, {
      ...options,
      headers: {
        ...headers,
        ...options?.headers,
      },
    });
    return parseJsonResponse<T>(res, url);
  }

  public async fetchApi<T = any>(
    path: string,
    options?: RequestInit,
  ): Promise<T> {
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
    };

    const url = `${this.baseUrl}${path}`;
    const res = await fetch(url, {
      ...options,
      headers: {
        ...headers,
        ...options?.headers,
      },
    });
    return parseJsonResponse<T>(res, url);
  }

  async getHostCapabilities(): Promise<LocalCapabilitySnapshot> {
    return this.fetchApi("/capability-snapshot");
  }
  async getHostCapabilitiesV2(
    mode: RuntimeModeV2 = "desktop_advanced",
    demo?: {
      os: "windows" | "linux" | "macos";
      profile?: "ready" | "observe_only" | "needs_setup";
    },
  ): Promise<LocalCapabilitySnapshotV2> {
    const params = new URLSearchParams({ mode });
    if (demo) {
      params.set("demo_os", demo.os);
      params.set("demo_profile", demo.profile ?? "ready");
    }
    return this.fetchRootApi(
      `/v1/tenants/${this.tenantId}/devices/local/capability-snapshot-v2?${params}`,
    );
  }
  async refreshHostCapabilitiesV2(
    mode: RuntimeModeV2 = "desktop_advanced",
    demo?: {
      os: "windows" | "linux" | "macos";
      profile?: "ready" | "observe_only" | "needs_setup";
    },
  ): Promise<LocalCapabilitySnapshotV2> {
    const params = new URLSearchParams({ mode });
    if (demo) {
      params.set("demo_os", demo.os);
      params.set("demo_profile", demo.profile ?? "ready");
    }
    return this.fetchRootApi(
      `/v1/tenants/${this.tenantId}/devices/local/capability-refresh?${params}`,
      { method: "POST" },
    );
  }
  async scanAgents(): Promise<{ job_id: string }> {
    return this.fetchApi("/scan", { method: "POST" });
  }
  async getScanResult(jobId: string) {
    return this.fetchApi(`/scans/${jobId}`);
  }
  async getPolicySuggestions(agentIds: string[]): Promise<PolicySuggestion[]> {
    return this.fetchApi("/policy-suggestions", {
      method: "POST",
      body: JSON.stringify({ agents: agentIds }),
    });
  }
  async previewFeasibility(
    policy: unknown,
    level: ControlLevel,
  ): Promise<PolicyFeasibilityResult> {
    return this.fetchApi("/policies/feasibility", {
      method: "POST",
      body: JSON.stringify({ policy, requested_level: level }),
    });
  }
  async createDeploySession(input: {
    policy: unknown;
    agents: string[];
    requested_level: ControlLevel;
  }): Promise<DeploySession> {
    return this.fetchApi("/deployment-sessions", {
      method: "POST",
      body: JSON.stringify(input),
    });
  }
  async confirmDeploySession(id: string): Promise<ControlMethodPlan> {
    return this.fetchApi(`/deployment-sessions/${id}/confirm`, {
      method: "POST",
    });
  }
  async applyDeploySession(id: string) {
    return this.fetchApi(`/deployment-sessions/${id}/apply`, {
      method: "POST",
    });
  }

  // Registry
  async listAgents(): Promise<AiAgent[]> {
    const data = await this.fetchApi("/registry/agents");
    return data.items ?? data.agents ?? data;
  }

  async deleteAgent(agentId: string): Promise<void> {
    return this.fetchApi(`/registry/agents/${agentId}`, { method: "DELETE" });
  }

  async listMcpServers(): Promise<McpServer[]> {
    const data = await this.fetchApi("/registry/mcp-servers");
    return data.items ?? data.mcp_servers ?? data;
  }
  async registerMcpServer(payload: any): Promise<McpServer> {
    return this.fetchApi("/registry/mcp-servers", {
      method: "POST",
      body: JSON.stringify(payload),
    });
  }

  async deleteMcpServer(serverId: string): Promise<void> {
    return this.fetchApi(`/registry/mcp-servers/${serverId}`, {
      method: "DELETE",
    });
  }

  async listTools(): Promise<Tool[]> {
    const data = await this.fetchApi("/registry/tools");
    return data.items ?? data.tools ?? data;
  }

  async deleteTool(toolId: string): Promise<void> {
    return this.fetchApi(`/registry/tools/${toolId}`, { method: "DELETE" });
  }

  async listResources(): Promise<Resource[]> {
    const data = await this.fetchApi("/registry/resources");
    return data.items ?? data.resources ?? data;
  }

  async deleteResource(id: string): Promise<void> {
    return this.fetchApi(`/registry/resources/${id}`, {
      method: "DELETE",
    });
  }
  async listEntities(): Promise<Entity[]> {
    const data = await this.fetchApi("/registry/entities");
    return data.items ?? data.entities ?? data;
  }

  async deleteEntity(id: string): Promise<void> {
    return this.fetchApi(`/registry/entities/${id}`, {
      method: "DELETE",
    });
  }

  async listRelationships(): Promise<Relationship[]> {
    const data = await this.fetchApi("/registry/relationships");
    return data.items ?? data.relationships ?? data;
  }

  async deleteRelationship(id: string): Promise<void> {
    return this.fetchApi(`/registry/relationships/${id}`, {
      method: "DELETE",
    });
  }

  async listBlackboxAiProviders(): Promise<BlackboxAiProvider[]> {
    const data = await this.fetchApi("/registry/blackbox-ai");
    return data.items ?? data.providers ?? data;
  }

  async deleteBlackboxAi(id: string): Promise<void> {
    return this.fetchApi(`/registry/blackbox-ai/${id}`, { method: "DELETE" });
  }

  // Policies
  async listPolicies(): Promise<PolicyDraft[]> {
    const data = await this.fetchApi("/policies");
    return data.items ?? data.policies ?? data;
  }
  async createPolicy(draft: PolicyDraft): Promise<PolicyDraft> {
    return this.fetchApi("/policies", {
      method: "POST",
      body: JSON.stringify(draft),
    });
  }
  async updatePolicy(
    policyId: string,
    draft: PolicyDraft,
  ): Promise<PolicyDraft> {
    return this.fetchApi(`/policies/${policyId}`, {
      method: "PATCH",
      body: JSON.stringify(draft),
    });
  }
  async deletePolicy(policyId: string): Promise<void> {
    return this.fetchApi(`/policies/${policyId}`, { method: "DELETE" });
  }
  async publishPolicy(
    policyId: string,
  ): Promise<{ published: boolean; bundle_id: string; build_number: number }> {
    return this.fetchApi(`/policies/${policyId}/publish`, { method: "POST" });
  }

  async simulatePolicy(
    policyId: string,
    req: SimulationRequest,
  ): Promise<SimulationResult> {
    return this.fetchApi(`/policies/${policyId}/simulate`, {
      method: "POST",
      body: JSON.stringify(req),
    });
  }

  // Connectors (Legacy)
  async listConnectors(): Promise<ConnectorConfig[]> {
    const data = await this.fetchApi("/connectors");
    return data.items ?? data.connectors ?? data;
  }
  async upsertConnector(cfg: ConnectorConfig): Promise<ConnectorConfig> {
    return this.fetchApi("/connectors", {
      method: "POST",
      body: JSON.stringify(cfg),
    });
  }
  async testConnector(id: string): Promise<unknown> {
    return this.fetchApi(`/connectors/${id}/test`, { method: "POST" });
  }

  async listMarketplaceItems(): Promise<PluginMarketItem[]> {
    const data = await this.fetchApi("/marketplace/items");
    return data.items ?? data;
  }

  async listInstalledPlugins(): Promise<InstalledPlugin[]> {
    const data = await this.fetchApi("/plugins");
    return data.items ?? data;
  }

  async installPlugin(request: PluginInstallRequest): Promise<InstalledPlugin> {
    return this.fetchApi("/plugins/install", {
      method: "POST",
      body: JSON.stringify(request),
    });
  }

  async togglePlugin(id: string, enabled: boolean): Promise<InstalledPlugin> {
    return this.fetchApi(`/plugins/${id}/toggle`, {
      method: "POST",
      body: JSON.stringify({ enabled }),
    });
  }

  async uninstallPlugin(id: string): Promise<unknown> {
    return this.fetchApi(`/plugins/${id}`, { method: "DELETE" });
  }

  async checkPluginHealth(id: string): Promise<unknown> {
    return this.fetchApi(`/plugins/${id}/health`, { method: "POST" });
  }

  async updatePlugin(id: string, payload: Record<string, unknown> = {}) {
    return this.fetchApi(`/plugins/${id}/update`, {
      method: "POST",
      body: JSON.stringify(payload),
    });
  }

  async rollbackPlugin(id: string, payload: Record<string, unknown> = {}) {
    return this.fetchApi(`/plugins/${id}/rollback`, {
      method: "POST",
      body: JSON.stringify(payload),
    });
  }

  async canaryPlugin(id: string, payload: Record<string, unknown> = {}) {
    return this.fetchApi(`/plugins/${id}/canary`, {
      method: "POST",
      body: JSON.stringify(payload),
    });
  }

  async revokePlugin(id: string, payload: Record<string, unknown> = {}) {
    return this.fetchApi(`/plugins/${id}/revoke`, {
      method: "POST",
      body: JSON.stringify(payload),
    });
  }

  async getBrowserExtensionStatus(): Promise<BrowserExtensionStatusResponse> {
    return this.fetchApi("/browser-extension/status");
  }

  // PDP Runtimes
  async listPdpRuntimes(): Promise<PdpRuntime[]> {
    const data = await this.fetchApi("/pdp/runtimes");
    return data.items ?? data.runtimes ?? data;
  }
  async getPdpRuntime(id: string): Promise<PdpRuntime> {
    return this.fetchApi(`/pdp/runtimes/${id}`);
  }
  async upsertPdpRuntime(runtime: PdpRuntime): Promise<PdpRuntime> {
    return this.fetchApi("/pdp/runtimes", {
      method: "POST",
      body: JSON.stringify(runtime),
    });
  }
  async deletePdpRuntime(id: string): Promise<void> {
    return this.fetchApi(`/pdp/runtimes/${id}`, { method: "DELETE" });
  }
  async probePdpHealth(id: string, input?: any): Promise<unknown> {
    return this.fetchApi(`/pdp/runtimes/${id}/probe`, {
      method: "POST",
      body: JSON.stringify(input ?? {}),
    });
  }
  async validatePdpRuntime(id: string): Promise<unknown> {
    return this.fetchApi(`/pdp/runtimes/${id}/validate`, { method: "POST" });
  }
  async clearPdpCache(id: string): Promise<unknown> {
    return this.fetchApi(`/pdp/runtimes/${id}/cache/clear`, { method: "POST" });
  }

  // Cloud PDP
  async getCloudPdpProfile(): Promise<CloudPdpProfile> {
    return this.fetchApi("/pdp/cloud");
  }
  async loginCloudPdp(): Promise<CloudPdpProfile> {
    return this.fetchApi("/pdp/cloud/login", { method: "POST" });
  }
  async discoverCloudPdp(): Promise<CloudPdpProfile> {
    return this.fetchApi("/pdp/cloud/discover", { method: "POST" });
  }
  async probeCloudPdp(): Promise<unknown> {
    return this.fetchApi("/pdp/cloud/probe", { method: "POST" });
  }
  async updateCloudPdpProfile(payload: any): Promise<CloudPdpProfile> {
    return this.fetchApi("/pdp/cloud", {
      method: "PATCH",
      body: JSON.stringify(payload),
    });
  }
  async disconnectCloudPdp(): Promise<unknown> {
    return this.fetchApi("/pdp/cloud", { method: "DELETE" });
  }

  // PDP Routing
  async listPdpRoutes(): Promise<PdpRouteRule[]> {
    return this.fetchApi("/pdp/routes");
  }
  async getPdpRoute(id: string): Promise<PdpRouteRule> {
    return this.fetchApi(`/pdp/routes/${id}`);
  }
  async upsertPdpRoute(route: PdpRouteRule): Promise<PdpRouteRule> {
    return this.fetchApi("/pdp/routes", {
      method: "POST",
      body: JSON.stringify(route),
    });
  }
  async deletePdpRoute(id: string): Promise<void> {
    return this.fetchApi(`/pdp/routes/${id}`, { method: "DELETE" });
  }
  async simulatePdpRoute(payload: any): Promise<unknown> {
    return this.fetchApi("/pdp/routes/simulate", {
      method: "POST",
      body: JSON.stringify(payload),
    });
  }

  // Bundles
  async listBundles(): Promise<unknown[]> {
    return this.fetchApi("/bundles");
  }

  async pushSync(): Promise<unknown> {
    // Note: the server push is a stream, but we might just hit a sync endpoint.
    // Assuming /bundles/sync or just let the dashboard know it triggers a reload
    return this.fetchApi("/bundles/sync", { method: "POST" });
  }

  async deployToPep(pepId: string, bundleId: string): Promise<unknown> {
    return this.fetchApi(`/peps/${pepId}/deploy`, {
      method: "POST",
      body: JSON.stringify({ bundle_id: bundleId }),
    });
  }

  // Telemetry
  async listDecisionLogs(): Promise<TelemetryEventEnvelope[]> {
    const data = await this.fetchApi("/telemetry/decision-logs");
    return data.decisions ?? data;
  }

  async clearDecisionLogs(): Promise<void> {
    return this.fetchApi("/telemetry/decision-logs", { method: "DELETE" });
  }

  // Shadow AI & Discovery
  async listDiscoveryCandidates(): Promise<DiscoveredAgentCandidateV2[]> {
    return this.fetchApi("/discovery/candidates")
      .then((data: any) => data.items ?? data.candidates ?? data)
      .catch((err) => {
        // Degrade to an empty list but never silently: surface the real error
        // so a failing control plane is visible in the console, not hidden.
        console.warn("listDiscoveryCandidates failed:", err);
        return [];
      });
  }

  async listDiscoveryEntities(): Promise<DiscoveryEntityCandidate[]> {
    return this.fetchApi("/discovery/entities")
      .then((data: any) => data.items ?? data.entities ?? data)
      .catch(() => []);
  }

  async clearDiscoveryCandidates(): Promise<void> {
    return this.fetchApi("/discovery/candidates", { method: "DELETE" });
  }

  async deleteDiscoveryCandidate(id: string): Promise<void> {
    return this.fetchApi(`/discovery/candidates/${id}`, { method: "DELETE" });
  }

  async getDiscoveryCandidateCapabilities(
    candidateId: string,
  ): Promise<DiscoveryCapabilityInventory> {
    return this.fetchApi(`/discovery/candidates/${candidateId}/capabilities`);
  }

  async retrieveDiscoveryCandidateCapabilities(
    candidateId: string,
  ): Promise<DiscoveryCapabilityInventory> {
    return this.fetchApi(
      `/discovery/candidates/${candidateId}/retrieve-capabilities`,
      { method: "POST" },
    );
  }

  async getAgentObserveActivity(
    agentId: string,
    options: { altIds?: string[]; limit?: number } = {},
  ): Promise<AgentObserveActivity> {
    const params = new URLSearchParams();
    const altIds = (options.altIds ?? []).filter(Boolean);
    if (altIds.length > 0) params.set("alt_ids", altIds.join(","));
    if (options.limit) params.set("limit", String(options.limit));
    const query = params.toString();
    return this.fetchApi(
      `/observations/agents/${encodeURIComponent(agentId)}/activity${
        query ? `?${query}` : ""
      }`,
    );
  }

  async startDiscoveryCandidateEnrichment(
    candidateId: string,
    payload: { sources?: string[] } = {},
  ): Promise<DiscoveryEnrichmentSession> {
    return this.fetchApi(
      `/discovery/candidates/${candidateId}/enrichment/start`,
      {
        method: "POST",
        body: JSON.stringify(payload),
      },
    );
  }

  async getDiscoveryCandidateEnrichment(
    sessionId: string,
  ): Promise<DiscoveryEnrichmentSession> {
    return this.fetchApi(`/discovery/enrichment/${sessionId}`);
  }

  async approveDiscoveryCandidateEnrichment(
    sessionId: string,
    acceptedSources: string[],
  ): Promise<DiscoveryEnrichmentSession> {
    return this.fetchApi(`/discovery/enrichment/${sessionId}/approve`, {
      method: "POST",
      body: JSON.stringify({ accepted_sources: acceptedSources }),
    });
  }

  async submitDiscoveryCandidateEnrichment(
    sessionId: string,
  ): Promise<DiscoveryEnrichmentSession> {
    return this.fetchApi(`/discovery/enrichment/${sessionId}/submit`, {
      method: "POST",
    });
  }

  async confirmCandidate(
    candidateId: string,
    payload: IdentityConfirmation,
  ): Promise<any> {
    return this.fetchApi(`/discovery/candidates/${candidateId}/confirm`, {
      method: "POST",
      body: JSON.stringify(payload),
    });
  }

  async triggerDiscoveryScan(
    req: any = {},
  ): Promise<{ scan_id: string; status: string }> {
    return this.fetchApi("/discovery/scans", {
      method: "POST",
      body: JSON.stringify(req),
    });
  }

  async listDiscoveryScans(): Promise<DiscoveryScanJob[]> {
    return this.fetchApi("/discovery/scans")
      .then((data: any) => data.items ?? data.scans ?? data)
      .catch(() => []);
  }

  async getDiscoveryScanStatus(scanId: string): Promise<DiscoveryScanJob> {
    return this.fetchApi(`/discovery/scans/${scanId}`);
  }

  async cancelDiscoveryScan(scanId: string): Promise<DiscoveryScanJob> {
    return this.fetchApi(`/discovery/scans/${scanId}/cancel`, {
      method: "POST",
    });
  }

  async registerDiscoveryCandidate(
    candidateId: string,
    payload: any = {},
  ): Promise<any> {
    return this.fetchApi(`/discovery/candidates/${candidateId}/register`, {
      method: "POST",
      body: JSON.stringify(payload),
    });
  }

  async generateControlPlan(candidateId: string): Promise<any> {
    return this.fetchApi(`/discovery/candidates/${candidateId}/control-plan`, {
      method: "POST",
    });
  }

  async applyControlBinding(bindingId: string): Promise<any> {
    return this.fetchApi(`/discovery/control-bindings/${bindingId}/apply`, {
      method: "POST",
    });
  }

  async rollbackControlBinding(bindingId: string): Promise<any> {
    return this.fetchApi(`/discovery/control-bindings/${bindingId}/rollback`, {
      method: "POST",
    });
  }

  // Policy Suggestions
  async listPolicySuggestions(): Promise<PolicySuggestion[]> {
    const data = await this.fetchApi("/policy-suggestions");
    return data.items ?? data;
  }

  async generatePolicySuggestions(): Promise<unknown> {
    return this.fetchApi("/policy-suggestions/generate", { method: "POST" });
  }

  // Presets
  async listPresets(): Promise<unknown> {
    return this.fetchApi("/policy-presets");
  }
  async previewPreset(id: string, params: unknown): Promise<unknown> {
    return this.fetchApi(`/policy-presets/${id}/preview`, {
      method: "POST",
      body: JSON.stringify(params),
    });
  }
  async createDraftFromPreset(id: string, params: unknown): Promise<unknown> {
    return this.fetchApi(`/policy-presets/${id}/create-draft`, {
      method: "POST",
      body: JSON.stringify(params),
    });
  }
  async simulatePreset(id: string, payload: unknown): Promise<unknown> {
    return this.fetchApi(`/policy-presets/${id}/simulate`, {
      method: "POST",
      body: JSON.stringify(payload),
    });
  }
  async checkPepCapabilities(req: unknown): Promise<unknown> {
    return this.fetchApi("/pep-capabilities/check", {
      method: "POST",
      body: JSON.stringify(req),
    });
  }

  // Cost
  async getCostSummary(): Promise<unknown> {
    return this.fetchApi("/observations/costs");
  }

  async getSignalCorrelation(): Promise<SignalCorrelationResponse> {
    return this.fetchApi<SignalCorrelationResponse>("/correlation");
  }

  async getDekContract(): Promise<DekContractResponse> {
    return this.fetchApi<DekContractResponse>("/contract");
  }

  async evaluateContract(
    compatibility: BundleCompatibility,
  ): Promise<ContractEvaluationResponse> {
    return this.fetchApi<ContractEvaluationResponse>("/contract/evaluate", {
      method: "POST",
      body: JSON.stringify(compatibility),
    });
  }

  async getAdapterInfo(): Promise<ContractAdapterInfo> {
    return this.fetchApi<ContractAdapterInfo>("/contract/adapter");
  }

  async adaptBundle(bundle: unknown): Promise<ContractAdaptationResult> {
    return this.fetchApi<ContractAdaptationResult>("/contract/adapt", {
      method: "POST",
      body: JSON.stringify({ bundle }),
    });
  }

  async getAiUsageSummary(params?: {
    from?: string;
    to?: string;
    bucket?: string;
    agent_id?: string;
    agent_type?: string;
    provider?: string;
    model?: string;
    task_id?: string;
    session_id?: string;
    surface?: string;
  }): Promise<AiUsageSummary> {
    const query = new URLSearchParams();
    for (const [key, value] of Object.entries(params ?? {})) {
      if (value) query.set(key, value);
    }
    const suffix = query.toString() ? `?${query}` : "";
    return this.fetchApi(`/usage/summary${suffix}`);
  }

  async getAiUsageEvents(params?: {
    from?: string;
    to?: string;
    agent_id?: string;
    provider?: string;
    model?: string;
    sync_status?: string;
    limit?: number;
  }): Promise<AiUsageEventPage> {
    const query = new URLSearchParams();
    for (const [key, value] of Object.entries(params ?? {})) {
      if (value !== undefined && value !== null && value !== "") {
        query.set(key, String(value));
      }
    }
    const suffix = query.toString() ? `?${query}` : "";
    return this.fetchApi(`/usage/events${suffix}`);
  }

  async getDetectionCoverage(): Promise<DetectionCoverageResponse> {
    return this.fetchApi("/detections/coverage");
  }

  async listObserveSensors(): Promise<{
    schema_version: string;
    tenant_id: string;
    generated_at: string;
    items: ObserveSensor[];
  }> {
    return this.fetchApi("/detections/sensors");
  }

  async preflightObserveSensor(sensorId: string): Promise<unknown> {
    return this.fetchApi(`/detections/sensors/${sensorId}/preflight`, {
      method: "POST",
      body: JSON.stringify({}),
    });
  }

  async consentObserveSensor(
    sensorId: string,
    accepted = true,
  ): Promise<unknown> {
    return this.fetchApi(`/detections/sensors/${sensorId}/consent`, {
      method: "POST",
      body: JSON.stringify({
        accepted,
        scopes: ["observe_metadata", "local_history"],
      }),
    });
  }

  async requestObserveSensorInstall(
    sensorId: string,
    requestedLevel: "observe" | "enforce" = "observe",
  ): Promise<unknown> {
    return this.fetchApi(`/detections/sensors/${sensorId}/install`, {
      method: "POST",
      body: JSON.stringify({
        accepted: true,
        requested_level: requestedLevel,
      }),
    });
  }
}

// Global default client. The admin dashboard always targets the local
// control plane; cloud sync is performed server-side by local-control-plane
// (see cloud_sync.rs / DEK_CLOUD_URL), not from the browser.
export const defaultClient = new ControlPlaneClient();

export const DeploymentApi = {
  listInventory: () => defaultClient.fetchApi("/agent-inventory"),
  getInventory: (agentId: string) =>
    defaultClient.fetchApi(`/agent-inventory/${agentId}`),
  recommend: (payload: any) =>
    defaultClient.fetchApi("/policy-deployment/recommend", {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  preview: (payload: any) =>
    defaultClient.fetchApi("/policy-deployment/preview", {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  simulate: (payload: any) =>
    defaultClient.fetchApi("/policy-deployment/simulate", {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  deploy: (payload: any) =>
    defaultClient.fetchApi("/policy-deployment/deploy", {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  autoPlan: (intent: string) =>
    defaultClient.fetchRootApi("/v1/enforcement/auto-plan", {
      method: "POST",
      body: JSON.stringify({ intent }),
    }),
  rollback: (deploymentId: string) =>
    defaultClient.fetchApi(`/deployments/${deploymentId}/rollback`, {
      method: "POST",
    }),
};

export const CapabilityApi = {
  getSnapshotV2: (
    mode?: RuntimeModeV2,
    demo?: {
      os: "windows" | "linux" | "macos";
      profile?: "ready" | "observe_only" | "needs_setup";
    },
  ) => defaultClient.getHostCapabilitiesV2(mode, demo),
  refreshSnapshotV2: (
    mode?: RuntimeModeV2,
    demo?: {
      os: "windows" | "linux" | "macos";
      profile?: "ready" | "observe_only" | "needs_setup";
    },
  ) => defaultClient.refreshHostCapabilitiesV2(mode, demo),
};

export const LogApi = {
  decisions: () => defaultClient.fetchApi("/telemetry/decision-logs"),
  toolInvocations: () => defaultClient.fetchApi("/logs/tool-invocations"),
  resourceAccess: () => defaultClient.fetchApi("/logs/resource-access"),
  deployments: () => defaultClient.fetchApi("/logs/policy-deployments"),
  pepHealth: () => defaultClient.fetchApi("/logs/pep-health"),
};

export const RegistryApi = {
  listAgents: () => defaultClient.listAgents(),
  deleteAgent: (id: string) => defaultClient.deleteAgent(id),
  listMcpServers: () => defaultClient.listMcpServers(),
  deleteMcpServer: (id: string) => defaultClient.deleteMcpServer(id),
  listTools: () => defaultClient.listTools(),
  deleteTool: (id: string) => defaultClient.deleteTool(id),
  listResources: () => defaultClient.listResources(),
  deleteResource: (id: string) => defaultClient.deleteResource(id),
  listEntities: () => defaultClient.listEntities(),
  deleteEntity: (id: string) => defaultClient.deleteEntity(id),
  listRelationships: () => defaultClient.listRelationships(),
  deleteRelationship: (id: string) => defaultClient.deleteRelationship(id),
  listBlackboxAiProviders: () => defaultClient.listBlackboxAiProviders(),
  deleteBlackboxAi: (id: string) => defaultClient.deleteBlackboxAi(id),
  listDiscoveryCandidates: () => defaultClient.listDiscoveryCandidates(),
  listDiscoveryEntities: () => defaultClient.listDiscoveryEntities(),
  clearDiscoveryCandidates: () => defaultClient.clearDiscoveryCandidates(),
  deleteDiscoveryCandidate: (id: string) =>
    defaultClient.deleteDiscoveryCandidate(id),
  getDiscoveryCandidateCapabilities: (candidateId: string) =>
    defaultClient.getDiscoveryCandidateCapabilities(candidateId),
  retrieveDiscoveryCandidateCapabilities: (candidateId: string) =>
    defaultClient.retrieveDiscoveryCandidateCapabilities(candidateId),
  getAgentObserveActivity: (
    agentId: string,
    options?: { altIds?: string[]; limit?: number },
  ) => defaultClient.getAgentObserveActivity(agentId, options),
  startDiscoveryCandidateEnrichment: (
    candidateId: string,
    payload?: { sources?: string[] },
  ) => defaultClient.startDiscoveryCandidateEnrichment(candidateId, payload),
  getDiscoveryCandidateEnrichment: (sessionId: string) =>
    defaultClient.getDiscoveryCandidateEnrichment(sessionId),
  approveDiscoveryCandidateEnrichment: (
    sessionId: string,
    acceptedSources: string[],
  ) =>
    defaultClient.approveDiscoveryCandidateEnrichment(
      sessionId,
      acceptedSources,
    ),
  submitDiscoveryCandidateEnrichment: (sessionId: string) =>
    defaultClient.submitDiscoveryCandidateEnrichment(sessionId),
  confirmCandidate: (candidateId: string, payload: IdentityConfirmation) =>
    defaultClient.confirmCandidate(candidateId, payload),
  triggerDiscoveryScan: (req?: any) => defaultClient.triggerDiscoveryScan(req),
  listDiscoveryScans: () => defaultClient.listDiscoveryScans(),
  getDiscoveryScanStatus: (scanId: string) =>
    defaultClient.getDiscoveryScanStatus(scanId),
  cancelDiscoveryScan: (scanId: string) =>
    defaultClient.cancelDiscoveryScan(scanId),
  registerDiscoveryCandidate: (candidateId: string, payload?: any) =>
    defaultClient.registerDiscoveryCandidate(candidateId, payload),
  generateControlPlan: (candidateId: string) =>
    defaultClient.generateControlPlan(candidateId),
  applyControlBinding: (bindingId: string) =>
    defaultClient.applyControlBinding(bindingId),
  rollbackControlBinding: (bindingId: string) =>
    defaultClient.rollbackControlBinding(bindingId),
};

export const PolicySuggestionApi = {
  list: () => defaultClient.listPolicySuggestions(),
  generate: () => defaultClient.generatePolicySuggestions(),
};

export const ObservationApi = {
  getCostSummary: () => defaultClient.getCostSummary(),
};

export interface AgentProcessBinding {
  agent_id: string;
  pids: number[];
  exe_path_hash: string | null;
  process_names: string[];
  cgroup_ids: number[];
}

export type CorrelationBasis =
  | "pid_and_exe"
  | "exe_hash"
  | "cgroup"
  | "pid"
  | "process_name_unique";

export interface CorrelationAttribution {
  pid: number;
  process_name: string;
  exe_path_redacted: string | null;
  agent_id: string;
  basis: CorrelationBasis;
  confidence: number;
}

export interface SignalCorrelationResponse {
  schema_version: string;
  tenant_id: string;
  generated_at: string;
  agents_indexed: number;
  bindings: AgentProcessBinding[];
  live_scan: {
    processes_scanned: number;
    attributed: number;
    attributions: CorrelationAttribution[];
  };
}

export const CorrelationApi = {
  get: () => defaultClient.getSignalCorrelation(),
};

export interface OsModulesConfig {
  linux: string[];
  windows: string[];
  macos: string[];
}

export interface DekContract {
  dek_version: string;
  contract_version: string;
  supported_bundle_api_versions: string[];
  available_pep_types: string[];
  os_modules: OsModulesConfig;
  platform: string;
}

export interface DekContractResponse {
  schema_version: string;
  contract: DekContract;
}

export interface BundleCompatibility {
  min_dek_version: string;
  required_crates: string[];
  required_pep_types: string[];
  required_os_modules: OsModulesConfig;
}

export type CompatibilityStatus =
  | "compatible"
  | "needs_upgrade"
  | "unsupported";

export interface CompatibilityVerdict {
  status: CompatibilityStatus;
  reasons: string[];
  missing_pep_types: string[];
  missing_os_modules: string[];
  dek_version: string;
  min_dek_version: string;
}

export interface ContractEvaluationResponse {
  schema_version: string;
  contract: DekContract;
  compatibility: BundleCompatibility;
  verdict: CompatibilityVerdict;
}

export interface ContractAdapterInfo {
  loaded: boolean;
  plugin_id?: string;
  version?: string;
  wasm_sha256?: string;
  wasm_bytes?: number;
  runtime?: string;
  error?: string;
}

export interface ContractAdaptationResult {
  schema_version: string;
  adapter: { plugin_id: string; version: string; wasm_sha256: string };
  to_contract: string;
  adapted: boolean;
  changes: string[];
  migrated_bundle: unknown;
  verdict_before: CompatibilityVerdict | null;
  verdict_after: CompatibilityVerdict | null;
}

export interface WorkloadIdentity {
  schema_version: string;
  tenant_id: string;
  device: {
    actor_id: string;
    workspace_id: string;
    environment_id: string;
  };
  transport: {
    mode?: "mtls" | "bearer";
    mtls_ready: boolean;
    svid_present: boolean;
    private_key_present: boolean;
    trust_bundle_present: boolean;
  };
  workload_identity: {
    provisioned: boolean;
    spiffe_id?: string | null;
    subject?: string;
    issuer?: string;
    serial?: string;
    not_before_unix?: number;
    not_after_unix?: number;
    seconds_until_expiry?: number;
    expired?: boolean;
    error?: string;
  };
  user_identity: {
    oauth_configured: boolean;
    auth_mechanism?: "private_key_jwt" | "client_credentials" | "static_bearer" | "none";
    oidc_issuer?: string | null;
    oidc_client_id?: string | null;
    auth_subject?: string | null;
  };
}

export const IdentityApi = {
  get: () => defaultClient.fetchApi<WorkloadIdentity>("/identity"),
};

// ---- Trust & Provenance (Trust Policy Gate) --------------------------------

export type TrustCheckStatus = "pass" | "fail" | "skipped";
export type TrustDecision = "accept" | "quarantine";

export interface TrustCheck {
  name: string;
  status: TrustCheckStatus;
  detail: string;
}

export interface TrustVerdict {
  decision: TrustDecision;
  bundle_id: string;
  tenant: string;
  bundle_revision: string;
  signer_key_id?: string | null;
  checks: TrustCheck[];
  failure_classes: string[];
  evaluated_at_unix: number;
}

export interface TrustPolicyView {
  require_signature: boolean;
  require_provenance: boolean;
  require_sbom: boolean;
  require_test_attestation: boolean;
  require_generation_monotonicity: boolean;
  signer_allowlist: string[];
  expected_tenant?: string | null;
  min_slsa_level: number;
  min_approvers: number;
}

export interface TrustProvenanceView {
  schema_version: string;
  tenant: string;
  policy: TrustPolicyView;
  keys: { provisioned: boolean; usable_now: number };
  verdicts: TrustVerdict[];
}

export const TrustApi = {
  get: () => defaultClient.fetchApi<TrustProvenanceView>("/trust"),
  verify: (envelope: unknown, artifacts?: Record<string, string>) =>
    defaultClient.fetchApi<{ tenant: string; verdict: TrustVerdict }>(
      "/trust/verify",
      { method: "POST", body: JSON.stringify({ envelope, artifacts: artifacts ?? {} }) },
    ),
};

export const ContractApi = {
  get: () => defaultClient.getDekContract(),
  evaluate: (compat: BundleCompatibility) =>
    defaultClient.evaluateContract(compat),
  adapterInfo: () => defaultClient.getAdapterInfo(),
  adapt: (bundle: unknown) => defaultClient.adaptBundle(bundle),
};

export interface DefinitionState {
  schema_version: string;
  tenant_id: string;
  current: {
    schema_version: string;
    definition_version: number;
    counts: {
      signatures: number;
      web_ai_signatures: number;
      browser_processes: number;
      installed_app_signatures: number;
    };
  };
  last_activation: {
    operation?: string;
    kind?: string;
    from_version?: number;
    to_version?: number;
    activated_at?: string;
  } | null;
  rollback_available: boolean;
}

export interface DefinitionActivateResult {
  status: string;
  reason?: string;
  event?: Record<string, unknown>;
  current: DefinitionState["current"];
}

export const DefinitionApi = {
  getState: () => defaultClient.fetchApi<DefinitionState>("/definitions"),
  activate: (definition: unknown) =>
    defaultClient.fetchApi<DefinitionActivateResult>("/definitions/activate", {
      method: "POST",
      body: JSON.stringify(definition),
    }),
  rollback: () =>
    defaultClient.fetchApi<DefinitionActivateResult>("/definitions/rollback", {
      method: "POST",
      body: "{}",
    }),
};

export const UsageApi = {
  getSummary: (
    params?: Parameters<ControlPlaneClient["getAiUsageSummary"]>[0],
  ) => defaultClient.getAiUsageSummary(params),
  getEvents: (params?: Parameters<ControlPlaneClient["getAiUsageEvents"]>[0]) =>
    defaultClient.getAiUsageEvents(params),
  streamUrl: () =>
    `${defaultClient.rootUrl}/v1/tenants/${defaultClient.tenantId}/usage/stream`,
};

export interface ProviderCreditConfig {
  provider: string;
  currency_per_credit: number;
  initial_credits?: number | null;
  label?: string | null;
}

export interface CreditLedgerConfig {
  schema_version?: string;
  currency: string;
  providers: ProviderCreditConfig[];
}

export interface ProviderCreditStatus {
  provider: string;
  label?: string | null;
  currency_per_credit: number;
  initial_credits?: number | null;
  consumed_cost: number;
  consumed_credits: number;
  remaining_credits?: number | null;
}

export interface CreditLedgerStatus {
  currency: string;
  providers: ProviderCreditStatus[];
  total_consumed_credits: number;
  total_remaining_credits?: number | null;
}

export interface CreditLedgerResponse {
  config: CreditLedgerConfig;
  status: CreditLedgerStatus;
}

export const CreditApi = {
  get: (params?: {
    from?: string;
    bucket?: string;
  }): Promise<CreditLedgerResponse> => {
    const query = new URLSearchParams();
    if (params?.from) query.set("from", params.from);
    if (params?.bucket) query.set("bucket", params.bucket);
    const suffix = query.toString() ? `?${query}` : "";
    return defaultClient
      .fetchApi<CreditLedgerResponse>(`/usage/credits${suffix}`)
      .catch(() => ({
        config: { currency: "USD", providers: [] },
        status: {
          currency: "USD",
          providers: [],
          total_consumed_credits: 0,
          total_remaining_credits: null,
        },
      }));
  },
  put: (config: CreditLedgerConfig): Promise<{ config: CreditLedgerConfig }> =>
    defaultClient.fetchApi("/usage/credits", {
      method: "PUT",
      body: JSON.stringify(config),
    }),
};

export const LocalObserveApi = {
  refresh: (payload?: LocalObserveRefreshRequest) =>
    defaultClient.fetchApi<LocalObserveRefreshResponse>(
      "/local-observe/refresh",
      {
        method: "POST",
        body: JSON.stringify(payload ?? { include_estimates: true }),
      },
    ),
};

export const ObserveAccuracyApi = {
  get: () =>
    defaultClient.fetchApi<ObserveAccuracyResponse>("/observe/accuracy"),
  getRequest: (kind: ObserveInputKind) =>
    defaultClient.fetchApi<ObserveCredentialRequest>(
      `/observe/accuracy/requests/${kind}`,
    ),
  storeInput: (payload: StoreObserveInputRequest) =>
    defaultClient.fetchApi<{
      schema_version: string;
      input: ObserveAccuracyInput;
      message: string;
    }>("/observe/accuracy/inputs", {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  revokeInput: (inputId: string) =>
    defaultClient.fetchApi<{
      schema_version: string;
      input_id: string;
      revoked: boolean;
    }>(`/observe/accuracy/inputs/${encodeURIComponent(inputId)}`, {
      method: "DELETE",
    }),
};

export const DetectionApi = {
  coverage: () => defaultClient.getDetectionCoverage(),
  sensors: () => defaultClient.listObserveSensors(),
  preflightSensor: (sensorId: string) =>
    defaultClient.preflightObserveSensor(sensorId),
  consentSensor: (sensorId: string, accepted = true) =>
    defaultClient.consentObserveSensor(sensorId, accepted),
  requestSensorInstall: (
    sensorId: string,
    requestedLevel: "observe" | "enforce" = "observe",
  ) => defaultClient.requestObserveSensorInstall(sensorId, requestedLevel),
};

export const PolicyApi = {
  list: () => defaultClient.listPolicies(),
  create: (draft: PolicyDraft) => defaultClient.createPolicy(draft),
  update: (policyId: string, draft: PolicyDraft) =>
    defaultClient.updatePolicy(policyId, draft),
  delete: (policyId: string) => defaultClient.deletePolicy(policyId),
  publish: (policyId: string) => defaultClient.publishPolicy(policyId),
  simulate: (policyId: string, req: SimulationRequest) =>
    defaultClient.simulatePolicy(policyId, req),
  listPresets: () => defaultClient.listPresets(),
  previewPreset: (id: string, params: unknown) =>
    defaultClient.previewPreset(id, params),
  createDraftFromPreset: (id: string, params: unknown) =>
    defaultClient.createDraftFromPreset(id, params),
  simulatePreset: (id: string, payload: unknown) =>
    defaultClient.simulatePreset(id, payload),
  checkPepCapabilities: (req: unknown) =>
    defaultClient.fetchRootApi("/v1/tenants/default/pep-capabilities/check", {
      method: "POST",
      body: JSON.stringify(req),
    }),
  getCapabilities: () => defaultClient.fetchRootApi("/v1/host/capabilities"),
};

export const ActivityApi = {
  getActivity: () => defaultClient.fetchApi("/activity"),
};

export const BundleApi = {
  list: () => defaultClient.listBundles(),
  sync: () => defaultClient.pushSync(),
  deployToPep: (pepId: string, bundleId: string) =>
    defaultClient.deployToPep(pepId, bundleId),
};

export const TelemetryApi = {
  listDecisionLogs: () => defaultClient.listDecisionLogs(),
  clearDecisionLogs: () => defaultClient.clearDecisionLogs(),
  getObservations: (params?: {
    agentId?: string;
    target?: string;
    toolId?: string;
  }) => {
    let url = `/telemetry/observations?`;
    if (params?.agentId) url += `agent_id=${params.agentId}&`;
    if (params?.target)
      url += `target_redacted=${encodeURIComponent(params.target)}&`;
    if (params?.toolId) url += `tool_id=${encodeURIComponent(params.toolId)}&`;
    return defaultClient.fetchApi(url).catch(() => ({ items: [] }));
  },
  getEnforcementStatus: (agentId?: string) => {
    const url = agentId
      ? `/v1/telemetry/enforcement-status?agent_id=${agentId}`
      : `/v1/telemetry/enforcement-status`;
    return defaultClient.fetchRootApi(url);
  },
  listGuardEvents: (): Promise<GuardEventPage> =>
    defaultClient
      .fetchApi<GuardEventPage>("/telemetry/guard-events")
      .catch(() => ({
        schema_version: "guard-events.v1",
        count: 0,
        items: [],
        unavailable: true,
      })),
  checkPromptGuard: (
    request: PromptGuardCheckRequest,
  ): Promise<PromptGuardCheckResponse> =>
    defaultClient.fetchApi<PromptGuardCheckResponse>("/prompt-guard/check", {
      method: "POST",
      body: JSON.stringify(request),
    }),
  streamUrl: (
    channel:
      | "observations"
      | "resources"
      | "tools"
      | "identities"
      | "guard-events" = "observations",
  ) =>
    `${defaultClient.rootUrl}/v1/tenants/${defaultClient.tenantId}/telemetry/${channel}/stream`,
  listResourceInventory: async (
    params?: string | { agentId?: string; scope?: "local" | "cloud" },
  ) => {
    const normalized =
      typeof params === "string" ? { agentId: params } : (params ?? {});
    const query = new URLSearchParams();
    if (normalized.agentId) query.set("agent_id", normalized.agentId);
    if (normalized.scope) query.set("scope", normalized.scope);
    const suffix = query.toString() ? `?${query}` : "";
    const url = `/telemetry/resources${suffix}`;
    return defaultClient.fetchApi(url);
  },
  listToolInventory: async (agentId?: string) => {
    const query = new URLSearchParams();
    if (agentId) query.set("agent_id", agentId);
    const suffix = query.toString() ? `?${query}` : "";
    const url = `/telemetry/tools${suffix}`;
    return defaultClient.fetchApi(url);
  },
  listIdentityInventory: async (
    params?: string | { agentId?: string; scope?: "local" | "cloud" },
  ) => {
    const normalized =
      typeof params === "string" ? { agentId: params } : (params ?? {});
    const query = new URLSearchParams();
    if (normalized.agentId) query.set("agent_id", normalized.agentId);
    if (normalized.scope) query.set("scope", normalized.scope);
    const suffix = query.toString() ? `?${query}` : "";
    const url = `/telemetry/identities${suffix}`;
    return defaultClient.fetchApi(url);
  },
};

export const ConnectorApi = {
  list: () => defaultClient.listConnectors(),
  upsert: (cfg: ConnectorConfig) => defaultClient.upsertConnector(cfg),
  test: (id: string) => defaultClient.testConnector(id),
};

export const PluginApi = {
  marketplaceItems: () => defaultClient.listMarketplaceItems(),
  installed: () => defaultClient.listInstalledPlugins(),
  install: (request: PluginInstallRequest) =>
    defaultClient.installPlugin(request),
  toggle: (id: string, enabled: boolean) =>
    defaultClient.togglePlugin(id, enabled),
  uninstall: (id: string) => defaultClient.uninstallPlugin(id),
  health: (id: string) => defaultClient.checkPluginHealth(id),
  update: (id: string, payload?: Record<string, unknown>) =>
    defaultClient.updatePlugin(id, payload),
  rollback: (id: string, payload?: Record<string, unknown>) =>
    defaultClient.rollbackPlugin(id, payload),
  canary: (id: string, payload?: Record<string, unknown>) =>
    defaultClient.canaryPlugin(id, payload),
  revoke: (id: string, payload?: Record<string, unknown>) =>
    defaultClient.revokePlugin(id, payload),
};

export const BrowserExtensionApi = {
  status: () => defaultClient.getBrowserExtensionStatus(),
};

export const PdpRuntimeApi = {
  list: () => defaultClient.listPdpRuntimes(),
  get: (id: string) => defaultClient.getPdpRuntime(id),
  upsert: (rt: PdpRuntime) => defaultClient.upsertPdpRuntime(rt),
  delete: (id: string) => defaultClient.deletePdpRuntime(id),
  probe: (id: string, input?: any) => defaultClient.probePdpHealth(id, input),
  validate: (id: string) => defaultClient.validatePdpRuntime(id),
  clearCache: (id: string) => defaultClient.clearPdpCache(id),
};

export const PdpCloudApi = {
  get: () => defaultClient.getCloudPdpProfile(),
  login: () => defaultClient.loginCloudPdp(),
  discover: () => defaultClient.discoverCloudPdp(),
  probe: () => defaultClient.probeCloudPdp(),
  update: (payload: any) => defaultClient.updateCloudPdpProfile(payload),
  disconnect: () => defaultClient.disconnectCloudPdp(),
};

export const PdpRoutingApi = {
  list: () => defaultClient.listPdpRoutes(),
  get: (id: string) => defaultClient.getPdpRoute(id),
  upsert: (rt: PdpRouteRule) => defaultClient.upsertPdpRoute(rt),
  delete: (id: string) => defaultClient.deletePdpRoute(id),
  simulate: (payload: any) => defaultClient.simulatePdpRoute(payload),
};

export const PolicyFirstApi = {
  scan: () => defaultClient.fetchApi("/scan", { method: "POST" }),
  getLatestSnapshot: () => defaultClient.fetchApi("/capability-snapshot"),
  getPolicySuggestions: () => defaultClient.listPolicySuggestions(),
  generatePolicySuggestions: () => defaultClient.generatePolicySuggestions(),
  evaluateFeasibility: (req: any) =>
    defaultClient.fetchApi("/policies/feasibility", {
      method: "POST",
      body: JSON.stringify(req),
    }),
  createDeploymentSession: (req: any) =>
    defaultClient.fetchApi("/deployment-sessions", {
      method: "POST",
      body: JSON.stringify(req),
    }),
  approveAction: (sessionId: string, actionId: string) =>
    defaultClient.fetchApi(
      `/deployment-sessions/${sessionId}/actions/${actionId}/approve`,
      { method: "POST" },
    ),
};

export const SimpleWizardApi = {
  getHostCapabilities: () => defaultClient.getHostCapabilities(),
  scanAgents: () => defaultClient.scanAgents(),
  getScanResult: (jobId: string) => defaultClient.getScanResult(jobId),
  getPolicySuggestions: (agentIds: string[]) =>
    defaultClient.getPolicySuggestions(agentIds),
  previewFeasibility: (policy: unknown, level: ControlLevel) =>
    defaultClient.previewFeasibility(policy, level),
  createDeploySession: (input: {
    policy: unknown;
    agents: string[];
    requested_level: ControlLevel;
  }) => defaultClient.createDeploySession(input),
  confirmDeploySession: (id: string) => defaultClient.confirmDeploySession(id),
  applyDeploySession: (id: string) => defaultClient.applyDeploySession(id),
};
