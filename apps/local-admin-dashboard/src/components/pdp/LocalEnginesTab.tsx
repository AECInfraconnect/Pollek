import { useState, useEffect } from "react";
import { PdpRuntimeApi } from "../../services/api";
import type { PdpRuntime } from "../../services/api";

export function LocalEnginesTab() {
  const [items, setItems] = useState<PdpRuntime[]>([]);
  const [loading, setLoading] = useState(true);
  const [showInternal, setShowInternal] = useState(false);
  const [actionStates, setActionStates] = useState<Record<string, string>>({});

  const reload = async () => {
    setLoading(true);
    try {
      const data = await PdpRuntimeApi.list();
      const locals = data.filter(
        (r: PdpRuntime) => r.category === "local_engine",
      );
      // Deduplicate by ID
      const unique = Array.from(
        new Map(locals.map((r: PdpRuntime) => [r.id, r])).values(),
      );
      setItems(unique as PdpRuntime[]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    reload();
  }, []);

  const handleAction = async (
    id: string,
    actionName: string,
    actionFn: () => Promise<any>,
  ) => {
    setActionStates((prev) => ({ ...prev, [id]: `Running ${actionName}...` }));
    try {
      await actionFn();
      setActionStates((prev) => ({ ...prev, [id]: `${actionName} success` }));
      reload();
    } catch (e: any) {
      setActionStates((prev) => ({
        ...prev,
        [id]: `${actionName} failed: ${e.message || String(e)}`,
      }));
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <p className="text-sm text-muted-foreground">
          Built-in PDP engines managed by Local Enforcement Kit. Local engines
          do not require endpoints.
        </p>
        <div className="flex items-center gap-4">
          <label className="flex items-center gap-2 cursor-pointer text-sm">
            <span className="text-muted-foreground">Show Internal Engines</span>
            <input
              type="checkbox"
              className="toggle toggle-primary toggle-sm"
              checked={showInternal}
              onChange={(e) => setShowInternal(e.target.checked)}
            />
          </label>
          <button
            onClick={reload}
            className="px-3 py-1 bg-secondary text-secondary-foreground rounded text-xs hover:opacity-80"
          >
            Refresh
          </button>
        </div>
      </div>

      <div className="rounded-md border">
        <table className="w-full text-sm text-left">
          <thead className="text-xs uppercase bg-muted/50">
            <tr>
              <th className="px-4 py-3">Name</th>
              <th className="px-4 py-3">Kind</th>
              <th className="px-4 py-3">Status</th>
              <th className="px-4 py-3">Active Bundle</th>
              <th className="px-4 py-3">Last Probe</th>
              <th className="px-4 py-3 text-right">Actions</th>
            </tr>
          </thead>
          <tbody>
            {items
              .filter((r) => showInternal || !r.system_managed)
              .map((r) => (
                <tr
                  key={r.id}
                  className="border-b last:border-0 hover:bg-muted/30"
                >
                  <td className="px-4 py-3 font-medium">{r.name}</td>
                  <td className="px-4 py-3">
                    <span className="bg-primary/10 text-primary px-2 py-1 rounded text-xs">
                      {r.kind}
                    </span>
                  </td>
                  <td className="px-4 py-3">
                    <span
                      className={`px-2 py-1 rounded text-xs ${
                        (r.status || "").toLowerCase() === "ready"
                          ? "bg-emerald-500/10 text-emerald-500"
                          : (r.status || "").toLowerCase() === "error"
                            ? "bg-rose-500/10 text-rose-500"
                            : (r.status || "").toLowerCase() === "degraded"
                              ? "bg-amber-500/10 text-amber-500"
                              : "bg-secondary text-secondary-foreground"
                      }`}
                    >
                      {r.status}
                    </span>
                  </td>
                  <td className="px-4 py-3 font-mono text-xs text-muted-foreground">
                    {r.active_bundle_id ?? "none"}
                  </td>
                  <td className="px-4 py-3">
                    {r.last_probe ? (
                      <div
                        className={`text-xs ${r.last_probe.ok ? "text-green-500" : "text-red-500"}`}
                      >
                        {r.last_probe.effect} ({r.last_probe.latency_ms}ms)
                        {r.last_probe.reason && (
                          <div className="text-muted-foreground opacity-80">
                            {r.last_probe.reason}
                          </div>
                        )}
                      </div>
                    ) : (
                      <span className="text-xs text-muted-foreground">
                        never
                      </span>
                    )}
                  </td>
                  <td className="px-4 py-3 text-right">
                    <div className="flex flex-col items-end gap-1">
                      <div className="flex items-center gap-2">
                        <button
                          onClick={() =>
                            handleAction(r.id, "Validate", () =>
                              PdpRuntimeApi.validate(r.id),
                            )
                          }
                          className="px-2 py-1 bg-secondary text-secondary-foreground rounded text-xs hover:opacity-80"
                        >
                          Validate
                        </button>
                        <button
                          onClick={() =>
                            handleAction(r.id, "Probe", () =>
                              PdpRuntimeApi.probe(r.id),
                            )
                          }
                          className="px-2 py-1 bg-secondary text-secondary-foreground rounded text-xs hover:opacity-80"
                        >
                          Probe
                        </button>
                        <button
                          onClick={() =>
                            handleAction(r.id, "Clear Cache", () =>
                              PdpRuntimeApi.clearCache(r.id),
                            )
                          }
                          className="px-2 py-1 bg-red-500/10 text-red-500 rounded text-xs hover:opacity-80"
                        >
                          Clear
                        </button>
                      </div>
                      {actionStates[r.id] && (
                        <div
                          className="text-[10px] text-muted-foreground max-w-[150px] truncate"
                          title={actionStates[r.id]}
                        >
                          {actionStates[r.id]}
                        </div>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            {!loading && items.length === 0 && (
              <tr>
                <td
                  colSpan={6}
                  className="px-4 py-8 text-center text-muted-foreground"
                >
                  No local engines found. Check local service logs.
                </td>
              </tr>
            )}
            {loading && items.length === 0 && (
              <tr>
                <td
                  colSpan={6}
                  className="px-4 py-8 text-center text-muted-foreground"
                >
                  Loading local engines...
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
