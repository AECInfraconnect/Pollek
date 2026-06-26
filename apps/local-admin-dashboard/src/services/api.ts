import type {
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
  DiscoveryScanJob,
  DiscoveredAgentCandidateV2,
  IdentityConfirmation,
  LocalCapabilitySnapshot,
  PolicyFeasibilityResult,
  DeploySession,
  ControlMethodPlan,
  ControlLevel,
} from "./types";
export type * from "./types";
import type { components } from "../../../../contracts/generated/typescript/api";

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
  public mockRole: string;

  constructor(profile: "local" | "mock-cloud" = "local") {
    if (profile === "mock-cloud") {
      this.baseUrl = "http://localhost:43891/v1/tenants/local";
      this.mockRole = "admin";
    } else {
      this.baseUrl = "/v1/tenants/local";
      this.mockRole = "";
    }
    this.tenantId = "local";
  }

  get rootUrl(): string {
    if (this.baseUrl.startsWith("http")) {
      const url = new URL(this.baseUrl);
      return `${url.protocol}//${url.host}`;
    }
    return "";
  }

  async getContractDiscovery(): Promise<ContractDiscoveryResponse> {
    const res = await fetch(`${this.rootUrl}/.well-known/pollen-contract`);
    if (!res.ok) {
      throw new Error(await res.text());
    }
    return res.json();
  }

  public async fetchRootApi(path: string, options?: RequestInit) {
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
    };
    if (this.mockRole) {
      headers["x-mock-role"] = this.mockRole;
    }

    const res = await fetch(`${this.rootUrl}${path}`, {
      ...options,
      headers: {
        ...headers,
        ...options?.headers,
      },
    });
    if (!res.ok) {
      let errText = await res.text();
      try {
        const json = JSON.parse(errText);
        if (json.message) errText = json.message;
        else if (json.error) errText = json.error;
      } catch (e) {}
      throw new Error(errText || `HTTP Error ${res.status}: ${res.statusText}`);
    }
    return res.json();
  }

  public async fetchApi(path: string, options?: RequestInit) {
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
    };
    if (this.mockRole) {
      headers["x-mock-role"] = this.mockRole;
    }

    const res = await fetch(`${this.baseUrl}${path}`, {
      ...options,
      headers: {
        ...headers,
        ...options?.headers,
      },
    });
    if (!res.ok) {
      let errText = await res.text();
      try {
        const json = JSON.parse(errText);
        if (json.message) errText = json.message;
        else if (json.error) errText = json.error;
      } catch (e) {}
      throw new Error(errText || `HTTP Error ${res.status}: ${res.statusText}`);
    }
    return res.json();
  }

  async getHostCapabilities(): Promise<LocalCapabilitySnapshot> {
    return this.fetchApi("/v1/host/capabilities");
  }
  async scanAgents(): Promise<{ job_id: string }> {
    return this.fetchApi("/v1/discovery/scan", { method: "POST" });
  }
  async getScanResult(jobId: string) {
    return this.fetchApi(`/v1/discovery/scan/${jobId}`);
  }
  async getPolicySuggestions(agentIds: string[]): Promise<PolicySuggestion[]> {
    return this.fetchApi("/v1/policy/suggestions", {
      method: "POST",
      body: JSON.stringify({ agents: agentIds }),
    });
  }
  async previewFeasibility(
    policy: unknown,
    level: ControlLevel,
  ): Promise<PolicyFeasibilityResult> {
    return this.fetchApi("/v1/policy/feasibility", {
      method: "POST",
      body: JSON.stringify({ policy, requested_level: level }),
    });
  }
  async createDeploySession(input: {
    policy: unknown;
    agents: string[];
    requested_level: ControlLevel;
  }): Promise<DeploySession> {
    return this.fetchApi("/v1/deploy/session", {
      method: "POST",
      body: JSON.stringify(input),
    });
  }
  async confirmDeploySession(id: string): Promise<ControlMethodPlan> {
    return this.fetchApi(`/v1/deploy/session/${id}/confirm`, {
      method: "POST",
    });
  }
  async applyDeploySession(id: string) {
    return this.fetchApi(`/v1/deploy/session/${id}/apply`, { method: "POST" });
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
    // Note: the mock server push is a stream, but we might just hit a sync endpoint.
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
      .catch(() => []); // Mock fallback if endpoint not exist
  }

  async clearDiscoveryCandidates(): Promise<void> {
    return this.fetchApi("/discovery/candidates", { method: "DELETE" });
  }

  async deleteDiscoveryCandidate(id: string): Promise<void> {
    return this.fetchApi(`/discovery/candidates/${id}`, { method: "DELETE" });
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
}

// Store the active profile in localStorage to persist across reloads
const getStoredProfile = (): "local" | "mock-cloud" => {
  const p = localStorage.getItem("dek_admin_profile");
  if (p === "mock-cloud") return "mock-cloud";
  return "local";
};

// Global default client
export const defaultClient = new ControlPlaneClient(getStoredProfile());

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
    defaultClient.fetchApi("/enforcement/auto-plan", {
      method: "POST",
      body: JSON.stringify({ intent }),
    }),
  rollback: (deploymentId: string) =>
    defaultClient.fetchApi(`/policy-deployment/${deploymentId}/rollback`, {
      method: "POST",
    }),
};

export const LogApi = {
  decisions: () => defaultClient.fetchApi("/telemetry/decision-logs"),
  toolInvocations: () => defaultClient.fetchApi("/logs/tool-invocations"),
  resourceAccess: () => defaultClient.fetchApi("/logs/resource-access"),
  deployments: () => defaultClient.fetchApi("/logs/policy-deployments"),
  pepHealth: () => defaultClient.fetchApi("/logs/pep-health"),
};

// Helper to switch profile
export const switchProfile = (profile: "local" | "mock-cloud") => {
  localStorage.setItem("dek_admin_profile", profile);
  window.location.reload();
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
  clearDiscoveryCandidates: () => defaultClient.clearDiscoveryCandidates(),
  deleteDiscoveryCandidate: (id: string) =>
    defaultClient.deleteDiscoveryCandidate(id),
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
  getObservations: (params?: { agentId?: string, target?: string, toolId?: string }) => {
    let url = `/telemetry/observations?`;
    if (params?.agentId) url += `agent_id=${params.agentId}&`;
    if (params?.target) url += `target_redacted=${encodeURIComponent(params.target)}&`;
    if (params?.toolId) url += `tool_id=${encodeURIComponent(params.toolId)}&`;
    return defaultClient.fetchApi(url).catch(() => ({ items: [] }));
  },
  getEnforcementStatus: (agentId?: string) => {
    const url = agentId ? `/v1/telemetry/enforcement-status?agent_id=${agentId}` : `/v1/telemetry/enforcement-status`;
    return defaultClient.fetchRootApi(url);
  },
  streamUrl: (
    channel: "observations" | "resources" | "tools" | "identities" = "observations",
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


