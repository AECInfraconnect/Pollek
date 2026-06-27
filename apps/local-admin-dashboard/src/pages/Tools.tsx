import { toast } from "sonner";
import { useState, useEffect } from "react";
import { Wrench, Info, FileKey, Activity } from "lucide-react";
import { useSearchParams } from "react-router-dom";
import { RegistryApi, TelemetryApi } from "../services/api";
import type { Tool, ObservedTool } from "../services/api";

export interface UnifiedTool {
  id: string;
  name: string;
  tool_id: string;
  description?: string;
  risk_level?: string;
  data_access_level?: string;
  side_effect_level?: string;
  is_registered: boolean;
  is_observed: boolean;
  observed_details?: ObservedTool;
  registered_details?: Tool;
}
import { MasterDetailLayout } from "../components/master-detail/MasterDetailLayout";
import { EntityCard } from "../components/master-detail/EntityCard";
import { DetailPane } from "../components/master-detail/DetailPane";
import { EmptyState } from "../components/master-detail/EmptyState";
import { RegisterControlBar } from "../components/RegisterControlBar";
import type { UiStatus } from "../lib/status";
import { useConfirm } from "../components/ui/ConfirmDialog";
import { Entity360Layout } from "../features/entity-360/Entity360Layout";
import { useEntity360 } from "../features/entity-graph/useEntity360";

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

function toolStatus(tool: UnifiedTool): { status: UiStatus; label: string } {
  if (!tool.is_registered) {
    return { status: "idle", label: "Observed" };
  }
  if (tool.risk_level === "high" || tool.risk_level === "critical") {
    return {
      status: "failed",
      label: tool.risk_level ? tool.risk_level.toUpperCase() : "HIGH",
    };
  }
  if (tool.risk_level === "medium") {
    return { status: "degraded", label: "MEDIUM" };
  }
  return {
    status: "ok",
    label: tool.risk_level ? tool.risk_level.toUpperCase() : "UNKNOWN",
  };
}

export function Tools({ hideHeader = false }: { hideHeader?: boolean }) {
  const [tools, setTools] = useState<UnifiedTool[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [kindFilter, setKindFilter] = useState("all");
  const [agentFilter, setAgentFilter] = useState("");
  const [params, setParams] = useSearchParams();
  const selectedId = params.get("selected") ?? undefined;
  const { confirm } = useConfirm();

  const fetchTools = async () => {
    setLoading(true);
    try {
      const [regRes, obsRes] = await Promise.all([
        RegistryApi.listTools(),
        TelemetryApi.listToolInventory(agentFilter || undefined).catch(() => ({
          items: [] as ObservedTool[],
        })),
      ]);

      const unifiedMap = new Map<string, UnifiedTool>();

      for (const t of regRes) {
        unifiedMap.set(t.tool_id, {
          id: t.tool_id,
          name: t.name,
          tool_id: t.tool_id,
          description: t.description,
          risk_level: t.risk_level,
          data_access_level: t.data_access_level,
          side_effect_level: t.side_effect_level,
          is_registered: true,
          is_observed: false,
          registered_details: t,
        });
      }

      for (const o of obsRes.items || []) {
        const id = o.tool_id;
        if (unifiedMap.has(id)) {
          const existing = unifiedMap.get(id)!;
          existing.is_observed = true;
          existing.observed_details = o;
        } else {
          unifiedMap.set(id, {
            id,
            name: o.tool_name,
            tool_id: id,
            description: `Kind: ${o.tool_kind}, Server: ${o.server || "unknown"}`,
            risk_level: "unknown",
            data_access_level: "unknown",
            side_effect_level: "unknown",
            is_registered: false,
            is_observed: true,
            observed_details: o,
          });
        }
      }

      setTools(
        Array.from(unifiedMap.values()).filter((tool) => {
          const haystack =
            `${tool.name} ${tool.tool_id} ${tool.description ?? ""}`.toLowerCase();
          const matchesSearch = haystack.includes(search.trim().toLowerCase());
          const matchesKind =
            kindFilter === "all" ||
            tool.observed_details?.tool_kind === kindFilter ||
            tool.registered_details?.category === kindFilter;
          return matchesSearch && matchesKind;
        }),
      );
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchTools();

    const source = new EventSource(TelemetryApi.streamUrl("tools"));
    source.onmessage = (e) => {
      try {
        const data = JSON.parse(e.data);
        if (data.event_type === "tool_usage") {
          fetchTools();
        }
      } catch (err) {}
    };

    return () => source.close();
  }, [search, kindFilter, agentFilter]);

  const select = (id: string) =>
    setParams((p) => {
      p.set("selected", id);
      return p;
    });

  const deleteTool = async (id: string) => {
    if (
      !(await confirm({
        title: "Delete Tool",
        description: "Are you sure you want to delete this tool?",
        danger: true,
      }))
    )
      return;
    try {
      await RegistryApi.deleteTool(id);
      if (selectedId === id) {
        setParams((p) => {
          p.delete("selected");
          return p;
        });
      }
      toast.success("Tool deleted successfully");
      fetchTools();
    } catch (e) {
      console.error("Failed to delete tool:", e);
      toast.error("Failed to delete tool");
    }
  };

  return (
    <div className={hideHeader ? "space-y-6" : "p-6 md:p-8 space-y-6"}>
      {!hideHeader && (
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-lg font-semibold tracking-tight">Tools</h2>
            <p className="text-sm text-muted-foreground">
              Manage function-calling definitions available to AI Agents.
            </p>
          </div>
        </div>
      )}

      <MasterDetailLayout
        items={tools}
        loading={loading}
        selectedId={selectedId}
        onSelect={select}
        idSelector={(t: UnifiedTool) => t.id}
        toolbar={
          <div className="flex items-center gap-2 mb-4">
            <input
              type="text"
              placeholder="Search tools..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="px-3 py-1.5 text-sm rounded-md border bg-background"
            />
            <select
              value={kindFilter}
              onChange={(e) => setKindFilter(e.target.value)}
              className="px-3 py-1.5 text-sm rounded-md border bg-background"
            >
              <option value="all">All kinds</option>
              <option value="mcp_tool">MCP tool</option>
              <option value="function_call">Function</option>
              <option value="http_api">HTTP API</option>
              <option value="a2a_skill">A2A skill</option>
              <option value="shell_command">Shell</option>
              <option value="browser_action">Browser</option>
            </select>
            <input
              type="text"
              placeholder="Agent ID"
              value={agentFilter}
              onChange={(e) => setAgentFilter(e.target.value)}
              className="px-3 py-1.5 text-sm rounded-md border bg-background"
            />
          </div>
        }
        emptyState={
          <EmptyState
            icon={Wrench}
            title="No tools found"
            description="Register JSON schemas for tools that your agents can invoke."
          />
        }
        renderCard={(t: UnifiedTool, selected) => {
          const { status, label } = toolStatus(t);
          const observed = t.observed_details;

          return (
            <EntityCard
              title={t.name}
              subtitle={t.description || "No description"}
              summary={
                observed
                  ? `${observed.use_count} observed invocation(s) from ${observed.agents.length || 0} agent(s). ${observed.governed ? "Policy governed." : "No active policy yet."}`
                  : `${t.risk_level || "Unknown"} risk tool with ${t.data_access_level || "unknown"} data access.`
              }
              icon={Wrench}
              status={status}
              statusLabel={label}
              meta={[
                { label: "Data Access", value: t.data_access_level },
                { label: "Side Effect", value: t.side_effect_level },
              ]}
              actions={[
                {
                  label: t.is_registered ? "Policy" : "Protect",
                  primary: !t.is_registered,
                  onClick: () => {},
                },
              ]}
              selected={selected}
            />
          );
        }}
        renderDetail={(t: UnifiedTool) => (
          <Tool360Detail
            tool={t}
            onDelete={() => deleteTool(t.tool_id)}
            onRegistered={() => fetchTools()}
          />
        )}
      />
    </div>
  );
}

function ToolFriendlyOverview({
  tool,
  onRegistered,
}: {
  tool: UnifiedTool;
  onRegistered: () => void;
}) {
  return (
    <div className="space-y-5">
      <div className="grid grid-cols-1 gap-4 text-sm md:grid-cols-2">
        <SummaryMetric
          label="What POLLEK saw"
          value={tool.name}
          helper={
            tool.observed_details
              ? `${tool.observed_details.tool_kind} - ${
                  tool.observed_details.server || "local"
                }`
              : tool.is_registered
                ? "Registered tool definition"
                : "Observed tool"
          }
        />
        <SummaryMetric
          label="Risk"
          value={tool.risk_level || "unknown"}
          helper={`Data: ${tool.data_access_level || "unknown"} - Effects: ${
            tool.side_effect_level || "unknown"
          }`}
        />
        {tool.is_observed && tool.observed_details && (
          <>
            <SummaryMetric
              label="Last used"
              value={new Date(tool.observed_details.last_used).toLocaleString()}
              helper={`${tool.observed_details.use_count} observed invocation(s).`}
            />
            <SummaryMetric
              label="Agents invoking it"
              value={tool.observed_details.agents.length}
              helper={
                tool.observed_details.agents.join(", ") ||
                "No agent linked yet."
              }
            />
            <SummaryMetric
              label="Governance"
              value={
                tool.observed_details.governed
                  ? "Policy attached"
                  : "Needs policy"
              }
              helper={
                tool.is_registered
                  ? "Registered tool can be targeted directly."
                  : "Protect will create a policy target for this observed tool."
              }
            />
          </>
        )}
      </div>

      <div className="rounded-lg border bg-muted/30 p-4">
        <h4 className="mb-2 text-sm font-semibold">Registration Status</h4>
        <RegisterControlBar
          agentId={tool.tool_id}
          tenantId="local"
          onSuccess={onRegistered}
        />
      </div>
    </div>
  );
}

function ToolPolicyPrompt({ tool }: { tool: UnifiedTool }) {
  return (
    <div className="flex flex-col items-center justify-center rounded-lg border border-dashed p-8 text-center text-muted-foreground">
      <FileKey className="mb-4 h-8 w-8 opacity-50" />
      <p className="mb-4 text-sm">
        Protect this tool by assigning an access policy.
      </p>
      <button
        type="button"
        className="rounded-md bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90"
        onClick={() => {
          toast.success(`Policy draft requested for ${tool.name}.`);
        }}
      >
        Create Policy
      </button>
    </div>
  );
}

function ToolSchemaPanel({ tool }: { tool: UnifiedTool }) {
  return (
    <div>
      <h4 className="mb-2 flex items-center gap-2 text-sm font-medium">
        <Info className="h-4 w-4" /> JSON Schema
      </h4>
      <pre className="overflow-x-auto rounded-lg border bg-muted/50 p-4 font-mono text-[10px]">
        {JSON.stringify((tool as any).schema, null, 2)}
      </pre>
    </div>
  );
}

function Tool360Detail({
  tool,
  onDelete,
  onRegistered,
}: {
  tool: UnifiedTool;
  onDelete: () => void;
  onRegistered: () => void;
}) {
  const { data } = useEntity360("tool", tool.tool_id);
  const { status, label } = toolStatus(tool);
  const protect = () =>
    toast.success(`Policy draft requested for ${tool.name}.`);
  const actions = tool.is_registered ? (
    <button
      type="button"
      onClick={onDelete}
      className="inline-flex h-9 items-center rounded-md border border-red-500/30 bg-red-500/10 px-4 text-sm font-medium text-red-600 hover:bg-red-500/15"
    >
      Delete
    </button>
  ) : (
    <button
      type="button"
      onClick={protect}
      className="inline-flex h-9 items-center rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground hover:bg-primary/90"
    >
      Protect Tool
    </button>
  );

  if (data) {
    return (
      <Entity360Layout
        data={data}
        actions={actions}
        overview={
          <ToolFriendlyOverview tool={tool} onRegistered={onRegistered} />
        }
      />
    );
  }

  return (
    <DetailPane
      title={tool.name}
      subtitle={tool.description}
      status={status}
      statusLabel={label}
      actions={
        tool.is_registered
          ? [{ label: "Delete", danger: true, onClick: onDelete }]
          : [{ label: "Protect Tool", primary: true, onClick: protect }]
      }
      tabs={[
        {
          id: "overview",
          label: "Overview",
          content: (
            <ToolFriendlyOverview tool={tool} onRegistered={onRegistered} />
          ),
        },
        {
          id: "schema",
          label: "Schema",
          content: <ToolSchemaPanel tool={tool} />,
        },
        {
          id: "policies",
          label: "Policies",
          content: <ToolPolicyPrompt tool={tool} />,
        },
        {
          id: "activity",
          label: "Activity",
          content: <ToolActivityTimeline tool={tool} />,
        },
      ]}
    />
  );
}

function ToolActivityTimeline({ tool }: { tool: UnifiedTool }) {
  const [events, setEvents] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let mounted = true;
    setLoading(true);
    TelemetryApi.getObservations({ toolId: tool.tool_id || tool.name }).then(
      (res) => {
        if (mounted) {
          setEvents(res.items || []);
          setLoading(false);
        }
      },
    );
    return () => {
      mounted = false;
    };
  }, [tool.tool_id, tool.name]);

  if (loading)
    return (
      <div className="p-8 text-center text-sm text-muted-foreground">
        Loading activity...
      </div>
    );
  if (events.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center p-8 text-center border border-dashed rounded-lg text-muted-foreground">
        <Activity className="h-8 w-8 mb-2 opacity-50" />
        <p className="text-sm">No activity recorded yet.</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {events.map((ev, i) => (
        <div key={i} className="flex gap-4 p-4 border rounded-lg bg-card">
          <div className="mt-1">
            <Activity className="h-4 w-4 text-primary" />
          </div>
          <div>
            <p className="text-sm font-medium">
              Invoked by Agent: {ev.agent_id || "Unknown"}
            </p>
            <p className="text-xs text-muted-foreground mt-1">
              Method:{" "}
              {ev.details?.tool_name ||
                ev.details?.tool_kind ||
                ev.tool_name ||
                "execute"}{" "}
              • {new Date(ev.observed_at || ev.timestamp).toLocaleString()}
            </p>
          </div>
        </div>
      ))}
    </div>
  );
}
