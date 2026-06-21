import type { AiAgent, McpServer, Tool, Resource, Entity, Relationship, PolicyDraft, TelemetryEventEnvelope, BlackboxAiProvider, DiscoveryCandidate, PolicySuggestion } from './types';
export type * from './types';
import type { components } from '../../../../contracts/generated/typescript/api';

export type ContractDiscoveryResponse = components['schemas']['ContractDiscoveryResponse'];

export class ControlPlaneClient {
  public baseUrl: string;
  public tenantId: string;
  public mockRole: string;

  constructor(profile: 'local' | 'mock-cloud' = 'local') {
    if (profile === 'mock-cloud') {
      this.baseUrl = 'http://localhost:43891/v1/tenants/local';
      this.mockRole = 'admin';
    } else {
      this.baseUrl = '/v1/tenants/local';
      this.mockRole = '';
    }
    this.tenantId = 'local';
  }

  get rootUrl(): string {
    if (this.baseUrl.startsWith('http')) {
      const url = new URL(this.baseUrl);
      return `${url.protocol}//${url.host}`;
    }
    return '';
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
      'Content-Type': 'application/json',
    };
    if (this.mockRole) {
      headers['x-mock-role'] = this.mockRole;
    }
    
    const res = await fetch(`${this.baseUrl}${path}`, {
      ...options,
      headers: {
        ...headers,
        ...options?.headers,
      }
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
  async listAgents(): Promise<AiAgent[]> { return this.fetchApi('/registry/agents'); }
  async listMcpServers(): Promise<McpServer[]> { return this.fetchApi('/registry/mcp-servers'); }
  async listTools(): Promise<Tool[]> { return this.fetchApi('/registry/tools'); }
  async listResources(): Promise<Resource[]> { return this.fetchApi('/registry/resources'); }
  async listEntities(): Promise<Entity[]> { return this.fetchApi('/registry/entities'); }
  async listRelationships(): Promise<Relationship[]> { return this.fetchApi('/registry/relationships'); }
  async listBlackboxAiProviders(): Promise<BlackboxAiProvider[]> { return this.fetchApi('/registry/blackbox-ai'); }
  
  // Policies
  async listPolicies(): Promise<PolicyDraft[]> { return this.fetchApi('/policies'); }
  async createPolicy(draft: PolicyDraft): Promise<PolicyDraft> {
    return this.fetchApi('/policies', { method: 'POST', body: JSON.stringify(draft) });
  }
  async updatePolicy(policyId: string, draft: PolicyDraft): Promise<PolicyDraft> {
    return this.fetchApi(`/policies/${policyId}`, { method: 'PATCH', body: JSON.stringify(draft) });
  }
  async deletePolicy(policyId: string): Promise<void> {
    return this.fetchApi(`/policies/${policyId}`, { method: 'DELETE' });
  }
  async publishPolicy(policyId: string): Promise<{ published: boolean; bundle_id: string; build_number: number }> {
    return this.fetchApi(`/policies/${policyId}/publish`, { method: 'POST' });
  }

  async simulatePolicy(policyId: string, req: any): Promise<any> {
    return this.fetchApi(`/policies/${policyId}/simulate`, { method: 'POST', body: JSON.stringify(req) });
  }

  // Connectors
  async listConnectors(): Promise<any[]> { return this.fetchApi('/connectors'); }
  async upsertConnector(cfg: any): Promise<any> { return this.fetchApi('/connectors', { method: 'POST', body: JSON.stringify(cfg) }); }
  async testConnector(id: string): Promise<any> { return this.fetchApi(`/connectors/${id}/test`, { method: 'POST' }); }

  // Bundles
  async listBundles(): Promise<any[]> {
    return this.fetchApi('/bundles');
  }
  
  async pushSync(): Promise<any> {
    // Note: the mock server push is a stream, but we might just hit a sync endpoint.
    // Assuming /bundles/sync or just let the dashboard know it triggers a reload
    return this.fetchApi('/bundles/sync', { method: 'POST' });
  }

  async deployToPep(pepId: string, bundleId: string): Promise<any> {
    return this.fetchApi(`/peps/${pepId}/deploy`, { 
      method: 'POST', 
      body: JSON.stringify({ bundle_id: bundleId }) 
    });
  }

  // Telemetry
  async listDecisionLogs(): Promise<TelemetryEventEnvelope[]> {
    const data = await this.fetchApi('/telemetry/decision-logs');
    return data.decisions ?? data;
  }

  // Shadow AI & Discovery
  async listDiscoveryCandidates(): Promise<DiscoveryCandidate[]> {
    return this.fetchApi('/discovery/candidates').then((data: any) => data.candidates ?? data).catch(() => []); // Mock fallback if endpoint not exist
  }

  async triggerDiscoveryScan(): Promise<any> {
    return this.fetchApi('/discovery/scans', { method: 'POST', body: JSON.stringify({}) });
  }

  // Policy Suggestions
  async listPolicySuggestions(): Promise<PolicySuggestion[]> {
    const data = await this.fetchApi('/policy-suggestions');
    return data.items ?? data;
  }
  
  async generatePolicySuggestions(): Promise<any> {
    return this.fetchApi('/policy-suggestions/generate', { method: 'POST' });
  }

  // Cost
  async getCostSummary(): Promise<any> {
    return this.fetchApi('/observations/costs');
  }

}

// Store the active profile in localStorage to persist across reloads
const getStoredProfile = (): 'local' | 'mock-cloud' => {
  const p = localStorage.getItem('dek_admin_profile');
  if (p === 'mock-cloud') return 'mock-cloud';
  return 'local';
};

// Global default client
export const defaultClient = new ControlPlaneClient(getStoredProfile());

// Helper to switch profile
export const switchProfile = (profile: 'local' | 'mock-cloud') => {
  localStorage.setItem('dek_admin_profile', profile);
  window.location.reload();
};

// Proxy objects for backward compatibility with existing code
export const RegistryApi = {
  listAgents: () => defaultClient.listAgents(),
  listMcpServers: () => defaultClient.listMcpServers(),
  listTools: () => defaultClient.listTools(),
  listResources: () => defaultClient.listResources(),
  listEntities: () => defaultClient.listEntities(),
  listRelationships: () => defaultClient.listRelationships(),
  listBlackboxAiProviders: () => defaultClient.listBlackboxAiProviders(),
  listDiscoveryCandidates: () => defaultClient.listDiscoveryCandidates(),
  triggerDiscoveryScan: () => defaultClient.triggerDiscoveryScan(),
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
  update: (policyId: string, draft: PolicyDraft) => defaultClient.updatePolicy(policyId, draft),
  delete: (policyId: string) => defaultClient.deletePolicy(policyId),
  publish: (policyId: string) => defaultClient.publishPolicy(policyId),
  simulate: (policyId: string, req: any) => defaultClient.simulatePolicy(policyId, req),
};

export const BundleApi = {
  list: () => defaultClient.listBundles(),
  sync: () => defaultClient.pushSync(),
  deployToPep: (pepId: string, bundleId: string) => defaultClient.deployToPep(pepId, bundleId),
};

export const TelemetryApi = {
  listDecisionLogs: () => defaultClient.listDecisionLogs(),
};

export const ConnectorApi = {
  list: () => defaultClient.listConnectors(),
  upsert: (cfg: any) => defaultClient.upsertConnector(cfg),
  test: (id: string) => defaultClient.testConnector(id),
};

