import { useState, useEffect } from "react";
import { Package, RefreshCw, CheckCircle, Clock, XCircle } from "lucide-react";
import { BundleApi } from "../services/api";

export function Bundles() {
  const [bundles, setBundles] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showDialog, setShowDialog] = useState(false);
  const [showErrorDialog, setShowErrorDialog] = useState(false);
  const [errorMessage, setErrorMessage] = useState("");
  const [syncResult, setSyncResult] = useState<any>(null);

  const load = async () => {
    setLoading(true);
    try {
      const data = await BundleApi.list();
      setBundles(Array.isArray(data) ? data : []);
    } catch (e: any) {
      console.warn("Bundles endpoint issue:", e);
      setBundles([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const handleSync = async () => {
    setSyncing(true);
    setError(null);
    try {
      const res = await BundleApi.sync();
      setSyncResult(res);
      setShowDialog(true);
      await load();
    } catch (e: any) {
      setError(`Sync failed: ${e.message || String(e)}`);
    } finally {
      setSyncing(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight flex items-center gap-2">
            <Package className="h-6 w-6 text-primary" /> Bundles &amp;
            Deployments
          </h2>
          <p className="text-muted-foreground">
            Manage deployed policy bundles and synchronize with the control
            plane.
          </p>
        </div>
        <button
          onClick={handleSync}
          disabled={syncing}
          className="flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50 transition-colors shadow-lg shadow-primary/20"
        >
          <RefreshCw className={`h-4 w-4 ${syncing ? "animate-spin" : ""}`} />
          {syncing ? "Syncing..." : "Sync Now"}
        </button>
      </div>

      {error && (
        <div className="rounded-md bg-red-500/10 px-4 py-3 text-sm text-red-400 border border-red-500/20">
          {error}
        </div>
      )}

      <div className="glass rounded-xl overflow-hidden border">
        <table className="w-full text-sm text-left">
          <thead className="bg-muted/50 text-muted-foreground">
            <tr>
              <th className="px-6 py-4 font-medium">Bundle ID</th>
              <th className="px-6 py-4 font-medium">Version</th>
              <th className="px-6 py-4 font-medium">Status</th>
              <th className="px-6 py-4 font-medium">Deployed At</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {loading ? (
              <tr>
                <td
                  colSpan={4}
                  className="px-6 py-8 text-center text-muted-foreground"
                >
                  Loading deployments...
                </td>
              </tr>
            ) : bundles.length === 0 ? (
              <tr>
                <td
                  colSpan={4}
                  className="px-6 py-8 text-center text-muted-foreground"
                >
                  No bundles found or endpoint unavailable in this profile.
                </td>
              </tr>
            ) : (
              bundles.map((b, i) => (
                <tr
                  key={b.bundle_id || i}
                  className="hover:bg-muted/30 transition-colors"
                >
                  <td className="px-6 py-4 font-medium font-mono text-xs">
                    {b.bundle_id || "unknown"}
                  </td>
                  <td className="px-6 py-4 text-muted-foreground">
                    {b.version || "v1.0"}
                  </td>
                  <td className="px-6 py-4">
                    <span
                      className={`inline-flex items-center gap-1.5 rounded-full px-2 py-1 text-xs font-medium ${
                        i === 0
                          ? "bg-emerald-500/10 text-emerald-500"
                          : "bg-muted text-muted-foreground"
                      }`}
                    >
                      {i === 0 ? (
                        <CheckCircle className="h-3 w-3" />
                      ) : (
                        <Clock className="h-3 w-3" />
                      )}
                      {i === 0 ? "Active" : "Archived"}
                    </span>
                  </td>
                  <td className="px-6 py-4 text-muted-foreground">
                    {b.deployed_at
                      ? new Date(b.deployed_at).toLocaleString()
                      : "Just now"}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      <div className="mt-8 space-y-4">
        <div>
          <h3 className="text-xl font-semibold tracking-tight">
            Local PEP Deployments
          </h3>
          <p className="text-sm text-muted-foreground mt-1">
            Deploy policies directly to Personal Enforcement Points running on
            your local machine.
          </p>
        </div>

        <div className="grid grid-cols-1 gap-4 md:grid-cols-3">
          {[
            {
              id: "mcp_proxy",
              name: "MCP Proxy PEP",
              desc: "Enforces policies on AI Agents and MCP servers accessing local tools and resources.",
              cmd: "dek-mcp-proxy",
            },
            {
              id: "ext_authz",
              name: "Envoy / L7 Proxy PEP",
              desc: "Enforces policies on network traffic, external APIs, and internet resources.",
              cmd: "dek-ext-authz",
            },
            {
              id: "stdio_wrapper",
              name: "STDIO Agent Wrapper",
              desc: "Enforces policies on standalone CLI agents communicating via standard input/output.",
              cmd: "dek-mcp-stdio-wrapper",
            },
          ].map((pep) => (
            <div
              key={pep.id}
              className="rounded-xl border bg-card text-card-foreground shadow-sm flex flex-col"
            >
              <div className="p-6 pb-4 flex flex-col space-y-1.5 flex-grow">
                <h3 className="font-semibold leading-none tracking-tight">
                  {pep.name}
                </h3>
                <p className="text-sm text-muted-foreground flex-grow">
                  {pep.desc}
                </p>
              </div>
              <div className="p-6 pt-0 space-y-4">
                <div className="space-y-2">
                  <label className="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70">
                    Select Bundle
                  </label>
                  <select
                    id={`bundle-${pep.id}`}
                    className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
                  >
                    <option value="bundle:latest">bundle:latest</option>
                    {bundles
                      .filter((b) => b.bundle_id)
                      .map((b) => (
                        <option key={b.bundle_id} value={b.bundle_id}>
                          {b.bundle_id}
                        </option>
                      ))}
                  </select>
                </div>
                <button
                  onClick={async () => {
                    const select = document.getElementById(
                      `bundle-${pep.id}`,
                    ) as HTMLSelectElement;
                    const bundleId = select.value;
                    try {
                      const res = await BundleApi.deployToPep(pep.id, bundleId);
                      setSyncResult({
                        message: `Deployed ${bundleId} to ${pep.name}.`,
                        bundle_id: bundleId,
                        timestamp: (res as any).timestamp,
                      });
                      setShowDialog(true);
                    } catch (e: any) {
                      setErrorMessage(e.message || String(e));
                      setShowErrorDialog(true);
                      setError(`Deploy to ${pep.name} failed: ${e.message}`);
                    }
                  }}
                  className="inline-flex w-full items-center justify-center whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80 h-9 px-4 py-2"
                >
                  Deploy to PEP
                </button>
                <div className="rounded-md bg-muted/50 p-3 text-[10px] font-mono break-all text-muted-foreground">
                  $env:DEK_BUNDLE_PATH="C:\ProgramData\PollenDEK\state\pep_
                  {pep.id}\active_bundle.json"
                  <br />
                  cargo run -p {pep.cmd}
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      {showDialog && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
          <div className="w-full max-w-md rounded-xl border bg-card p-6 shadow-2xl animate-in fade-in zoom-in duration-200">
            <div className="mb-4 flex items-center gap-3">
              <div className="rounded-full bg-emerald-500/20 p-2 text-emerald-500">
                <CheckCircle className="h-6 w-6" />
              </div>
              <h3 className="text-xl font-semibold">Deployment Successful</h3>
            </div>
            <p className="mb-6 text-sm text-muted-foreground">
              {syncResult?.message ||
                "Policy bundles have been successfully synchronized and deployed to connected PEPs."}
            </p>
            <div className="mb-6 rounded-md bg-muted/50 p-3 text-xs font-mono">
              Bundle ID: {syncResult?.bundle_id || "unknown"}
              <br />
              Timestamp:{" "}
              {syncResult?.timestamp
                ? new Date(syncResult.timestamp).toLocaleString()
                : new Date().toLocaleString()}
            </div>
            <div className="flex justify-end">
              <button
                onClick={() => setShowDialog(false)}
                className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
              >
                Done
              </button>
            </div>
          </div>
        </div>
      )}

      {showErrorDialog && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
          <div className="w-full max-w-md rounded-xl border border-red-500/20 bg-card p-6 shadow-2xl animate-in fade-in zoom-in duration-200">
            <div className="mb-4 flex items-center gap-3">
              <div className="rounded-full bg-red-500/20 p-2 text-red-500">
                <XCircle className="h-6 w-6" />
              </div>
              <h3 className="text-xl font-semibold">Deployment Failed</h3>
            </div>
            <p className="mb-6 text-sm text-red-400">{errorMessage}</p>
            <div className="flex justify-end">
              <button
                onClick={() => setShowErrorDialog(false)}
                className="rounded-md bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-700 transition-colors"
              >
                Close
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
