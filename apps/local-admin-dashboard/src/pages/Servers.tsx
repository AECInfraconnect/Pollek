import { useState, useEffect } from "react";
import { Server, MoreVertical, Plus } from "lucide-react";
import { RegistryApi } from "../services/api";
import type { McpServer } from "../services/api";

export function Servers({ hideHeader = false }: { hideHeader?: boolean }) {
  const [servers, setServers] = useState<McpServer[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    RegistryApi.listMcpServers()
      .then(setServers)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  return (
    <div className="space-y-6">
      {!hideHeader && (
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-2xl font-bold tracking-tight">MCP Servers</h2>
            <p className="text-muted-foreground">
              Manage Model Context Protocol servers available in the local registry.
            </p>
          </div>
          <button className="flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors shadow-lg shadow-primary/20">
            <Plus className="h-4 w-4" />
            Add Server
          </button>
        </div>
      )}

      <div className="glass rounded-xl overflow-hidden border">
        <table className="w-full text-sm text-left">
          <thead className="bg-muted/50 text-muted-foreground">
            <tr>
              <th className="px-6 py-4 font-medium">Server Name</th>
              <th className="px-6 py-4 font-medium">Server ID</th>
              <th className="px-6 py-4 font-medium">Transport</th>
              <th className="px-6 py-4 font-medium">Endpoint</th>
              <th className="px-6 py-4 font-medium">Risk Level</th>
              <th className="px-6 py-4 font-medium text-right">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {loading ? (
              <tr>
                <td colSpan={6} className="px-6 py-8 text-center text-muted-foreground">
                  Loading MCP servers...
                </td>
              </tr>
            ) : servers.length === 0 ? (
              <tr>
                <td colSpan={6} className="px-6 py-8 text-center text-muted-foreground">
                  No MCP servers registered.
                </td>
              </tr>
            ) : servers.map((server) => (
              <tr key={server.server_id} className="hover:bg-muted/30 transition-colors">
                <td className="px-6 py-4">
                  <div className="flex items-center gap-3">
                    <div className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center">
                      <Server className="h-4 w-4 text-primary" />
                    </div>
                    <span className="font-medium">{server.name}</span>
                  </div>
                </td>
                <td className="px-6 py-4 text-muted-foreground font-mono text-xs">{server.server_id}</td>
                <td className="px-6 py-4">
                  <span className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium bg-muted text-foreground uppercase">
                    {server.transport}
                  </span>
                </td>
                <td className="px-6 py-4 text-muted-foreground truncate max-w-[200px]" title={server.endpoint}>
                  {server.endpoint}
                </td>
                <td className="px-6 py-4">
                  <span className={`inline-flex items-center gap-1.5 rounded-full px-2 py-1 text-xs font-medium ${
                    server.risk_level === 'high' || server.risk_level === 'critical'
                      ? 'bg-destructive/10 text-destructive' 
                      : server.risk_level === 'medium'
                      ? 'bg-amber-500/10 text-amber-500'
                      : 'bg-emerald-500/10 text-emerald-500'
                  }`}>
                    <span className={`h-1.5 w-1.5 rounded-full ${server.risk_level === 'high' || server.risk_level === 'critical' ? 'bg-destructive' : server.risk_level === 'medium' ? 'bg-amber-500' : 'bg-emerald-500'}`} />
                    {server.risk_level}
                  </span>
                </td>
                <td className="px-6 py-4 text-right">
                  <button className="text-muted-foreground hover:text-foreground transition-colors p-1">
                    <MoreVertical className="h-4 w-4" />
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
