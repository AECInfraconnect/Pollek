import { useState, useEffect } from "react";
import { PlayCircle, CheckCircle2, XCircle } from "lucide-react";
import { PolicyApi } from "../services/api";
import type { PolicyDraft } from "../services/types";

export function Simulator() {
  const [policies, setPolicies] = useState<PolicyDraft[]>([]);
  const [selectedPolicyId, setSelectedPolicyId] = useState<string>("");
  const [action, setAction] = useState("read");
  const [resource, setResource] = useState("document:123");
  const [principal, setPrincipal] = useState("user:alice");
  const [contextStr, setContextStr] = useState('{\n  "device": "trusted"\n}');
  const [result, setResult] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [targetPep, setTargetPep] = useState<string>("auto");

  useEffect(() => {
    PolicyApi.list()
      .then((data) => {
        setPolicies(data);
        if (data.length > 0) {
          setSelectedPolicyId(data[0].policy_id);
        }
      })
      .catch(console.error);
  }, []);

  useEffect(() => {
    const policy = policies.find((p) => p.policy_id === selectedPolicyId);
    if (policy && policy.source?.kind === "raw_text" && policy.source.text) {
      const text = policy.source.text;

      if (
        policy.policy_type === "cedar" ||
        policy.source.language === "cedar"
      ) {
        const principalMatch = text.match(
          /principal\s*==\s*([A-Za-z0-9_]+::"[^"]+")/,
        );
        if (principalMatch) setPrincipal(principalMatch[1]);
        else setPrincipal('User::"alice"');

        const actionMatch = text.match(
          /action\s*==\s*([A-Za-z0-9_]+::"[^"]+")/,
        );
        if (actionMatch) setAction(actionMatch[1]);
        else setAction('Action::"read"');

        const resourceMatch = text.match(
          /resource\s*==\s*([A-Za-z0-9_]+::"[^"]+")/,
        );
        if (resourceMatch) setResource(resourceMatch[1]);
        else setResource('Document::"123"');

        const contextMatches = text.matchAll(
          /context\.([a-zA-Z0-9_]+)\s*==\s*"([^"]+)"/g,
        );
        const ctxObj: any = {};
        let foundContext = false;
        for (const match of contextMatches) {
          ctxObj[match[1]] = match[2];
          foundContext = true;
        }
        if (foundContext) {
          setContextStr(JSON.stringify(ctxObj, null, 2));
        } else {
          setContextStr("{}");
        }
      } else if (
        policy.policy_type === "rego" ||
        policy.source.language === "rego"
      ) {
        const actionMatch = text.match(/input\.action\s*==\s*"([^"]+)"/);
        if (actionMatch) setAction(actionMatch[1]);
        else setAction("read");

        setPrincipal("alice");
        setResource("document_123");
        setContextStr("{}");
      } else if (
        policy.policy_type === "open_fga" ||
        policy.source.language === "fga"
      ) {
        const relationMatch = text.match(/define\s+([a-zA-Z0-9_]+)\s*:/);
        if (relationMatch) setAction(relationMatch[1]);
        else setAction("viewer");

        setPrincipal("user:alice");
        setResource("document:123");
        setContextStr("{}");
      }
    }
  }, [selectedPolicyId, policies]);

  const handleSimulate = async () => {
    if (!selectedPolicyId) {
      setError("Please select a policy to simulate");
      return;
    }

    setLoading(true);
    setError(null);
    setResult(null);

    let ctx = {};
    try {
      ctx = JSON.parse(contextStr);
    } catch (e) {
      setError("Invalid JSON context");
      setLoading(false);
      return;
    }

    const payload = {
      action,
      resource,
      principal,
      context: ctx,
      target_pep: targetPep === "auto" ? undefined : targetPep,
    };

    try {
      const res = await PolicyApi.simulate(selectedPolicyId, payload);
      setResult(res);
    } catch (e: any) {
      setError(e.message || String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-6 max-w-4xl">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold tracking-tight flex items-center gap-2">
            <PlayCircle className="h-6 w-6 text-primary" /> Policy Simulator
          </h2>
          <p className="text-muted-foreground">
            Evaluate requests against a policy draft using dry-run mode.
          </p>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-6">
        <div className="glass p-6 rounded-xl space-y-4 border">
          <h3 className="font-medium">Request Context</h3>

          <div className="space-y-3">
            <div>
              <label className="text-xs font-medium text-muted-foreground">
                Target Policy
              </label>
              <select
                value={selectedPolicyId}
                onChange={(e) => setSelectedPolicyId(e.target.value)}
                className="mt-1 w-full rounded-md border bg-background px-3 py-2 text-sm"
              >
                {policies.length === 0 && (
                  <option value="">No policies available</option>
                )}
                {policies.map((p) => (
                  <option key={p.policy_id} value={p.policy_id}>
                    {p.name} ({p.policy_id})
                  </option>
                ))}
              </select>
              {selectedPolicyId &&
                policies.find((p) => p.policy_id === selectedPolicyId) && (
                  <div className="mt-2 text-[11px] px-2 py-1.5 rounded-md bg-blue-500/10 text-blue-400 border border-blue-500/20 flex items-center gap-1.5">
                    <span className="font-semibold">
                      Recommended Deployment:
                    </span>
                    {policies.find((p) => p.policy_id === selectedPolicyId)
                      ?.policy_type === "cedar" &&
                      "MCP Proxy PEP, Envoy Proxy, STDIO Wrapper"}
                    {policies.find((p) => p.policy_id === selectedPolicyId)
                      ?.policy_type === "rego" && "Envoy / L7 Proxy PEP"}
                    {policies.find((p) => p.policy_id === selectedPolicyId)
                      ?.policy_type === "open_fga" && "Envoy / L7 Proxy PEP"}
                    {!["cedar", "rego", "open_fga"].includes(
                      policies.find((p) => p.policy_id === selectedPolicyId)
                        ?.policy_type || "",
                    ) && "Gateway PEP"}
                  </div>
                )}
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground">
                Target PEP for Deployment Test
              </label>
              <select
                value={targetPep}
                onChange={(e) => setTargetPep(e.target.value)}
                className="mt-1 w-full rounded-md border bg-background px-3 py-2 text-sm"
              >
                <option value="auto">Auto-Detect (Default)</option>
                <option value="mcp_proxy">MCP Proxy PEP</option>
                <option value="ext_authz">Envoy / L7 Proxy PEP</option>
                <option value="stdio_wrapper">STDIO Agent Wrapper</option>
              </select>
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground">
                Principal ID
              </label>
              <input
                value={principal}
                onChange={(e) => setPrincipal(e.target.value)}
                className="mt-1 w-full rounded-md border bg-background px-3 py-2 text-sm"
              />
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground">
                Action
              </label>
              <input
                value={action}
                onChange={(e) => setAction(e.target.value)}
                className="mt-1 w-full rounded-md border bg-background px-3 py-2 text-sm"
              />
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground">
                Resource ID
              </label>
              <input
                value={resource}
                onChange={(e) => setResource(e.target.value)}
                className="mt-1 w-full rounded-md border bg-background px-3 py-2 text-sm"
              />
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground">
                Additional Context (JSON)
              </label>
              <textarea
                value={contextStr}
                onChange={(e) => setContextStr(e.target.value)}
                rows={5}
                className="mt-1 w-full rounded-md border bg-black/30 px-3 py-2 font-mono text-xs"
                spellCheck={false}
              />
            </div>

            {error && (
              <div className="text-xs text-red-400 p-2 bg-red-400/10 rounded">
                {error}
              </div>
            )}

            <button
              onClick={handleSimulate}
              disabled={loading || policies.length === 0}
              className="w-full flex justify-center items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              <PlayCircle className="h-4 w-4" />
              {loading ? "Simulating..." : "Run Simulation"}
            </button>
          </div>
        </div>

        <div className="glass p-6 rounded-xl space-y-4 border h-full flex flex-col">
          <h3 className="font-medium">Decision Result</h3>

          <div className="flex-1 bg-black/40 rounded-lg p-4 font-mono text-xs overflow-auto border border-white/5 relative">
            {!result && !loading && (
              <div className="absolute inset-0 flex items-center justify-center text-muted-foreground">
                Run a simulation to see the result
              </div>
            )}
            {loading && (
              <div className="absolute inset-0 flex items-center justify-center text-muted-foreground animate-pulse">
                Evaluating policy...
              </div>
            )}
            {result && (
              <div className="space-y-4">
                <div className="flex items-center gap-2 text-lg">
                  {result.decision !== "Not Evaluated" &&
                    (result.allowed ? (
                      <span className="flex items-center gap-2 text-emerald-400">
                        <CheckCircle2 className="h-5 w-5" /> ALLOW
                      </span>
                    ) : (
                      <span className="flex items-center gap-2 text-red-400">
                        <XCircle className="h-5 w-5" /> DENY
                      </span>
                    ))}
                  {result.decision === "Not Evaluated" && (
                    <span className="flex items-center gap-2 text-slate-400">
                      <PlayCircle className="h-5 w-5" /> SIMULATION ONLY
                    </span>
                  )}
                </div>

                <div className="grid grid-cols-2 gap-4 text-sm mt-4 border-t border-white/10 pt-4">
                  <div>
                    <div className="text-muted-foreground mb-1">
                      Syntax Check
                    </div>
                    <div
                      className={
                        result.syntax_check?.startsWith("Passed")
                          ? "text-emerald-400"
                          : "text-red-400 font-semibold"
                      }
                    >
                      {result.syntax_check || "N/A"}
                    </div>
                  </div>
                  <div>
                    <div className="text-muted-foreground mb-1">
                      Deployment Test
                    </div>
                    <div
                      className={
                        result.deployment_test?.startsWith("Passed")
                          ? "text-emerald-400"
                          : "text-red-400 font-semibold"
                      }
                    >
                      {result.deployment_test || "N/A"}
                    </div>
                  </div>
                  <div className="col-span-2">
                    <div className="text-muted-foreground mb-1">
                      Recommended PEP Target
                    </div>
                    <div className="text-blue-400 inline-block px-3 py-1 rounded bg-blue-500/10 border border-blue-500/20">
                      {result.recommended_pep || "N/A"}
                    </div>
                  </div>
                </div>

                <div className="text-muted-foreground whitespace-pre-wrap mt-4 border-t border-white/10 pt-4">
                  <div className="text-xs mb-2">Raw Response:</div>
                  {JSON.stringify(result, null, 2)}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
