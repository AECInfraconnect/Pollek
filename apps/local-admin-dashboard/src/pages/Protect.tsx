import { useMode } from "../context/ModeContext";
import { SimplePolicyWizard } from "../components/simple/SimplePolicyWizard";
import { ShieldCheck } from "lucide-react";
import { useSearchParams } from "react-router-dom";
import { useState, useEffect } from "react";
import { RegistryApi } from "../services/api";

export function Protect() {
  const { mode } = useMode();
  const [params] = useSearchParams();
  const agentId = params.get("agent") || undefined;

  const [agents, setAgents] = useState<{ id: string; label: string }[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    RegistryApi.listAgents()
      .then((res) => {
        setAgents(
          res.map((a) => ({ id: a.agent_id, label: a.name || a.agent_id }))
        );
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between border-b pb-4">
        <div>
          <h2 className="flex items-center gap-2 text-lg font-semibold tracking-tight">
            <ShieldCheck className="h-6 w-6 text-primary" />
            {mode === "desktop_simple"
              ? "Protect Agents"
              : "Advance Protection"}
          </h2>
          <p className="mt-1 text-muted-foreground">
            {mode === "desktop_simple"
              ? "Deploy guardrails in 3 easy steps. The system will handle the rest."
              : "Deploy guardrails with automatic feasibility planning and method selection."}
          </p>
        </div>
      </div>

      <div className="mt-8">
        {!loading && (
          <SimplePolicyWizard
            agents={
              agents.length
                ? agents
                : agentId
                  ? [{ id: agentId, label: "Loading..." }]
                  : []
            }
            initialTarget={agentId}
          />
        )}
      </div>
    </div>
  );
}
