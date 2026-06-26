import { useEffect, useState } from "react";
import type { components } from "../../../contracts/generated/typescript/api";

type ObservedResource = components["schemas"]["ObservedResource"];
type ObservedTool = components["schemas"]["ObservedTool"];
type ObservedIdentity = components["schemas"]["ObservedIdentity"];

interface InventoryPage<T> {
  schema_version: string;
  items: T[];
}

interface DashboardData {
  devices: { id: string; tenant_id: string; revoked: boolean }[];
  telemetry_count: number;
  current_version: string;
  audits: any[];
  resource_inventory?: InventoryPage<ObservedResource>;
  tool_inventory?: InventoryPage<ObservedTool>;
  identity_inventory?: InventoryPage<ObservedIdentity>;
}

function formatTime(value?: string) {
  return value ? new Date(value).toLocaleString() : "N/A";
}

function App() {
  const [data, setData] = useState<DashboardData | null>(null);

  useEffect(() => {
    // In a real scenario we'd fetch from /api/admin/dashboard
    fetch("/api/admin/dashboard/data")
      .then((r) => r.json())
      .then((d) => setData(d))
      .catch((e) => console.error(e));
  }, []);

  return (
    <div className="min-h-screen bg-background text-textMain p-8 flex flex-col items-center animate-fade-in">
      <div className="w-full max-w-6xl">
        <header className="flex justify-between items-center mb-8 glass-panel p-6">
          <h1 className="text-3xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-primary to-secondary">
            Mock Cloud Admin
          </h1>
          <div className="flex space-x-4">
            <button className="btn-primary">Settings</button>
            <button className="btn-danger">Logout</button>
          </div>
        </header>

        <main className="grid grid-cols-1 lg:grid-cols-3 gap-6 animate-slide-up">
          <section className="lg:col-span-2 glass-panel p-6">
            <h2 className="text-xl font-semibold mb-4">Enrolled Devices</h2>
            <div className="overflow-x-auto">
              <table className="w-full text-left">
                <thead>
                  <tr className="border-b border-white/10 text-textMuted">
                    <th className="pb-3 font-medium">Device ID</th>
                    <th className="pb-3 font-medium">Tenant</th>
                    <th className="pb-3 font-medium">Status</th>
                  </tr>
                </thead>
                <tbody>
                  {data?.devices?.map((d: any) => (
                    <tr
                      key={d.id}
                      className="border-b border-white/5 last:border-0 hover:bg-white/5 transition-colors"
                    >
                      <td className="py-3">{d.id}</td>
                      <td className="py-3 text-textMuted">{d.tenant_id}</td>
                      <td className="py-3">
                        <span
                          className={`px-2 py-1 rounded text-xs font-medium ${d.revoked ? "bg-accent/20 text-accent" : "bg-green-500/20 text-green-400"}`}
                        >
                          {d.revoked ? "Revoked" : "Active"}
                        </span>
                      </td>
                    </tr>
                  ))}
                  {!data?.devices?.length && (
                    <tr>
                      <td
                        colSpan={3}
                        className="py-4 text-center text-textMuted"
                      >
                        No devices enrolled
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </section>

          <section className="glass-panel p-6 flex flex-col space-y-6">
            <div>
              <h2 className="text-xl font-semibold mb-2">Policy Status</h2>
              <div className="p-4 bg-surface rounded-xl border border-white/5">
                <p className="text-sm text-textMuted">Current Active Bundle</p>
                <p className="text-lg font-bold">
                  {data?.current_version || "Unknown"}
                </p>
              </div>
            </div>
            <div>
              <h2 className="text-xl font-semibold mb-2">Telemetry</h2>
              <div className="p-4 bg-surface rounded-xl border border-white/5">
                <p className="text-sm text-textMuted">Events Captured</p>
                <p className="text-2xl font-bold text-primary">
                  {data?.telemetry_count || 0}
                </p>
              </div>
            </div>
          </section>

          <section className="lg:col-span-3 glass-panel p-6">
            <h2 className="text-xl font-semibold mb-4">
              Identities, Resources, and Tools
            </h2>
            <div className="grid grid-cols-1 xl:grid-cols-3 gap-5">
              <div className="overflow-x-auto">
                <h3 className="text-sm font-medium text-textMuted mb-3">
                  Identities
                </h3>
                <table className="w-full text-left text-sm">
                  <thead>
                    <tr className="border-b border-white/10 text-textMuted">
                      <th className="pb-2 font-medium">Identity</th>
                      <th className="pb-2 font-medium">Kind</th>
                      <th className="pb-2 font-medium">Provider</th>
                      <th className="pb-2 font-medium">Last Seen</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-white/5">
                    {data?.identity_inventory?.items?.map((identity) => (
                      <tr key={identity.identity_id}>
                        <td className="py-3">{identity.identity_label}</td>
                        <td className="py-3 text-textMuted">
                          {identity.identity_kind}
                        </td>
                        <td className="py-3 text-textMuted">
                          {identity.spiffe_id || identity.provider || "local"}
                        </td>
                        <td className="py-3 text-textMuted">
                          {formatTime(identity.last_seen)}
                        </td>
                      </tr>
                    ))}
                    {!data?.identity_inventory?.items?.length && (
                      <tr>
                        <td colSpan={4} className="py-4 text-center text-textMuted">
                          No identity access observed
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>

              <div className="overflow-x-auto">
                <h3 className="text-sm font-medium text-textMuted mb-3">
                  Data Resources
                </h3>
                <table className="w-full text-left text-sm">
                  <thead>
                    <tr className="border-b border-white/10 text-textMuted">
                      <th className="pb-2 font-medium">Resource</th>
                      <th className="pb-2 font-medium">Scope</th>
                      <th className="pb-2 font-medium">Count</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-white/5">
                    {data?.resource_inventory?.items?.map((resource) => (
                      <tr key={resource.resource_id}>
                        <td className="py-3">{resource.target_redacted}</td>
                        <td className="py-3 text-textMuted">{resource.scope}</td>
                        <td className="py-3 text-textMuted">
                          {resource.access_count}
                        </td>
                      </tr>
                    ))}
                    {!data?.resource_inventory?.items?.length && (
                      <tr>
                        <td colSpan={3} className="py-4 text-center text-textMuted">
                          No resource access observed
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>

              <div className="overflow-x-auto">
                <h3 className="text-sm font-medium text-textMuted mb-3">
                  Tools
                </h3>
                <table className="w-full text-left text-sm">
                  <thead>
                    <tr className="border-b border-white/10 text-textMuted">
                      <th className="pb-2 font-medium">Tool</th>
                      <th className="pb-2 font-medium">Kind</th>
                      <th className="pb-2 font-medium">Uses</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-white/5">
                    {data?.tool_inventory?.items?.map((tool) => (
                      <tr key={tool.tool_id}>
                        <td className="py-3">{tool.tool_name}</td>
                        <td className="py-3 text-textMuted">{tool.tool_kind}</td>
                        <td className="py-3 text-textMuted">{tool.use_count}</td>
                      </tr>
                    ))}
                    {!data?.tool_inventory?.items?.length && (
                      <tr>
                        <td colSpan={3} className="py-4 text-center text-textMuted">
                          No tool usage observed
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>
            </div>
          </section>
        </main>
      </div>
    </div>
  );
}

export default App;
