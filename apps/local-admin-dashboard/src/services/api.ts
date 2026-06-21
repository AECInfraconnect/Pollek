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
  DiscoveryScanJob,
  DiscoveredAgentCandidateV2,
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

  private async fetchApi(path: string, options?: RequestInit) {
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

  // Registry
  async listAgents(): Promise<AiAgent[]> {
    return this.fetchApi("/registry/agents");
  }
  async listMcpServers(): Promise<McpServer[]> {
    return this.fetchApi("/registry/mcp-servers");
  }
  async listTools(): Promise<Tool[]> {
    return this.fetchApi("/registry/tools");
  }
  async listResources(): Promise<Resource[]> {
    return this.fetchApi("/registry/resources");
  }
  async listEntities(): Promise<Entity[]> {
    return this.fetchApi("/registry/entities");
  }
  async listRelationships(): Promise<Relationship[]> {
    return this.fetchApi("/registry/relationships");
  }
  async listBlackboxAiProviders(): Promise<BlackboxAiProvider[]> {
    return this.fetchApi("/registry/blackbox-ai");
  }

  // Policies
  async listPolicies(): Promise<PolicyDraft[]> {
    return this.fetchApi("/policies");
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
    return this.fetchApi("/connectors");
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
    return this.fetchApi("/pdp/runtimes");
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
  async probePdpHealth(id: string): Promise<unknown> {
    return this.fetchApi(`/pdp/runtimes/${id}/health`, { method: "POST" });
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

  // Shadow AI & Discovery
  async listDiscoveryCandidates(): Promise<DiscoveredAgentCandidateV2[]> {
    return this.fetchApi("/discovery/candidates")
      .then((data: any) => data.candidates ?? data)
      .catch(() => []); // Mock fallback if endpoint not exist
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
      .then((data: any) => data.scans ?? data)
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

// Helper to switch profile
export const switchProfile = (profile: "local" | "mock-cloud") => {
  localStorage.setItem("dek_admin_profile", profile);
  window.location.reload();
};

export const RegistryApi = {
  listAgents: () => defaultClient.listAgents(),
  listMcpServers: () => defaultClient.listMcpServers(),
  listTools: () => defaultClient.listTools(),
  listResources: () => defaultClient.listResources(),
  listEntities: () => defaultClient.listEntities(),
  listRelationships: () => defaultClient.listRelationships(),
  listBlackboxAiProviders: () => defaultClient.listBlackboxAiProviders(),
  listDiscoveryCandidates: () => defaultClient.listDiscoveryCandidates(),
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
    defaultClient.checkPepCapabilities(req),
};

export const BundleApi = {
  list: () => defaultClient.listBundles(),
  sync: () => defaultClient.pushSync(),
  deployToPep: (pepId: string, bundleId: string) =>
    defaultClient.deployToPep(pepId, bundleId),
};

export const TelemetryApi = {
  listDecisionLogs: () => defaultClient.listDecisionLogs(),
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
  probeHealth: (id: string) => defaultClient.probePdpHealth(id),
};

export const PdpRoutingApi = {
  list: () => defaultClient.listPdpRoutes(),
  get: (id: string) => defaultClient.getPdpRoute(id),
  upsert: (rt: PdpRouteRule) => defaultClient.upsertPdpRoute(rt),
  delete: (id: string) => defaultClient.deletePdpRoute(id),
};
