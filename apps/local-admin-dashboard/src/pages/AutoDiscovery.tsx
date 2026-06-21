import { useState, useEffect } from "react";
import { RefreshCw, Search, ShieldAlert, CheckCircle } from "lucide-react";
import { RegistryApi } from "../services/api";
import type {
  DiscoveredAgentCandidateV2,
  DiscoveryScanJob,
} from "../services/types";

export function AutoDiscovery() {
  const [activeTab, setActiveTab] = useState("candidates");
  const [candidates, setCandidates] = useState<DiscoveredAgentCandidateV2[]>(
    [],
  );
  const [loading, setLoading] = useState(false);
  const [loadingCandidates, setLoadingCandidates] = useState(true);
  const [scanJob, setScanJob] = useState<DiscoveryScanJob | null>(null);
  const [showModal, setShowModal] = useState(false);
  const [scanType, setScanType] = useState("deep");
  const [privacyMode, setPrivacyMode] = useState(true);
  const [scanHistory, setScanHistory] = useState<DiscoveryScanJob[]>([]);

  const fetchCandidates = () => {
    setLoadingCandidates(true);
    RegistryApi.listDiscoveryCandidates()
      .then(setCandidates)
      .catch(console.error)
      .finally(() => setLoadingCandidates(false));
  };

  useEffect(() => {
    fetchCandidates();
  }, []);

  useEffect(() => {
    let interval: ReturnType<typeof setInterval>;
    if (
      scanJob &&
      (scanJob.status === "queued" || scanJob.status === "running")
    ) {
      interval = setInterval(async () => {
        try {
          const status = await RegistryApi.getDiscoveryScanStatus(
            scanJob.scan_id,
          );
          setScanJob(status);
          if (
            status.status === "completed" ||
            status.status === "partial" ||
            status.status === "failed"
          ) {
            setLoading(false);
            fetchCandidates();
            clearInterval(interval);
          }
        } catch (e) {
          console.error(e);
        }
      }, 2000);
    }
    return () => clearInterval(interval);
  }, [scanJob]);

  useEffect(() => {
    if (activeTab === "history") {
      RegistryApi.listDiscoveryScans()
        .then(setScanHistory)
        .catch(console.error);
    }
  }, [activeTab]);

  const triggerScan = () => {
    setShowModal(true);
  };

  const confirmScan = async () => {
    setShowModal(false);
    setLoading(true);
    try {
      const sources =
        scanType === "quick"
          ? ["process", "mcp_config"]
          : [
              "process",
              "mcp_config",
              "local_model",
              "ide_extension",
              "cli_agent",
              "container",
              "browser_extension",
            ];
      const result = await RegistryApi.triggerDiscoveryScan({
        sources,
        privacy_mode: privacyMode,
      });
      setScanJob({
        scan_id: result.scan_id,
        tenant_id: "local",
        status: result.status as any,
        sources: sources,
        candidates_found: 0,
      });
    } catch (e) {
      console.error(e);
      setLoading(false);
    }
  };

  const cancelScan = async () => {
    if (scanJob?.scan_id) {
      try {
        await RegistryApi.cancelDiscoveryScan(scanJob.scan_id);
      } catch (e) {
        console.error(e);
      }
    }
    setLoading(false);
    setScanJob((prev) =>
      prev ? { ...prev, status: "cancelled" as any } : null,
    );
  };

  const handleRegister = async (candidate: DiscoveredAgentCandidateV2) => {
    try {
      await RegistryApi.registerDiscoveryCandidate(candidate.candidate_id, {
        agent_name: candidate.suggested_registration.name,
      });
      alert(`Successfully registered ${candidate.suggested_registration.name}`);
      fetchCandidates();
    } catch (err) {
      console.error(err);
      alert("Failed to register: " + err);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">Auto Discovery</h2>
          <p className="text-muted-foreground">
            Find and manage local AI agents, MCP servers, and model endpoints.
          </p>
        </div>
        {loading ? (
          <button
            onClick={cancelScan}
            className="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 ring-offset-background bg-red-500 text-white hover:bg-red-600 h-10 py-2 px-4 shadow-lg"
          >
            <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
            Stop Scan
          </button>
        ) : (
          <button
            onClick={triggerScan}
            className="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 ring-offset-background bg-primary text-primary-foreground hover:bg-primary/90 h-10 py-2 px-4 shadow-lg shadow-primary/20"
          >
            <Search className="mr-2 h-4 w-4" />
            Deep Scan
          </button>
        )}
      </div>

      {scanJob && (
        <div className="p-4 border rounded-md bg-muted/20">
          <p className="text-sm font-medium">
            Scan Status: <span className="uppercase">{scanJob.status}</span>
          </p>
          <p className="text-xs text-muted-foreground">
            Scan ID: {scanJob.scan_id}
          </p>
          {scanJob.error && (
            <p className="text-xs text-red-500">Error: {scanJob.error}</p>
          )}
        </div>
      )}

      <div className="border-b border-border">
        <nav className="-mb-px flex space-x-6">
          {["candidates", "control_plans", "sources", "history"].map((tab) => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              className={
                "whitespace-nowrap pb-4 px-1 border-b-2 font-medium text-sm " +
                (activeTab === tab
                  ? "border-primary text-foreground"
                  : "border-transparent text-muted-foreground hover:text-foreground hover:border-border")
              }
            >
              {tab.charAt(0).toUpperCase() + tab.slice(1).replace("_", " ")}
            </button>
          ))}
        </nav>
      </div>

      <div className="glass rounded-xl p-6">
        {activeTab === "candidates" && (
          <div>
            <h3 className="font-semibold mb-4">Discovered Agents</h3>
            {loadingCandidates ? (
              <div className="flex h-[200px] items-center justify-center rounded-md border border-dashed border-muted">
                <p className="text-sm text-muted-foreground">
                  Loading candidates...
                </p>
              </div>
            ) : candidates.length === 0 ? (
              <div className="flex h-[200px] items-center justify-center rounded-md border border-dashed border-muted">
                <p className="text-sm text-muted-foreground">
                  No discovered agents yet. Click "Deep Scan" to begin.
                </p>
              </div>
            ) : (
              <div className="space-y-4">
                {candidates.map((c, idx) => (
                  <div
                    key={idx}
                    className="border rounded-lg p-4 hover:bg-muted/30 transition-colors"
                  >
                    <div className="flex justify-between items-start mb-2">
                      <div className="flex items-center gap-2">
                        <ShieldAlert className="h-5 w-5 text-primary" />
                        <h4 className="font-medium text-lg">
                          {c.display_name}{" "}
                          <span className="text-xs text-muted-foreground">
                            ({c.inferred_agent_type})
                          </span>
                        </h4>
                      </div>
                      <div className="flex gap-2">
                        {c.status === "registered" ? (
                          <span className="inline-flex items-center gap-1 text-xs text-green-500 font-medium px-2 py-1">
                            <CheckCircle className="w-3 h-3" /> Registered
                          </span>
                        ) : (
                          <button
                            onClick={() => handleRegister(c)}
                            className="text-xs border px-3 py-1.5 rounded hover:bg-primary hover:text-primary-foreground font-medium"
                          >
                            Register Agent
                          </button>
                        )}
                      </div>
                    </div>
                    <p className="text-muted-foreground text-sm">
                      Risk Score: {c.risk_score} | Confidence:{" "}
                      {(c.confidence * 100).toFixed(0)}% <br />
                      Candidate ID: {c.candidate_id} <br />
                      First seen: {new Date(c.first_seen).toLocaleString()}{" "}
                      <br />
                    </p>

                    {c.discovered_mcp_servers &&
                      c.discovered_mcp_servers.length > 0 && (
                        <div className="mt-3 p-3 bg-muted/40 rounded-md">
                          <h5 className="text-xs font-semibold mb-1">
                            Discovered MCP Servers:
                          </h5>
                          <ul className="text-xs space-y-1">
                            {c.discovered_mcp_servers.map(
                              (mcp: any, i: number) => (
                                <li key={i}>
                                  - {mcp.server_name} ({mcp.transport})
                                </li>
                              ),
                            )}
                          </ul>
                        </div>
                      )}
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {activeTab === "control_plans" && (
          <div>
            <h3 className="font-semibold mb-4">Control Binding Plans</h3>
            <p className="text-sm text-muted-foreground mb-4">
              Review suggested security bindings for discovered agents before
              enforcing them.
            </p>

            {candidates.filter(
              (c) =>
                c.suggested_control_bindings &&
                c.suggested_control_bindings.length > 0,
            ).length === 0 ? (
              <div className="flex h-[150px] items-center justify-center rounded-md border border-dashed border-muted">
                <p className="text-sm text-muted-foreground">
                  No suggested control bindings found.
                </p>
              </div>
            ) : (
              <div className="space-y-4">
                {candidates
                  .filter(
                    (c) =>
                      c.suggested_control_bindings &&
                      c.suggested_control_bindings.length > 0,
                  )
                  .map((c, idx) => (
                    <div
                      key={idx}
                      className="border rounded-lg overflow-hidden"
                    >
                      <div className="bg-muted/30 p-3 border-b">
                        <h4 className="font-medium">
                          {c.display_name}{" "}
                          <span className="text-xs text-muted-foreground font-normal">
                            ({c.candidate_id})
                          </span>
                        </h4>
                      </div>
                      <div className="p-4 space-y-3">
                        {c.suggested_control_bindings.map(
                          (plan: any, planIdx: number) => (
                            <div
                              key={planIdx}
                              className="flex items-center justify-between p-3 border rounded border-primary/20 bg-primary/5"
                            >
                              <div>
                                <p className="font-medium text-sm">
                                  {plan.summary}
                                </p>
                                <p className="text-xs text-muted-foreground">
                                  Action: {plan.action} | Kind: {plan.kind}
                                </p>
                              </div>
                              <button className="text-xs bg-primary text-primary-foreground px-3 py-1.5 rounded font-medium shadow-sm hover:opacity-90">
                                Apply Binding
                              </button>
                            </div>
                          ),
                        )}
                      </div>
                    </div>
                  ))}
              </div>
            )}
          </div>
        )}

        {activeTab === "sources" && (
          <div>
            <h3 className="font-semibold mb-4">Discovery Evidence (Sources)</h3>
            <p className="text-sm text-muted-foreground mb-4">
              Raw telemetry collected by scanners.
            </p>
            {candidates.length === 0 ? (
              <div className="flex h-[150px] items-center justify-center rounded-md border border-dashed border-muted">
                <p className="text-sm text-muted-foreground">
                  No evidence collected.
                </p>
              </div>
            ) : (
              <div className="space-y-4">
                {candidates.map((c) =>
                  c.evidence.map((ev, i) => (
                    <div
                      key={`${c.candidate_id}-${i}`}
                      className="border rounded p-3 text-sm flex flex-col gap-1 font-mono bg-muted/10"
                    >
                      <div className="flex justify-between items-center text-xs">
                        <span className="font-bold text-primary">
                          {ev.source}
                        </span>
                        <span className="text-muted-foreground">
                          {ev.observed_at}
                        </span>
                      </div>
                      <div className="text-xs text-muted-foreground">
                        Privacy Class: {ev.privacy_class}
                      </div>
                      <pre className="mt-2 text-xs overflow-x-auto p-2 bg-muted rounded border">
                        {JSON.stringify(ev.data, null, 2)}
                      </pre>
                    </div>
                  )),
                )}
              </div>
            )}
          </div>
        )}

        {activeTab === "history" && (
          <div>
            <h3 className="font-semibold mb-4">Scan History</h3>
            {scanHistory.length === 0 ? (
              <div className="flex h-[200px] items-center justify-center rounded-md border border-dashed border-muted">
                <p className="text-sm text-muted-foreground">
                  No scans have been performed yet.
                </p>
              </div>
            ) : (
              <div className="space-y-3">
                {scanHistory
                  .sort(
                    (a, b) =>
                      new Date(b.started_at || 0).getTime() -
                      new Date(a.started_at || 0).getTime(),
                  )
                  .map((job) => (
                    <div
                      key={job.scan_id}
                      className="border rounded p-3 text-sm flex justify-between items-center bg-muted/10"
                    >
                      <div>
                        <div className="font-medium">{job.scan_id}</div>
                        <div className="text-xs text-muted-foreground">
                          Sources: {job.sources.join(", ")}
                        </div>
                      </div>
                      <div className="text-right">
                        <div
                          className={
                            "font-medium uppercase " +
                            (job.status === "failed"
                              ? "text-red-500"
                              : job.status === "cancelled"
                                ? "text-yellow-500"
                                : "text-primary")
                          }
                        >
                          {job.status}
                        </div>
                        <div className="text-xs text-muted-foreground">
                          {job.started_at
                            ? new Date(job.started_at).toLocaleString()
                            : ""}
                        </div>
                      </div>
                    </div>
                  ))}
              </div>
            )}
          </div>
        )}
      </div>

      {showModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="bg-background border rounded-xl shadow-xl w-full max-w-md p-6">
            <h3 className="text-xl font-bold mb-4">Start Discovery Scan</h3>
            <div className="space-y-4">
              <label className="flex items-start gap-3 p-3 border rounded cursor-pointer hover:bg-muted/20">
                <input
                  type="radio"
                  name="scan_type"
                  value="quick"
                  checked={scanType === "quick"}
                  onChange={() => setScanType("quick")}
                  className="mt-1"
                />
                <div>
                  <div className="font-medium">Quick Scan</div>
                  <div className="text-xs text-muted-foreground">
                    Process scan and MCP config only. High confidence.
                  </div>
                </div>
              </label>
              <label className="flex items-start gap-3 p-3 border rounded cursor-pointer hover:bg-muted/20">
                <input
                  type="radio"
                  name="scan_type"
                  value="deep"
                  checked={scanType === "deep"}
                  onChange={() => setScanType("deep")}
                  className="mt-1"
                />
                <div>
                  <div className="font-medium">Deep Scan</div>
                  <div className="text-xs text-muted-foreground">
                    Includes IDE extensions, CLI tools, and Local Model servers.
                  </div>
                </div>
              </label>
              <div className="flex items-center gap-2 pt-2">
                <input
                  type="checkbox"
                  id="privacy"
                  checked={privacyMode}
                  onChange={(e) => setPrivacyMode(e.target.checked)}
                />
                <label htmlFor="privacy" className="text-sm">
                  Redact sensitive paths locally (Privacy Mode)
                </label>
              </div>
            </div>
            <div className="mt-6 flex justify-end gap-3">
              <button
                onClick={() => setShowModal(false)}
                className="px-4 py-2 text-sm border rounded hover:bg-muted"
              >
                Cancel
              </button>
              <button
                onClick={confirmScan}
                className="px-4 py-2 text-sm bg-primary text-primary-foreground rounded hover:bg-primary/90"
              >
                Start Scan
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
