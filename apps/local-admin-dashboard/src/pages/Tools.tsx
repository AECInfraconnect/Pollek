import { useState, useEffect } from "react";
import { Wrench, MoreVertical, Plus } from "lucide-react";
import { RegistryApi } from "../services/api";
import type { Tool } from "../services/api";
import { ToolDetailDrawer } from "../components/ToolDetailDrawer";

export function Tools({ hideHeader = false }: { hideHeader?: boolean }) {
  const [tools, setTools] = useState<Tool[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedTool, setSelectedTool] = useState<Tool | null>(null);

  useEffect(() => {
    RegistryApi.listTools()
      .then(setTools)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  return (
    <div className="space-y-6">
      {!hideHeader && (
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-2xl font-bold tracking-tight">MCP Tools</h2>
            <p className="text-muted-foreground">
              View registered capabilities provided by connected MCP servers.
            </p>
          </div>
          <button className="flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors shadow-lg shadow-primary/20">
            <Plus className="h-4 w-4" />
            Add Tool
          </button>
        </div>
      )}

      <div className="glass rounded-xl overflow-hidden border">
        <table className="w-full text-sm text-left">
          <thead className="bg-muted/50 text-muted-foreground">
            <tr>
              <th className="px-6 py-4 font-medium">Tool Name</th>
              <th className="px-6 py-4 font-medium">Description</th>
              <th className="px-6 py-4 font-medium">Data Access</th>
              <th className="px-6 py-4 font-medium">Side Effect Level</th>
              <th className="px-6 py-4 font-medium">Risk Level</th>
              <th className="px-6 py-4 font-medium text-right">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {loading ? (
              <tr>
                <td
                  colSpan={6}
                  className="px-6 py-8 text-center text-muted-foreground"
                >
                  Loading tools...
                </td>
              </tr>
            ) : tools.length === 0 ? (
              <tr>
                <td
                  colSpan={6}
                  className="px-6 py-8 text-center text-muted-foreground"
                >
                  No tools registered.
                </td>
              </tr>
            ) : (
              tools.map((tool) => (
                <tr
                  key={tool.tool_id}
                  className="hover:bg-muted/30 transition-colors cursor-pointer"
                  onClick={() => setSelectedTool(tool)}
                >
                  <td className="px-6 py-4">
                    <div className="flex items-center gap-3">
                      <div className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center">
                        <Wrench className="h-4 w-4 text-primary" />
                      </div>
                      <span className="font-medium">{tool.name}</span>
                    </div>
                  </td>
                  <td
                    className="px-6 py-4 text-muted-foreground truncate max-w-[200px]"
                    title={tool.description}
                  >
                    {tool.description || "No description"}
                  </td>
                  <td className="px-6 py-4">
                    <span className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium bg-muted text-foreground uppercase">
                      {tool.data_access_level}
                    </span>
                  </td>
                  <td className="px-6 py-4">
                    <span className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium bg-muted text-foreground uppercase">
                      {tool.side_effect_level}
                    </span>
                  </td>
                  <td className="px-6 py-4">
                    <span
                      className={`inline-flex items-center gap-1.5 rounded-full px-2 py-1 text-xs font-medium ${
                        tool.risk_level === "high" ||
                        tool.risk_level === "critical"
                          ? "bg-destructive/10 text-destructive"
                          : tool.risk_level === "medium"
                            ? "bg-amber-500/10 text-amber-500"
                            : "bg-emerald-500/10 text-emerald-500"
                      }`}
                    >
                      <span
                        className={`h-1.5 w-1.5 rounded-full ${tool.risk_level === "high" || tool.risk_level === "critical" ? "bg-destructive" : tool.risk_level === "medium" ? "bg-amber-500" : "bg-emerald-500"}`}
                      />
                      {tool.risk_level}
                    </span>
                  </td>
                  <td className="px-6 py-4 text-right">
                    <button className="text-muted-foreground hover:text-foreground transition-colors p-1">
                      <MoreVertical className="h-4 w-4" />
                    </button>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      <ToolDetailDrawer
        tool={selectedTool}
        onClose={() => setSelectedTool(null)}
      />
    </div>
  );
}
