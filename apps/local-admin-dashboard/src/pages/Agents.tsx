import { toast } from "sonner";
import { useState, useEffect } from "react";
import { Plus, Users, Cpu, Info } from "lucide-react";
import { useSearchParams, useNavigate } from "react-router-dom";
import { RegistryApi } from "../services/api";
import type { AiAgent } from "../services/api";
import { MasterDetailLayout } from "../components/master-detail/MasterDetailLayout";
import { EntityCard } from "../components/master-detail/EntityCard";
import { DetailPane } from "../components/master-detail/DetailPane";
import { EmptyState } from "../components/master-detail/EmptyState";
import type { UiStatus } from "../lib/status";
import { useConfirm } from "../components/ui/ConfirmDialog";
import { AgentEnforcementTab } from "../components/agents/AgentEnforcementTab";
import { AgentActivityTab } from "../components/agents/AgentActivityTab";

function SummaryMetric({
  label,
  value,
  helper,
}: {
  label: string;
  value: React.ReactNode;
  helper?: string;
}) {
  return (
    <div className="p-4 bg-muted/30 rounded-xl border">
      <span className="text-muted-foreground block mb-1 text-xs">{label}</span>
      <span className="text-sm font-medium break-words">{value}</span>
      {helper && <p className="mt-1 text-xs text-muted-foreground">{helper}</p>}
    </div>
  );
}

export function Agents({ hideHeader = false }: { hideHeader?: boolean }) {
  const [agents, setAgents] = useState<AiAgent[]>([]);
  const [loading, setLoading] = useState(true);
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const selectedAgentId = params.get("selected") ?? undefined;
  const { confirm } = useConfirm();

  const fetchAgents = () => {
    setLoading(true);
    RegistryApi.listAgents()
      .then(setAgents)
      .catch(console.error)
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    fetchAgents();
  }, []);

  const select = (id: string) =>
    setParams((p) => {
      p.set("selected", id);
      return p;
    });

  const deleteAgent = async (id: string) => {
    if (
      !(await confirm({
        title: "Delete Agent",
        description:
          "Are you sure you want to delete this agent? Note: Make sure no active policies depend on it.",
        danger: true,
      }))
    )
      return;
    try {
      await RegistryApi.deleteAgent(id);
      if (selectedAgentId === id) {
        setParams((p) => {
          p.delete("selected");
          return p;
        });
      }
      toast.success("Agent deleted successfully");
      fetchAgents();
    } catch (e) {
      console.error("Failed to delete agent:", e);
      toast.error("Failed to delete agent");
    }
  };

  return (
    <div className={hideHeader ? "space-y-6" : "p-6 md:p-8 space-y-6"}>
      {!hideHeader && (
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-2xl font-semibold tracking-tight">
              Authorized Agents
            </h2>
            <p className="text-sm text-muted-foreground">
              Manage local AI instances connected to the PEP.
            </p>
          </div>
          <button
            onClick={() => navigate("/discovery")}
            className="flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 shadow-sm"
          >
            <Plus className="h-4 w-4" />
            Add Agent
          </button>
        </div>
      )}

      <MasterDetailLayout
        idSelector={(x: any) => x.agent_id || x.id}
        items={agents}
        loading={loading}
        selectedId={selectedAgentId}
        onSelect={select}
        toolbar={
          <div className="flex items-center gap-2 mb-4">
            <input
              type="text"
              placeholder="Search agents..."
              className="px-3 py-1.5 text-sm rounded-md border bg-background"
            />
          </div>
        }
        emptyState={
          <EmptyState
            icon={Users}
            title="No agents found"
            description="Register a local AI agent to start managing its policies."
            actionLabel="Add Agent"
            onAction={() => navigate("/discovery")}
          />
        }
        renderCard={(a, selected) => {
          let status: UiStatus = "ok";
          let label = a.enforcement_mode || "Registered";

          if (a.enforcement_mode === "Enforce") {
            status = "ok";
            label = "🛡️ Protected";
          } else if (a.enforcement_mode === "Observe") {
            status = "info";
            label = "👁️ Observing";
          } else if (a.enforcement_mode === "Shadow") {
            status = "degraded";
            label = "🔧 Shadow AI";
          }

          return (
            <EntityCard
              title={a.name}
              subtitle={a.runtime.runtime_name || "Unknown"}
              icon={Cpu}
              status={status}
              statusLabel={label}
              meta={[
                { label: "Version", value: a.runtime.version || "Unknown" },
                {
                  label: "Identity",
                  value: a.identity?.spiffe_id ? "SPIFFE" : "Local",
                },
              ]}
              selected={selected}
            />
          );
        }}
        renderDetail={(a) => {
          let status: UiStatus = "ok";
          let label = a.enforcement_mode || "Registered";

          if (a.enforcement_mode === "Enforce") {
            status = "ok";
            label = "🛡️ Protected";
          } else if (a.enforcement_mode === "Observe") {
            status = "info";
            label = "👁️ Observing";
          } else if (a.enforcement_mode === "Shadow") {
            status = "degraded";
            label = "🔧 Shadow AI";
          }

          return (
            <DetailPane
              title={a.name}
              subtitle={a.runtime.runtime_name || "Unknown"}
              status={status}
              statusLabel={label}
              actions={[
                {
                  label: "Apply Policy",
                  primary: true,
                  onClick: () => navigate(`/protect?agent=${a.agent_id}`),
                },
                {
                  label: "Delete",
                  danger: true,
                  onClick: () => deleteAgent(a.agent_id),
                },
              ]}
              tabs={[
                {
                  id: "overview",
                  label: "Overview",
                  content: (
                    <div className="space-y-6">
                      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                        <SummaryMetric
                          label="Cloud trace identity"
                          value={a.identity?.spiffe_id || "Not bound yet"}
                          helper={
                            a.identity?.spiffe_id
                              ? "Used as the canonical workload identity when this agent reports to Pollek Cloud."
                              : "Local mode works, but Cloud control should bind a SPIFFE ID before enforcement at fleet scale."
                          }
                        />
                        <SummaryMetric
                          label="Token bindings"
                          value={a.identity?.token_bindings?.length ?? 0}
                          helper={
                            a.identity?.token_bindings?.length
                              ? a.identity.token_bindings
                                  .map(
                                    (binding) =>
                                      `${binding.provider}:${binding.kind}`,
                                  )
                                  .join(", ")
                              : "No OAuth/OIDC/SVID token bindings recorded."
                          }
                        />
                        <SummaryMetric
                          label="Runtime identity"
                          value={a.identity?.user_subject || "Local process"}
                          helper={
                            a.identity?.process_path
                              ? `Process: ${a.identity.process_path}`
                              : "Process path not confirmed yet."
                          }
                        />
                        <SummaryMetric
                          label="Signing key"
                          value={
                            a.identity?.signing_key_fingerprint
                              ? "Fingerprint present"
                              : "Not available"
                          }
                          helper="Fingerprints only; private keys and OAuth tokens are never stored here."
                        />
                      </div>

                      <div className="p-4 bg-muted/30 rounded-xl border space-y-3">
                        <h4 className="text-sm font-semibold">Capabilities</h4>
                        <ul className="text-sm space-y-1.5 text-muted-foreground">
                          {a.capabilities?.map((cap: string) => (
                            <li key={cap} className="flex items-center gap-2">
                              <div className="h-1.5 w-1.5 rounded-full bg-primary/50" />
                              <span className="text-foreground/80">{cap}</span>
                            </li>
                          )) || <li>No specific capabilities</li>}
                        </ul>
                      </div>

                      <div>
                        <h4 className="font-medium mb-2 flex items-center gap-2 text-sm">
                          <Info className="h-4 w-4" /> Raw JSON
                        </h4>
                        <pre className="text-[10px] font-mono bg-muted/50 p-4 rounded-lg overflow-x-auto border">
                          {JSON.stringify(a, null, 2)}
                        </pre>
                      </div>
                    </div>
                  ),
                },
                {
                  id: "enforcement",
                  label: "Enforcement",
                  content: <AgentEnforcementTab agentId={a.agent_id} />,
                },
                {
                  id: "activity",
                  label: "Activity (Live)",
                  content: <AgentActivityTab agentId={a.agent_id} />,
                },
              ]}
            />
          );
        }}
      />
    </div>
  );
}
