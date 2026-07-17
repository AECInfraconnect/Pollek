import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import {
  Activity,
  Bot,
  ChevronDown,
  ChevronUp,
  CheckCircle2,
  Clock3,
  Eye,
  FolderSearch,
  Search,
  ShieldCheck,
  ShieldX,
  Trash2,
} from "lucide-react";
import { RegistryApi, type AiAgent } from "../services/api";
import { UserActivityApi } from "../features/user-activity/api";
import { ReferenceIntelGuide } from "../components/reference/ReferenceIntelGuide";
import { ReferenceIntelMark } from "../components/reference/ReferenceIntelMark";
import {
  formatDateTime,
  labelize,
} from "../features/user-activity/userActivityModel";
import type { UserFriendlyActivityEvent } from "../features/user-activity/types";
import { findAgentReferenceIntel } from "../lib/entityReferenceIntel";
import { MasterDetailLayout } from "../components/master-detail/MasterDetailLayout";
import { PageHeader } from "../components/layout/PageHeader";
import { cn } from "@/lib/utils";
import { AgentDetailView } from "./AgentsV2";
import { useConfirm } from "../components/ui/ConfirmDialog";
import { toast } from "sonner";

function agentSource(agent: AiAgent) {
  const source = agent.meta?.source ?? "registry";
  if (source === "discovery") return "Found by scan";
  if (source === "agent_self_registration") return "Reported by AI app";
  return labelize(source);
}

function agentStatus(agent: AiAgent) {
  if (agent.enforcement_mode === "Enforce") {
    return {
      label: "Rules can block",
      icon: ShieldCheck,
      className: "border-emerald-500/25 bg-emerald-500/10 text-emerald-700",
    };
  }
  if (agent.enforcement_mode === "Observe") {
    return {
      label: "Watching",
      icon: Eye,
      className: "border-blue-500/25 bg-blue-500/10 text-blue-700",
    };
  }
  if (agent.enforcement_mode === "NotEnforceable") {
    return {
      label: "Watch only",
      icon: ShieldX,
      className: "border-amber-500/25 bg-amber-500/10 text-amber-700",
    };
  }
  return {
    label: agent.enforcement_mode ?? "Registered",
    icon: Bot,
    className: "border-border bg-background text-muted-foreground",
  };
}

function friendlyAgentType(type?: string) {
  const normalized = (type ?? "").toLowerCase();
  const compact = normalized.replace(/[^a-z0-9]+/g, "");
  if (
    normalized === "open_ai_agent" ||
    normalized === "openai_agent" ||
    compact === "openaiagent"
  ) {
    return "OpenAI agent";
  }
  if (normalized === "cli_agent") return "CLI agent";
  if (normalized === "desktop_agent") return "Desktop agent";
  if (normalized === "browser_agent") return "Browser agent";
  if (normalized === "web_ai_app" || normalized === "web_a_i_app") {
    return "Web AI app";
  }
  if (!normalized || normalized === "unknown") return "Unknown type";
  return labelize(type);
}

function eventsForAgent(agent: AiAgent, activity: UserFriendlyActivityEvent[]) {
  const agentName = agent.name.toLowerCase();
  return activity.filter(
    (event) =>
      event.agent_id === agent.agent_id ||
      event.agent_name.toLowerCase() === agentName,
  );
}

function AgentCard({
  agent,
  activity,
  selected,
  onDelete,
  expanded,
  onExpandedChange,
}: {
  agent: AiAgent;
  activity: UserFriendlyActivityEvent[];
  selected: boolean;
  onDelete: () => void;
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
}) {
  const status = agentStatus(agent);
  const StatusIcon = status.icon;
  const events = eventsForAgent(agent, activity);
  const lastEvent = events[0];
  const blocked = events.filter((event) => event.result === "blocked").length;
  const reference = findAgentReferenceIntel({
    name: agent.name,
    vendor: agent.vendor,
    agentType: agent.agent_type,
    runtimeName: agent.runtime?.runtime_name,
  })[0];
  const observedTerms = [
    agent.name,
    agent.vendor,
    agent.agent_type,
    agent.runtime?.runtime_name,
    ...(agent.capabilities ?? []),
    ...(agent.declared_tools ?? []),
    ...(agent.declared_resources ?? []),
    ...events
      .slice(0, 12)
      .flatMap((event) => [
        event.category,
        event.action,
        event.access_mode,
        event.target_label,
        event.plain_summary,
      ]),
  ];

  return (
    <article
      className={cn(
        "rounded-lg border bg-card/60 p-4 transition-all hover:border-primary/40 hover:bg-card",
        selected &&
          "border-primary/50 bg-card shadow-md ring-1 ring-primary/50",
      )}
    >
      <div className="flex items-start gap-3">
        {reference ? (
          <ReferenceIntelMark reference={reference} size="sm" />
        ) : (
          <div className="rounded-lg bg-primary/10 p-2 text-primary">
            <Bot className="h-4 w-4" />
          </div>
        )}
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-start justify-between gap-2">
            <div className="min-w-0">
              <h3 className="truncate text-sm font-semibold">{agent.name}</h3>
              <p className="mt-1 truncate text-xs text-muted-foreground">
                {friendlyAgentType(agent.agent_type)} /{" "}
                {agent.vendor ?? "Unknown vendor"}
              </p>
            </div>
            <div className="flex shrink-0 flex-wrap items-center justify-end gap-1.5">
              <span
                className={cn(
                  "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px]",
                  status.className,
                )}
              >
                <StatusIcon className="h-3 w-3" />
                {status.label}
              </span>
              <button
                type="button"
                onClick={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  onExpandedChange(!expanded);
                }}
                className="inline-flex h-7 items-center gap-1 rounded-md border bg-background px-2 text-[11px] text-muted-foreground hover:bg-muted hover:text-foreground"
                aria-expanded={expanded}
              >
                {expanded ? (
                  <>
                    Collapse <ChevronUp className="h-3 w-3" />
                  </>
                ) : (
                  <>
                    Expand <ChevronDown className="h-3 w-3" />
                  </>
                )}
              </button>
              <button
                type="button"
                onClick={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  onDelete();
                }}
                className="inline-flex h-7 items-center gap-1 rounded-md border border-red-500/25 bg-red-500/10 px-2 text-[11px] font-medium text-red-600 hover:bg-red-500/20"
              >
                <Trash2 className="h-3 w-3" />
                Delete
              </button>
            </div>
          </div>

          <div className="mt-3 flex flex-wrap gap-1.5 text-[11px] text-muted-foreground">
            <span className="rounded-full border bg-background px-2 py-0.5">
              Activity:{" "}
              <span className="font-medium text-foreground">
                {events.length}
              </span>
            </span>
            <span className="rounded-full border bg-background px-2 py-0.5">
              Blocked:{" "}
              <span className="font-medium text-foreground">{blocked}</span>
            </span>
            <span className="rounded-full border bg-background px-2 py-0.5">
              Trust:{" "}
              <span className="font-medium capitalize text-foreground">
                {agent.trust_level}
              </span>
            </span>
            <span className="rounded-full border bg-background px-2 py-0.5">
              {agentSource(agent)}
            </span>
          </div>

          {expanded && (
            <>
              <p className="mt-3 text-xs leading-5 text-muted-foreground">
                {lastEvent
                  ? `Last seen: ${lastEvent.plain_summary} (${formatDateTime(
                      lastEvent.timestamp,
                    )})`
                  : "No recent activity is linked to this AI app yet."}
              </p>

              <div className="mt-3">
                <ReferenceIntelGuide
                  reference={reference}
                  observedTerms={observedTerms}
                  compact
                />
              </div>

              <div className="mt-3 flex flex-wrap gap-2">
                <Link
                  to={`/activity?q=${encodeURIComponent(agent.name)}`}
                  onClick={(event) => event.stopPropagation()}
                  className="inline-flex h-8 items-center gap-2 rounded-md border px-3 text-xs hover:bg-muted"
                >
                  <Activity className="h-3.5 w-3.5" />
                  Activity
                </Link>
                <Link
                  to={`/allowed-blocked?q=${encodeURIComponent(agent.name)}`}
                  onClick={(event) => event.stopPropagation()}
                  className="inline-flex h-8 items-center gap-2 rounded-md border px-3 text-xs hover:bg-muted"
                >
                  <ShieldCheck className="h-3.5 w-3.5" />
                  Rules
                </Link>
                <Link
                  to="/setup"
                  onClick={(event) => event.stopPropagation()}
                  className="inline-flex h-8 items-center gap-2 rounded-md border px-3 text-xs hover:bg-muted"
                >
                  <CheckCircle2 className="h-3.5 w-3.5" />
                  Setup
                </Link>
              </div>

              <div className="mt-3 flex flex-wrap gap-1.5">
                <span className="rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground">
                  {agent.runtime?.runtime_name ?? "Unknown runtime"}
                </span>
                <span className="rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground">
                  {agent.declared_tools?.length ?? 0} tools
                </span>
                <span className="rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground">
                  {agent.declared_resources?.length ?? 0} resources
                </span>
              </div>
            </>
          )}
        </div>
      </div>
    </article>
  );
}

export function MyAiAppsPage() {
  const { confirm } = useConfirm();
  const [searchParams, setSearchParams] = useSearchParams();
  const [agents, setAgents] = useState<AiAgent[]>([]);
  const [activity, setActivity] = useState<UserFriendlyActivityEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [expandedCards, setExpandedCards] = useState<Set<string>>(
    () => new Set(),
  );
  const selectedId = searchParams.get("selected") ?? undefined;

  const load = useCallback(() => {
    setLoading(true);
    Promise.all([
      RegistryApi.listAgents().catch(() => [] as AiAgent[]),
      UserActivityApi.list({ limit: 300 }).catch(() => ({ items: [] })),
    ])
      .then(([agentRows, activityPage]) => {
        setAgents(agentRows);
        setActivity(activityPage.items ?? []);
      })
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const filtered = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return agents;
    return agents.filter((agent) =>
      [
        agent.name,
        agent.vendor,
        agent.agent_type,
        agent.runtime?.runtime_name,
        agent.enforcement_mode,
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase()
        .includes(query),
    );
  }, [agents, search]);

  const handleSelect = useCallback(
    (agentId: string) => {
      const next = new URLSearchParams(searchParams);
      if (agentId) next.set("selected", agentId);
      else next.delete("selected");
      setSearchParams(next, { replace: true });
    },
    [searchParams, setSearchParams],
  );

  const deleteAgent = useCallback(
    async (agent: AiAgent) => {
      if (
        !(await confirm({
          title: "Delete AI app",
          description: `Delete ${agent.name}? Pollek will remove this registered AI app, its linked policies, setup properties, and local registry metadata. If a later scan finds the same app again, it will appear as Pending and can be registered again.`,
          confirmText: "Delete",
          danger: true,
        }))
      ) {
        return;
      }

      try {
        await RegistryApi.deleteAgent(agent.agent_id);
        if (selectedId === agent.agent_id) {
          setSearchParams(new URLSearchParams(), { replace: true });
        }
        load();
        toast.success("AI app deleted");
      } catch (error) {
        console.error("Failed to delete AI app:", error);
        toast.error("Failed to delete AI app");
      }
    },
    [confirm, load, selectedId, setSearchParams],
  );

  return (
    <div className="space-y-5">
      {!selectedId && (
        <>
          <PageHeader
            title="My AI Apps"
            subtitle="AI assistants found on this computer, with what Pollek can currently see."
            icon={Bot}
            actions={
              <Link
                to="/scan"
                className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
              >
                <FolderSearch className="h-4 w-4" />
                Find AI apps
              </Link>
            }
          />

          <section className="grid gap-3 sm:grid-cols-3">
            <div className="rounded-lg border bg-card/60 p-4">
              <div className="text-2xl font-semibold">{agents.length}</div>
              <p className="mt-1 text-xs text-muted-foreground">
                AI apps found
              </p>
            </div>
            <div className="rounded-lg border bg-card/60 p-4">
              <div className="text-2xl font-semibold">{activity.length}</div>
              <p className="mt-1 text-xs text-muted-foreground">
                Recent activities
              </p>
            </div>
            <div className="rounded-lg border bg-card/60 p-4">
              <div className="flex items-center gap-2 text-2xl font-semibold">
                <Clock3 className="h-5 w-5 text-primary" />
                Live
              </div>
              <p className="mt-1 text-xs text-muted-foreground">
                Local dashboard data
              </p>
            </div>
          </section>

          <section className="rounded-lg border bg-card/60 p-4">
            <label className="relative block">
              <span className="sr-only">Search AI apps</span>
              <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
              <input
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder="Search AI app, vendor, runtime..."
                className="h-9 w-full rounded-md border bg-background pl-9 pr-3 text-sm"
              />
            </label>
          </section>
        </>
      )}

      <MasterDetailLayout
        items={filtered}
        selectedId={selectedId}
        onSelect={handleSelect}
        idSelector={(agent) => agent.agent_id}
        loading={loading && agents.length === 0}
        masterLayout="grid"
        masterListClassName="grid gap-4 lg:grid-cols-2 2xl:grid-cols-3"
        itemClassName={(agent) =>
          expandedCards.has(agent.agent_id)
            ? "lg:col-span-2 2xl:col-span-3"
            : undefined
        }
        detailBackLabel="Back to all AI apps"
        emptyState={
          <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
            No AI apps match this search yet.
          </div>
        }
        renderCard={(agent, selected) => (
          <AgentCard
            agent={agent}
            activity={activity}
            selected={selected}
            expanded={expandedCards.has(agent.agent_id)}
            onExpandedChange={(expanded) => {
              setExpandedCards((current) => {
                const next = new Set(current);
                if (expanded) next.add(agent.agent_id);
                else next.delete(agent.agent_id);
                return next;
              });
            }}
            onDelete={() => {
              void deleteAgent(agent);
            }}
          />
        )}
        renderDetail={(agent) => (
          <AgentDetailView
            key={agent.agent_id}
            agent={agent}
            onDelete={() => {
              void deleteAgent(agent);
            }}
          />
        )}
      />
    </div>
  );
}
