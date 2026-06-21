import { useState, useEffect } from "react";
import { PdpRuntimeApi, PdpRoutingApi } from "../../services/api";
import type { PdpRuntime, PdpRouteRule } from "../../services/api";

export function PdpRuntimeRouting() {
  const [activeTab, setActiveTab] = useState<
    "local" | "remote" | "cloud" | "routing"
  >("local");
  const [runtimes, setRuntimes] = useState<PdpRuntime[]>([]);
  const [routes, setRoutes] = useState<PdpRouteRule[]>([]);
  const [testResults, setTestResults] = useState<Record<string, any>>({});
  const [newRemoteName, setNewRemoteName] = useState("");
  const [newRemoteKind, setNewRemoteKind] = useState<
    "opa_server" | "openfga_server" | "cedar_http"
  >("opa_server");
  const [newRemoteUrl, setNewRemoteUrl] = useState("http://localhost:8181");

  // New Route form state
  const [newRouteName, setNewRouteName] = useState("");
  const [newRouteMode, setNewRouteMode] = useState<PdpRouteRule["mode"]>(
    "local_primary_remote_fallback",
  );
  const [newRoutePrimary, setNewRoutePrimary] = useState("");
  const [newRouteFallback, setNewRouteFallback] = useState("");
  const [newRouteFailure, setNewRouteFailure] = useState<
    PdpRouteRule["failure_behavior"]
  >("fallback" as any);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      const rtRes = await PdpRuntimeApi.list();
      setRuntimes(rtRes);
      const rrRes = await PdpRoutingApi.list();
      setRoutes(rrRes);
    } catch (e) {
      console.error(e);
    }
  };

  const handleTestRuntime = async (id: string) => {
    try {
      const res = await PdpRuntimeApi.probeHealth(id);
      setTestResults((prev) => ({ ...prev, [id]: res }));
    } catch (e) {
      console.error(e);
      setTestResults((prev) => ({
        ...prev,
        [id]: { ok: false, latency_ms: 0, detail: "error" },
      }));
    }
  };

  const handleAddRemote = async () => {
    if (!newRemoteUrl || !newRemoteName) return;
    try {
      await PdpRuntimeApi.upsert({
        id: `${newRemoteKind}-${Date.now()}`,
        name: newRemoteName,
        category: "external_connector",
        kind: newRemoteKind,
        enabled: true,
        status: "ready",
        endpoint: newRemoteUrl,
        capabilities: [],
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      });
      setNewRemoteName("");
      setNewRemoteUrl("");
      loadData();
    } catch (e) {
      console.error(e);
    }
  };

  const handleDeleteRuntime = async (id: string) => {
    if (!confirm("Are you sure you want to delete this remote connector?"))
      return;
    try {
      await PdpRuntimeApi.delete(id);
      loadData();
    } catch (e) {
      console.error(e);
    }
  };

  const handleAddRoute = async () => {
    if (!newRouteName || !newRoutePrimary) return;
    try {
      await PdpRoutingApi.upsert({
        id: `route-${Date.now()}`,
        name: newRouteName,
        enabled: true,
        priority: 100,
        match_cond: {},
        mode: newRouteMode,
        primary_pdp_id: newRoutePrimary,
        fallback_pdp_ids: newRouteFallback
          ? newRouteFallback.split(",").map((s) => s.trim())
          : [],
        shadow_pdp_ids: [],
        merge_strategy: "override",
        failure_behavior: newRouteFailure,
        timeout_ms: 200,
        max_retries: 0,
      });
      setNewRouteName("");
      setNewRoutePrimary("");
      setNewRouteFallback("");
      loadData();
    } catch (e) {
      console.error(e);
    }
  };

  const handleDeleteRoute = async (id: string) => {
    if (!confirm("Are you sure you want to delete this route?")) return;
    try {
      await PdpRoutingApi.delete(id);
      loadData();
    } catch (e) {
      console.error(e);
    }
  };

  const localRuntimes = runtimes.filter((r) => r.category === "local_engine");
  const remoteRuntimes = runtimes.filter(
    (r) => r.category === "external_connector",
  );

  return (
    <div className="glass p-6 rounded-xl space-y-6">
      <h3 className="text-lg font-medium">
        PDP Runtime & Routing Configuration
      </h3>

      <div className="flex border-b">
        <button
          className={`px-4 py-2 text-sm font-medium border-b-2 ${activeTab === "local" ? "border-primary text-primary" : "border-transparent text-muted-foreground hover:text-foreground"}`}
          onClick={() => setActiveTab("local")}
        >
          Local Engines
        </button>
        <button
          className={`px-4 py-2 text-sm font-medium border-b-2 ${activeTab === "remote" ? "border-primary text-primary" : "border-transparent text-muted-foreground hover:text-foreground"}`}
          onClick={() => setActiveTab("remote")}
        >
          Remote Connectors
        </button>
        <button
          className={`px-4 py-2 text-sm font-medium border-b-2 ${activeTab === "cloud" ? "border-primary text-primary" : "border-transparent text-muted-foreground hover:text-foreground"}`}
          onClick={() => setActiveTab("cloud")}
        >
          Pollen Cloud PDP
        </button>
        <button
          className={`px-4 py-2 text-sm font-medium border-b-2 ${activeTab === "routing" ? "border-primary text-primary" : "border-transparent text-muted-foreground hover:text-foreground"}`}
          onClick={() => setActiveTab("routing")}
        >
          Routing & Failover
        </button>
      </div>

      <div className="pt-4">
        {activeTab === "local" && (
          <div className="space-y-4">
            <p className="text-sm text-muted-foreground">
              These are built-in engines provided by the DEK. They are read-only
              and automatically managed.
            </p>
            <div className="rounded-md border">
              <table className="w-full text-sm text-left">
                <thead className="text-xs uppercase bg-muted/50">
                  <tr>
                    <th className="px-4 py-3">Name</th>
                    <th className="px-4 py-3">Kind</th>
                    <th className="px-4 py-3 text-right">Action</th>
                  </tr>
                </thead>
                <tbody>
                  {localRuntimes.map((c) => (
                    <tr key={c.id} className="border-b last:border-0">
                      <td className="px-4 py-3 font-medium">{c.name}</td>
                      <td className="px-4 py-3">
                        <span className="bg-primary/10 text-primary px-2 py-1 rounded text-xs">
                          {c.kind}
                        </span>
                      </td>
                      <td className="px-4 py-3 text-right">
                        <button
                          onClick={() => handleTestRuntime(c.id)}
                          className="px-3 py-1 bg-secondary text-secondary-foreground rounded text-xs hover:opacity-80"
                        >
                          Check Status
                        </button>
                        {testResults[c.id] && (
                          <div
                            className={`text-xs mt-1 ${testResults[c.id].ok ? "text-green-500" : "text-red-500"}`}
                          >
                            {testResults[c.id].ok ? "✓ ready" : "✗ error"}
                          </div>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        )}

        {activeTab === "remote" && (
          <div className="space-y-6">
            <p className="text-sm text-muted-foreground">
              Add third-party or custom external PDP servers to be used by the
              local DEK.
            </p>

            <div className="flex gap-2 max-w-2xl bg-muted/30 p-4 rounded-lg items-center">
              <input
                type="text"
                placeholder="Name"
                className="flex h-10 w-32 rounded-md border border-input bg-background px-3 py-2 text-sm"
                value={newRemoteName}
                onChange={(e) => setNewRemoteName(e.target.value)}
              />
              <select
                className="flex h-10 rounded-md border border-input bg-background px-3 py-2 text-sm w-40"
                value={newRemoteKind}
                onChange={(e) => {
                  setNewRemoteKind(e.target.value as any);
                  if (e.target.value === "opa_server")
                    setNewRemoteUrl("http://localhost:8181");
                  else if (e.target.value === "openfga_server")
                    setNewRemoteUrl("http://localhost:8080");
                  else if (e.target.value === "cedar_http")
                    setNewRemoteUrl("http://localhost:8081");
                }}
              >
                <option value="opa_server">OPA Server</option>
                <option value="openfga_server">OpenFGA</option>
                <option value="cedar_http">Cedar HTTP</option>
              </select>
              <input
                type="text"
                placeholder="http://localhost:8181"
                className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                value={newRemoteUrl}
                onChange={(e) => setNewRemoteUrl(e.target.value)}
              />
              <button
                onClick={handleAddRemote}
                className="h-10 px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm font-medium hover:opacity-90 whitespace-nowrap"
              >
                Add
              </button>
            </div>

            <div className="rounded-md border">
              <table className="w-full text-sm text-left">
                <thead className="text-xs uppercase bg-muted/50">
                  <tr>
                    <th className="px-4 py-3">Name / ID</th>
                    <th className="px-4 py-3">Kind</th>
                    <th className="px-4 py-3">Endpoint</th>
                    <th className="px-4 py-3 text-right">Action</th>
                  </tr>
                </thead>
                <tbody>
                  {remoteRuntimes.map((c) => (
                    <tr
                      key={c.id}
                      className="border-b last:border-0 hover:bg-muted/30"
                    >
                      <td className="px-4 py-3 font-medium">
                        {c.name}
                        <div className="text-xs text-muted-foreground mt-0.5">
                          {c.id}
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <span className="bg-secondary text-secondary-foreground px-2 py-1 rounded text-xs">
                          {c.kind}
                        </span>
                      </td>
                      <td className="px-4 py-3 font-mono text-xs">
                        {c.endpoint}
                      </td>
                      <td className="px-4 py-3 text-right">
                        <div className="flex items-center justify-end gap-2">
                          {testResults[c.id] && (
                            <span
                              className={`text-xs px-2 py-1 rounded ${testResults[c.id].ok ? "bg-green-500/10 text-green-500" : "bg-red-500/10 text-red-500"}`}
                            >
                              {testResults[c.id].ok
                                ? `✓ (${testResults[c.id].latency_ms}ms)`
                                : `✗ unreachable`}
                            </span>
                          )}
                          <button
                            onClick={() => handleTestRuntime(c.id)}
                            className="px-3 py-1 bg-secondary text-secondary-foreground rounded text-xs hover:opacity-80"
                          >
                            Probe
                          </button>
                          <button
                            onClick={() => handleDeleteRuntime(c.id)}
                            className="px-3 py-1 bg-red-500/10 text-red-500 rounded text-xs hover:opacity-80"
                          >
                            Delete
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                  {remoteRuntimes.length === 0 && (
                    <tr>
                      <td
                        colSpan={4}
                        className="px-4 py-8 text-center text-muted-foreground"
                      >
                        No remote connectors configured. Add one above.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </div>
        )}

        {activeTab === "cloud" && (
          <div className="py-8 text-center text-muted-foreground">
            <div className="inline-block p-4 bg-muted/50 rounded-full mb-4">
              <svg
                className="w-8 h-8 text-primary"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
                xmlns="http://www.w3.org/2000/svg"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M3 15a4 4 0 004 4h9a5 5 0 10-.1-9.999 5.002 5.002 0 10-9.78 2.096A4.001 4.001 0 003 15z"
                />
              </svg>
            </div>
            <h4 className="text-lg font-medium text-foreground mb-2">
              Pollen Cloud PDP
            </h4>
            <p className="max-w-md mx-auto text-sm">
              Connect this DEK to a fully managed Pollen Cloud PDP.
              Configuration and routing will be synced automatically.
            </p>
            <button
              disabled
              className="mt-6 px-4 py-2 bg-primary/50 text-primary-foreground rounded opacity-50 cursor-not-allowed"
            >
              Coming Soon
            </button>
          </div>
        )}

        {activeTab === "routing" && (
          <div className="space-y-4">
            <p className="text-sm text-muted-foreground">
              Manage how authorization requests are routed to the available PDP
              runtimes.
            </p>
            <div className="rounded-md border">
              <table className="w-full text-sm text-left">
                <thead className="text-xs uppercase bg-muted/50">
                  <tr>
                    <th className="px-4 py-3">Priority</th>
                    <th className="px-4 py-3">Name</th>
                    <th className="px-4 py-3">Mode</th>
                    <th className="px-4 py-3">Primary PDP</th>
                    <th className="px-4 py-3">Fallback</th>
                    <th className="px-4 py-3 text-right">Action</th>
                  </tr>
                </thead>
                <tbody>
                  {routes.map((r) => (
                    <tr
                      key={r.id}
                      className="border-b last:border-0 hover:bg-muted/30"
                    >
                      <td className="px-4 py-3 font-mono">{r.priority}</td>
                      <td className="px-4 py-3 font-medium">{r.name}</td>
                      <td className="px-4 py-3">
                        <span className="bg-primary/10 text-primary px-2 py-1 rounded text-xs">
                          {r.mode}
                        </span>
                      </td>
                      <td className="px-4 py-3 font-mono text-xs">
                        {r.primary_pdp_id}
                      </td>
                      <td className="px-4 py-3 font-mono text-xs">
                        {r.fallback_pdp_ids?.join(", ")}
                      </td>
                      <td className="px-4 py-3 text-right">
                        <button
                          onClick={() => handleDeleteRoute(r.id)}
                          className="px-3 py-1 bg-red-500/10 text-red-500 rounded text-xs hover:opacity-80"
                        >
                          Delete
                        </button>
                      </td>
                    </tr>
                  ))}
                  {routes.length === 0 && (
                    <tr>
                      <td
                        colSpan={6}
                        className="px-4 py-8 text-center text-muted-foreground"
                      >
                        No routes configured. Add one below.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>

            <div className="mt-8 pt-6 border-t">
              <h4 className="text-sm font-medium mb-4">Add New Route</h4>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4 bg-muted/20 p-4 rounded-lg">
                <div className="space-y-2">
                  <label className="text-xs font-medium">Route Name</label>
                  <input
                    type="text"
                    className="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm"
                    placeholder="e.g. Default Routing"
                    value={newRouteName}
                    onChange={(e) => setNewRouteName(e.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs font-medium">Routing Mode</label>
                  <select
                    className="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm"
                    value={newRouteMode}
                    onChange={(e) => setNewRouteMode(e.target.value as any)}
                  >
                    <option value="local_only">Local Only</option>
                    <option value="local_primary_remote_fallback">
                      Local Primary, Remote Fallback
                    </option>
                    <option value="remote_primary_local_fallback">
                      Remote Primary, Local Fallback
                    </option>
                    <option value="strict_remote">Strict Remote</option>
                  </select>
                </div>
                <div className="space-y-2">
                  <label className="text-xs font-medium">Primary PDP ID</label>
                  <input
                    type="text"
                    className="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm"
                    placeholder="e.g. local-cedar"
                    value={newRoutePrimary}
                    onChange={(e) => setNewRoutePrimary(e.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs font-medium">
                    Fallback PDP IDs (comma separated)
                  </label>
                  <input
                    type="text"
                    className="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm"
                    placeholder="e.g. remote-opa"
                    value={newRouteFallback}
                    onChange={(e) => setNewRouteFallback(e.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <label className="text-xs font-medium">
                    Failure Behavior
                  </label>
                  <select
                    className="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm"
                    value={newRouteFailure}
                    onChange={(e) => setNewRouteFailure(e.target.value as any)}
                  >
                    <option value="deny">Deny</option>
                    <option value="allow">Allow</option>
                    <option value="fallback">Fallback</option>
                  </select>
                </div>
                <div className="space-y-2 flex items-end">
                  <button
                    onClick={handleAddRoute}
                    className="h-9 px-4 py-2 bg-primary text-primary-foreground rounded-md text-sm font-medium hover:opacity-90 w-full"
                  >
                    Create Route
                  </button>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
