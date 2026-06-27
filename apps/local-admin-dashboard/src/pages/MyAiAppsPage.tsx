import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import {
  Activity,
  Bot,
  CheckCircle2,
  Clock3,
  Eye,
  FolderSearch,
  Search,
  ShieldCheck,
  ShieldX,
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
import { DetailPane } from "../components/master-detail/DetailPane";
import type { UiStatus } from "../lib/status";
import { useMode } from "../context/ModeContext";
import { isAdvanceMode } from "../lib/modes";
import { cn } from "@/lib/utils";

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

function agentUiStatus(agent: AiAgent): UiStatus {
  if (agent.enforcement_mode === "Enforce") return "ok";
  if (agent.enforcement_mode === "Observe") return "info";
  if (agent.enforcement_mode === "NotEnforceable") return "degraded";
  return "idle";
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
}: {
  agent: AiAgent;
  activity: UserFriendlyActivityEvent[];
  selected: boolean;
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
    ...events.slice(0, 12).flatMap((event) => [
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
        selected && "border-primary/50 bg-card shadow-md ring-1 ring-primary/50",
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
                {labelize(agent.agent_type)} /{" "}
                {agent.vendor ?? "Unknown vendor"}
              </p>
            </div>
            <span
              className={cn(
                "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px]",
                status.className,
              )}
            >
              <StatusIcon className="h-3 w-3" />
              {status.label}
            </span>
          </div>

          <div className="mt-3 grid gap-2 text-xs sm:grid-cols-3">
            <div className="rounded-md border bg-background/60 p-3">
              <div className="text-muted-foreground">Activity</div>
              <div className="mt-1 text-sm font-semibold">{events.length}</div>
            </div>
            <div className="rounded-md border bg-background/60 p-3">
              <div className="text-muted-foreground">Blocked</div>
              <div className="mt-1 text-sm font-semibold">{blocked}</div>
            </div>
            <div className="rounded-md border bg-background/60 p-3">
              <div className="text-muted-foreground">Trust</div>
              <div className="mt-1 text-sm font-semibold capitalize">
                {agent.trust_level}
              </div>
            </div>
          </div>

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
              className="inline-flex h-8 items-center gap-2 rounded-md border px-3 text-xs hover:bg-muted"
            >
              <Activity className="h-3.5 w-3.5" />
              Activity
            </Link>
            <Link
              to={`/allowed-blocked?q=${encodeURIComponent(agent.name)}`}
              className="inline-flex h-8 items-center gap-2 rounded-md border px-3 text-xs hover:bg-muted"
            >
              <ShieldCheck className="h-3.5 w-3.5" />
              Rules
            </Link>
            <Link
              to="/setup"
              className="inline-flex h-8 items-center gap-2 rounded-md border px-3 text-xs hover:bg-muted"
            >
              <CheckCircle2 className="h-3.5 w-3.5" />
              Setup
            </Link>
          </div>

          <div className="mt-3 flex flex-wrap gap-1.5">
            <span className="rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground">
              {agentSource(agent)}
            </span>
            <span className="rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground">
              {agent.runtime?.runtime_name ?? "Unknown runtime"}
            </span>
            <span className="rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground">
              {agent.declared_tools?.length ?? 0} tools
            </span>
          </div>
        </div>
      </div>
    </article>
  );
}

function AgentDetail({
  agent,
  activity,
  showTechnicalDetails,
}: {
  agent: AiAgent;
  activity: UserFriendlyActivityEvent[];
  showTechnicalDetails: boolean;
}) {
  const status = agentStatus(agent);
  const StatusIcon = status.icon;
  const events = eventsForAgent(agent, activity);
  const blocked = events.filter((event) => event.result === "blocked").length;
  const lastEvent = events[0];
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
    ...events.slice(0, 12).flatMap((event) => [
      event.category,
      event.action,
      event.access_mode,
      event.target_label,
      event.plain_summary,
    ]),
  ];

  return (
    <DetailPane
      title={agent.name}
      subtitle={`${labelize(agent.agent_type)} / ${agent.vendor ?? "Unknown vendor"}`}
      status={agentUiStatus(agent)}
      statusLabel={status.label}
      tabs={[
        {
          id: "overview",
          label: "Overview",
          content: (
            <div className="space-y-4">
              <div className="rounded-lg border bg-background/60 p-4">
                <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  <StatusIcon className="h-4 w-4" />
                  Current visibility
                </div>
                <p className="mt-2 text-sm leading-6 text-muted-foreground">
                  Pollek has this AI app in the local registry. Use Activity to
                  see files, websites, tools, commands, model usage, and policy
                  decisions linked to it.
                </p>
              </div>

              <div className="grid gap-3 md:grid-cols-3">
                <div className="rounded-lg border bg-background/60 p-4">
                  <div className="text-xs text-muted-foreground">Activity</div>
                  <div className="mt-1 text-lg font-semibold">
                    {events.length}
                  </div>
                </div>
                <div className="rounded-lg border bg-background/60 p-4">
                  <div className="text-xs text-muted-foreground">Blocked</div>
                  <div className="mt-1 text-lg font-semibold">{blocked}</div>
                </div>
                <div className="rounded-lg border bg-background/60 p-4">
                  <div className="text-xs text-muted-foreground">Trust</div>
                  <div className="mt-1 text-lg font-semibold capitalize">
                    {agent.trust_level}
                  </div>
                </div>
              </div>

              <div className="grid gap-3 md:grid-cols-2">
                <div className="rounded-lg border bg-background/60 p-4">
                  <div className="text-xs text-muted-foreground">Source</div>
                  <div className="mt-1 text-sm font-semibold">
                    {agentSource(agent)}
                  </div>
                </div>
                <div className="rounded-lg border bg-background/60 p-4">
                  <div className="text-xs text-muted-foreground">Runtime</div>
                  <div className="mt-1 text-sm font-semibold">
                    {agent.runtime?.runtime_name ?? "Unknown runtime"}
                  </div>
                </div>
              </div>

              <ReferenceIntelGuide
                reference={reference}
                observedTerms={observedTerms}
              />
            </div>
          ),
        },
        {
          id: "activity",
          label: "Activity",
          content: (
            <div className="space-y-3">
              {lastEvent ? (
                events.slice(0, 12).map((event) => (
                  <div
                    key={event.event_id}
                    className="rounded-lg border bg-background/60 p-4"
                  >
                    <div className="flex flex-wrap items-start justify-between gap-2">
                      <div>
                        <div className="text-sm font-medium">
                          {event.plain_summary}
                        </div>
                        <div className="mt-1 text-xs text-muted-foreground">
                          {formatDateTime(event.timestamp)}
                        </div>
                      </div>
                      <span className="rounded-full border bg-card px-2 py-0.5 text-[11px] text-muted-foreground">
                        {event.result_label}
                      </span>
                    </div>
                    <p className="mt-2 text-xs leading-5 text-muted-foreground">
                      {event.capability_note}
                    </p>
                  </div>
                ))
              ) : (
                <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
                  No recent activity is linked to this AI app yet.
                </div>
              )}
              <Link
                to={`/activity?q=${encodeURIComponent(agent.name)}`}
                className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
              >
                <Activity className="h-4 w-4" />
                Open full activity
              </Link>
            </div>
          ),
        },
        {
          id: "control",
          label: "Rules & Setup",
          content: (
            <div className="space-y-3">
              <div className="rounded-lg border bg-background/60 p-4">
                <h4 className="text-sm font-semibold">
                  What to review for this AI app
                </h4>
                <p className="mt-2 text-sm leading-6 text-muted-foreground">
                  Start by observing. If the timeline shows file writes, web
                  access, email use, terminal commands, or model spend that you
                  do not want, create a rule in Pollek or apply the matching
                  restriction inside the AI app settings.
                </p>
              </div>
              <div className="flex flex-wrap gap-2">
                <Link
                  to={`/allowed-blocked?q=${encodeURIComponent(agent.name)}`}
                  className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
                >
                  <ShieldCheck className="h-4 w-4" />
                  Rules
                </Link>
                <Link
                  to="/setup"
                  className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
                >
                  <CheckCircle2 className="h-4 w-4" />
                  Setup
                </Link>
              </div>
            </div>
          ),
        },
        ...(showTechnicalDetails
          ? [
              {
                id: "technical",
                label: "Technical Details",
                content: (
                  <pre className="overflow-auto rounded-lg border bg-muted/40 p-4 text-[11px]">
                    {JSON.stringify(agent, null, 2)}
                  </pre>
                ),
              },
            ]
          : []),
      ]}
    />
  );
}

export function MyAiAppsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const { mode } = useMode();
  const showTechnicalDetails = isAdvanceMode(mode);
  const [agents, setAgents] = useState<AiAgent[]>([]);
  const [activity, setActivity] = useState<UserFriendlyActivityEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
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

  return (
    <div className="space-y-5">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <h2 className="flex items-center gap-2 text-2xl font-bold tracking-tight">
            <Bot className="h-6 w-6 text-primary" />
            My AI Apps
          </h2>
          <p className="text-sm text-muted-foreground">
            AI assistants found on this computer, with what Pollek can currently
            see.
          </p>
        </div>
        <Link
          to="/scan"
          className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
        >
          <FolderSearch className="h-4 w-4" />
          Find AI apps
        </Link>
      </div>

      <section className="grid gap-3 sm:grid-cols-3">
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="text-2xl font-semibold">{agents.length}</div>
          <p className="mt-1 text-xs text-muted-foreground">AI apps found</p>
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

      <MasterDetailLayout
        items={filtered}
        selectedId={selectedId}
        onSelect={handleSelect}
        idSelector={(agent) => agent.agent_id}
        loading={loading && agents.length === 0}
        emptyState={
          <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
            No AI apps match this search yet.
          </div>
        }
        renderCard={(agent, selected) => (
          <AgentCard agent={agent} activity={activity} selected={selected} />
        )}
        renderDetail={(agent) => (
          <AgentDetail
            key={agent.agent_id}
            agent={agent}
            activity={activity}
            showTechnicalDetails={showTechnicalDetails}
          />
        )}
      />
    </div>
  );
}
