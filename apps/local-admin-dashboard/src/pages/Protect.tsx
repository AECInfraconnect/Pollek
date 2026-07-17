import { useMode } from "../context/ModeContext";
import { PageHeader } from "../components/layout/PageHeader";
import { SimplePolicyWizard } from "../components/simple/SimplePolicyWizard";
import { Activity, ShieldCheck } from "lucide-react";
import { useSearchParams } from "react-router-dom";
import { useState, useEffect } from "react";
import { RegistryApi } from "../services/api";

export function Protect() {
  const { mode } = useMode();
  const [params] = useSearchParams();
  const agentId = params.get("agent") || params.get("agent_id") || undefined;
  const sourceEventId = params.get("event") || undefined;
  const sourceTarget = params.get("target") || undefined;
  const sourceIntent = params.get("intent") || undefined;

  const [agents, setAgents] = useState<{ id: string; label: string }[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    RegistryApi.listAgents()
      .then((res) => {
        setAgents(
          res.map((a) => ({ id: a.agent_id, label: a.name || a.agent_id })),
        );
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  return (
    <div className="space-y-6">
      <PageHeader
        className="border-b pb-4"
        icon={ShieldCheck}
        title={
          mode === "desktop_simple" ? "Create AI Activity Rule" : "Create Rule"
        }
        subtitle={
          mode === "desktop_simple"
            ? "Choose what an AI app can watch, ask about, or block — no policy terms to learn."
            : "Create a user-friendly rule first, then inspect the generated policy and control plan when needed."
        }
      />

      <div className="mt-8">
        {(sourceEventId || sourceTarget || sourceIntent) && (
          <section className="mb-5 rounded-lg border border-blue-500/20 bg-blue-500/10 p-4">
            <div className="flex items-start gap-3">
              <div className="rounded-lg bg-blue-500/15 p-2 text-blue-700 dark:text-blue-200">
                <Activity className="h-4 w-4" />
              </div>
              <div>
                <h3 className="text-sm font-semibold">
                  Rule draft from activity
                </h3>
                <p className="mt-1 text-sm leading-6 text-muted-foreground">
                  Pollek is starting this rule from the selected activity event.
                  Review what can really be watched, asked about, or blocked
                  before saving.
                </p>
                <div className="mt-3 flex flex-wrap gap-2 text-xs">
                  {sourceTarget && (
                    <span className="rounded-md border bg-background/70 px-2 py-1">
                      Target: {sourceTarget}
                    </span>
                  )}
                  {sourceIntent && (
                    <span className="rounded-md border bg-background/70 px-2 py-1">
                      Intent: {sourceIntent.replace(/_/g, " ")}
                    </span>
                  )}
                  {sourceEventId && (
                    <span className="rounded-md border bg-background/70 px-2 py-1">
                      Event: {sourceEventId}
                    </span>
                  )}
                </div>
              </div>
            </div>
          </section>
        )}
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
