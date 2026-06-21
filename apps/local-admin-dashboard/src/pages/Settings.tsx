import { useState, useEffect } from "react";
import { switchProfile, defaultClient, ConnectorApi } from "../services/api";
import type { ContractDiscoveryResponse } from "../services/api";

export function Settings() {
  const [profile, setProfile] = useState<'local' | 'mock-cloud'>('local');
  const [connectors, setConnectors] = useState<any[]>([]);
  const [newConnectorUrl, setNewConnectorUrl] = useState('http://localhost:8181');
  const [testResults, setTestResults] = useState<Record<string, any>>({});
  const [discovery, setDiscovery] = useState<ContractDiscoveryResponse | null>(null);
  const [discoveryError, setDiscoveryError] = useState<string | null>(null);

  useEffect(() => {
    const p = localStorage.getItem('dek_admin_profile');
    if (p === 'mock-cloud') setProfile('mock-cloud');
    loadDiscovery();
    loadConnectors();
  }, []);

  const loadDiscovery = async () => {
    try {
      setDiscoveryError(null);
      const res = await defaultClient.getContractDiscovery();
      setDiscovery(res);
    } catch (e: any) {
      setDiscoveryError(e.message || String(e));
    }
  };

  const loadConnectors = async () => {
    try {
      const res = await ConnectorApi.list();
      setConnectors(res);
    } catch (e) {
      console.error(e);
    }
  };

  const handleProfileChange = (newProfile: 'local' | 'mock-cloud') => {
    setProfile(newProfile);
    switchProfile(newProfile); // This will reload the page
  };

  const handleAddConnector = async () => {
    if (!newConnectorUrl) return;
    try {
      await ConnectorApi.upsert({
        id: `opa-${Date.now()}`,
        kind: 'opa',
        endpoint: newConnectorUrl,
        health_interval_secs: 30,
        mtls_enabled: false
      });
      setNewConnectorUrl('');
      loadConnectors();
    } catch (e) {
      console.error(e);
    }
  };

  const handleTestConnector = async (id: string) => {
    try {
      const res = await ConnectorApi.test(id);
      setTestResults(prev => ({ ...prev, [id]: res }));
    } catch (e) {
      console.error(e);
      setTestResults(prev => ({ ...prev, [id]: { ok: false, latency_ms: 0, detail: 'error' } }));
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">Settings</h2>
          <p className="text-muted-foreground">
            Configure local control plane settings and synchronization profiles.
          </p>
        </div>
      </div>

      <div className="glass p-6 rounded-xl space-y-6">
        <h3 className="text-lg font-medium">Control Plane Profile</h3>
        
        <div className="space-y-4 max-w-md">
          <div className="grid gap-2">
            <label className="text-sm font-medium">Active Profile</label>
            <select 
              value={profile}
              onChange={(e) => handleProfileChange(e.target.value as any)}
              className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="local">Local Control Plane (127.0.0.1:43890)</option>
              <option value="mock-cloud">Mock Pollen Cloud (127.0.0.1:43891)</option>
            </select>
          </div>
          <div className="grid gap-2">
            <label className="text-sm font-medium">API Endpoint</label>
            <input 
              type="text" 
              className="flex h-10 w-full rounded-md border border-input bg-muted/50 px-3 py-2 text-sm text-muted-foreground"
              value={profile === 'mock-cloud' ? 'http://localhost:43891' : 'http://localhost:43890'}
              disabled
            />
          </div>
          <div className="grid gap-2">
            <label className="text-sm font-medium">Mock Role</label>
            <input 
              type="text" 
              className="flex h-10 w-full rounded-md border border-input bg-muted/50 px-3 py-2 text-sm text-muted-foreground"
              value={profile === 'mock-cloud' ? 'admin' : ''}
              disabled
            />
          </div>
        </div>
      </div>

      <div className="glass p-6 rounded-xl space-y-6">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-medium">Contract Discovery</h3>
          <button 
            onClick={loadDiscovery}
            className="px-3 py-1 bg-secondary text-secondary-foreground rounded text-xs hover:opacity-80"
          >
            Refresh
          </button>
        </div>
        
        {discoveryError ? (
          <div className="text-sm text-red-500 bg-red-500/10 p-4 rounded-md">
            Failed to load discovery: {discoveryError}
          </div>
        ) : discovery ? (
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-4 text-sm">
              <div className="space-y-1">
                <span className="text-muted-foreground block">Preferred Contract</span>
                <span className="font-medium bg-primary/10 text-primary px-2 py-1 rounded inline-block">{discovery.preferred}</span>
              </div>
              <div className="space-y-1">
                <span className="text-muted-foreground block">Schema Version</span>
                <span className="font-medium">{discovery.schema_version}</span>
              </div>
            </div>
            
            <div className="space-y-2">
              <span className="text-sm text-muted-foreground block">Supported Contracts</span>
              <div className="flex flex-wrap gap-2">
                {discovery.supported.map(s => (
                  <span key={s} className="text-xs bg-muted px-2 py-1 rounded-full">{s}</span>
                ))}
              </div>
            </div>

            <div className="space-y-2">
              <span className="text-sm text-muted-foreground block">Capabilities</span>
              <div className="flex flex-wrap gap-2">
                {discovery.capabilities.map(c => (
                  <span key={c} className="text-xs bg-muted px-2 py-1 rounded-full">{c}</span>
                ))}
              </div>
            </div>
          </div>
        ) : (
          <div className="text-sm text-muted-foreground">Loading...</div>
        )}
      </div>

      <div className="glass p-6 rounded-xl space-y-6">
        <h3 className="text-lg font-medium">PDP Connectors</h3>
        
        <div className="space-y-4">
          <div className="flex gap-2 max-w-md">
            <input 
              type="text" 
              placeholder="http://localhost:8181"
              className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              value={newConnectorUrl}
              onChange={(e) => setNewConnectorUrl(e.target.value)}
            />
            <button 
              onClick={handleAddConnector}
              className="h-10 px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm font-medium hover:opacity-90"
            >
              Add OPA
            </button>
          </div>

          <div className="rounded-md border">
            <table className="w-full text-sm text-left">
              <thead className="text-xs uppercase bg-muted/50">
                <tr>
                  <th className="px-4 py-3">ID</th>
                  <th className="px-4 py-3">Kind</th>
                  <th className="px-4 py-3">Endpoint</th>
                  <th className="px-4 py-3 text-right">Action</th>
                </tr>
              </thead>
              <tbody>
                {connectors.map(c => (
                  <tr key={c.id} className="border-b last:border-0">
                    <td className="px-4 py-3 font-medium">{c.id}</td>
                    <td className="px-4 py-3">{c.kind}</td>
                    <td className="px-4 py-3">{c.endpoint}</td>
                    <td className="px-4 py-3 text-right">
                      <div className="flex items-center justify-end gap-2">
                        {testResults[c.id] && (
                          <span className={`text-xs px-2 py-1 rounded ${testResults[c.id].ok ? 'bg-green-500/10 text-green-500' : 'bg-red-500/10 text-red-500'}`}>
                            {testResults[c.id].ok ? `✓ reachable (${testResults[c.id].latency_ms}ms)` : `✗ unreachable`}
                          </span>
                        )}
                        <button 
                          onClick={() => handleTestConnector(c.id)}
                          className="px-3 py-1 bg-secondary text-secondary-foreground rounded text-xs hover:opacity-80"
                        >
                          Test Connection
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
                {connectors.length === 0 && (
                  <tr>
                    <td colSpan={4} className="px-4 py-8 text-center text-muted-foreground">
                      No connectors configured. Add one above.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  );
}
