import { useEffect, useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import {
  AppWindow,
  Database,
  ExternalLink,
  FolderOpen,
  Globe2,
  Mail,
  ShieldCheck,
  Search,
  Sparkles,
  Terminal,
  Wrench,
} from "lucide-react";
import { RegistryApi, TelemetryApi, UsageApi } from "../services/api";
import type { Resource, Tool } from "../services/types";
import { UserActivityApi } from "../features/user-activity/api";
import {
  categoryLabel,
  labelize,
} from "../features/user-activity/userActivityModel";
import type { UserActivityCategory } from "../features/user-activity/types";
import {
  addUsageEventAgentAliases,
  buildAgentNameMap,
  resolveActivityAgentNames,
} from "../lib/agentNameResolver";
import { MasterDetailLayout } from "../components/master-detail/MasterDetailLayout";
import { PageHeader } from "../components/layout/PageHeader";
import { DetailPane } from "../components/master-detail/DetailPane";
import type { UiStatus } from "../lib/status";
import { useMode } from "../context/ModeContext";
import { isAdvanceMode } from "../lib/modes";
import { cn } from "@/lib/utils";
import { useConfirm } from "../components/ui/ConfirmDialog";
import { toast } from "sonner";
import { Collapsible } from "../components/ui";

type Tab =
  | "all"
  | "files"
  | "web"
  | "email"
  | "apps"
  | "commands"
  | "ai_models"
  | "tools"
  | "safety";

const tabs: Array<{ id: Tab; label: string; icon: any }> = [
  { id: "all", label: "All", icon: Database },
  { id: "files", label: "Files & folders", icon: FolderOpen },
  { id: "web", label: "Websites", icon: Globe2 },
  { id: "email", label: "Email & calendar", icon: Mail },
  { id: "apps", label: "Apps", icon: AppWindow },
  { id: "commands", label: "Commands", icon: Terminal },
  { id: "ai_models", label: "AI APIs & models", icon: Sparkles },
  { id: "tools", label: "AI tools", icon: Wrench },
  { id: "safety", label: "Prompt safety", icon: ShieldCheck },
];

type DataAppRow = {
  id: string;
  tab: Tab;
  title: string;
  subtitle: string;
  category: UserActivityCategory;
  source: string;
  kind:
    | "resource"
    | "tool"
    | "observed_resource"
    | "observed_tool"
    | "observed_activity";
  raw?: unknown;
};

function rawResourceText(resource: Resource) {
  return JSON.stringify(resource).toLowerCase();
}

function rawToolText(tool: Tool) {
  return JSON.stringify(tool).toLowerCase();
}

function resourceName(resource: Resource) {
  const row = resource as any;
  return (
    row.name ??
    row.display_name ??
    row.label ??
    row.uri ??
    row.resource_id ??
    row.id ??
    "Unnamed data target"
  );
}

function toolName(tool: Tool) {
  const row = tool as any;
  return (
    row.name ??
    row.display_name ??
    row.label ??
    row.tool_id ??
    row.id ??
    "Unnamed tool"
  );
}

function resourceCategory(resource: Resource): Tab {
  const text = rawResourceText(resource);
  if (
    text.includes("http") ||
    text.includes("domain") ||
    text.includes("url")
  ) {
    return "web";
  }
  if (text.includes("email") || text.includes("calendar")) return "email";
  if (
    text.includes("model") ||
    text.includes("llm") ||
    text.includes("token") ||
    text.includes("openai") ||
    text.includes("anthropic") ||
    text.includes("huggingface") ||
    text.includes("nvidia")
  ) {
    return "ai_models";
  }
  if (
    text.includes("prompt") ||
    text.includes("injection") ||
    text.includes("pii") ||
    text.includes("secret") ||
    text.includes("redact") ||
    text.includes("guard")
  ) {
    return "safety";
  }
  if (text.includes("process") || text.includes("app")) return "apps";
  if (
    text.includes("command") ||
    text.includes("terminal") ||
    text.includes("shell")
  ) {
    return "commands";
  }
  return "files";
}

function toolCategory(tool: Tool): Tab {
  const text = rawToolText(tool);
  if (
    text.includes("command") ||
    text.includes("terminal") ||
    text.includes("shell")
  ) {
    return "commands";
  }
  if (
    text.includes("prompt") ||
    text.includes("injection") ||
    text.includes("pii") ||
    text.includes("secret") ||
    text.includes("redact") ||
    text.includes("guard")
  ) {
    return "safety";
  }
  if (
    text.includes("model") ||
    text.includes("llm") ||
    text.includes("token") ||
    text.includes("openai") ||
    text.includes("anthropic")
  ) {
    return "ai_models";
  }
  if (
    text.includes("browser") ||
    text.includes("http") ||
    text.includes("web")
  ) {
    return "web";
  }
  return "tools";
}

function categoryForTab(tab: Tab): UserActivityCategory {
  if (tab === "web") return "web";
  if (tab === "email") return "email";
  if (tab === "apps") return "apps";
  if (tab === "commands") return "commands";
  if (tab === "ai_models") return "ai_models";
  if (tab === "tools") return "tools";
  if (tab === "safety") return "safety";
  return "files";
}

function rowStatus(row: DataAppRow): UiStatus {
  if (row.source === "telemetry") return "info";
  if (row.kind === "tool" || row.kind === "observed_tool") return "ok";
  return "idle";
}

function rowStatusLabel(row: DataAppRow) {
  if (row.source === "telemetry") return "Observed";
  if (row.kind === "tool") return "Tool";
  if (row.kind === "resource") return "Registered";
  return "Known";
}

function DataCard({ row, selected }: { row: DataAppRow; selected: boolean }) {
  return (
    <article
      className={cn(
        "rounded-lg border bg-card/60 p-4 transition-all hover:border-primary/40 hover:bg-card",
        selected &&
          "border-primary/50 bg-card shadow-md ring-1 ring-primary/50",
      )}
    >
      <div className="flex items-start gap-3">
        <div className="rounded-lg bg-primary/10 p-2 text-primary">
          <Database className="h-4 w-4" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-start justify-between gap-2">
            <h3 className="truncate text-sm font-semibold">{row.title}</h3>
            <span className="rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground">
              {categoryLabel(row.category)}
            </span>
          </div>
          <p className="mt-1 truncate text-xs text-muted-foreground">
            {row.subtitle}
          </p>
          <div className="mt-3 flex flex-wrap gap-1.5">
            <span className="rounded-full border px-2 py-0.5 text-[11px] text-muted-foreground">
              Source: {row.source}
            </span>
            <Link
              to={`/activity?q=${encodeURIComponent(row.title)}`}
              className="inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] text-primary hover:bg-primary/10"
            >
              <ExternalLink className="h-3 w-3" />
              Activity
            </Link>
            <Link
              to={`/allowed-blocked?q=${encodeURIComponent(row.title)}`}
              className="rounded-full border px-2 py-0.5 text-[11px] text-primary hover:bg-primary/10"
            >
              Rules
            </Link>
          </div>
        </div>
      </div>
    </article>
  );
}

function DataDetail({
  row,
  showTechnicalDetails,
  onRefresh,
}: {
  row: DataAppRow;
  showTechnicalDetails: boolean;
  onRefresh: () => void;
}) {
  const Icon = tabs.find((tab) => tab.id === row.tab)?.icon ?? Database;
  const status = rowStatus(row);
  const statusLabel = rowStatusLabel(row);
  const { confirm } = useConfirm();

  const handleDelete = async () => {
    if (
      !(await confirm({
        title: "Delete Record",
        description: `Are you sure you want to delete ${row.title}? This cannot be undone.`,
        confirmText: "Delete",
        cancelText: "Cancel",
      }))
    ) {
      return;
    }

    try {
      if (row.kind === "tool") {
        await RegistryApi.deleteTool((row.raw as Tool).tool_id);
      } else if (row.kind === "resource") {
        await RegistryApi.deleteResource((row.raw as Resource).resource_id);
      } else {
        toast.error("Cannot delete observed items directly.");
        return;
      }
      toast.success("Successfully deleted record");
      onRefresh();
    } catch (error) {
      console.error(error);
      toast.error("Failed to delete record");
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 border-b border-border/60 pb-4 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0">
          <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            <Icon className="h-4 w-4 text-primary" />
            Data & App Record
          </div>
          <h2 className="mt-1 break-words text-2xl font-bold tracking-tight">
            {row.title}
          </h2>
          <p className="mt-1 text-sm text-muted-foreground">
            {categoryLabel(row.category)} / {row.subtitle}
          </p>
        </div>
        <span className="inline-flex w-fit rounded-full border bg-card px-3 py-1 text-xs font-medium text-muted-foreground">
          {statusLabel}
        </span>
      </div>

      <div className="grid gap-4 md:grid-cols-[280px_minmax(0,1fr)] lg:grid-cols-[300px_minmax(0,1fr)_320px]">
        <aside className="space-y-3">
          <section className="rounded-lg border bg-card/50 p-4">
            <h3 className="mb-3 flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              <span className="h-1.5 w-1.5 rounded-full bg-primary" />
              Record Summary
            </h3>
            <div className="space-y-2 text-sm">
              <div className="border-b border-border/40 pb-2">
                <div className="text-xs text-muted-foreground">Name</div>
                <div className="mt-0.5 break-words font-medium">
                  {row.title}
                </div>
              </div>
              <div className="border-b border-border/40 pb-2">
                <div className="text-xs text-muted-foreground">Category</div>
                <div className="mt-0.5 font-medium">
                  {categoryLabel(row.category)}
                </div>
              </div>
              <div className="border-b border-border/40 pb-2">
                <div className="text-xs text-muted-foreground">Source</div>
                <div className="mt-0.5 font-medium">{row.source}</div>
              </div>
              <div>
                <div className="text-xs text-muted-foreground">Kind</div>
                <div className="mt-0.5 font-medium">{labelize(row.kind)}</div>
              </div>
            </div>
          </section>
        </aside>

        <section className="min-w-0">
          <DetailPane
            title="Detail Workspace"
            subtitle="Plain-language context, activity links, rules, and technical details for this record."
            status={status}
            statusLabel={statusLabel}
            actions={
              row.kind === "tool" || row.kind === "resource"
                ? [{ label: "Delete", danger: true, onClick: handleDelete }]
                : undefined
            }
            tabs={[
              {
                id: "overview",
                label: "Overview",
                content: (
                  <div className="space-y-4">
                    <div className="rounded-lg border bg-background/60 p-4">
                      <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                        <Icon className="h-4 w-4" />
                        What this record means
                      </div>
                      <p className="mt-2 text-sm leading-6 text-muted-foreground">
                        This is a data, app, tool, or service surface that
                        Pollek can show in one place so you can check whether an
                        AI app touched it, then decide whether to keep watching,
                        ask first, or block similar activity where supported.
                      </p>
                    </div>

                    <div className="grid gap-3 md:grid-cols-3">
                      <div className="rounded-lg border bg-background/60 p-4">
                        <div className="text-xs text-muted-foreground">
                          Category
                        </div>
                        <div className="mt-1 text-sm font-semibold">
                          {categoryLabel(row.category)}
                        </div>
                      </div>
                      <div className="rounded-lg border bg-background/60 p-4">
                        <div className="text-xs text-muted-foreground">
                          Source
                        </div>
                        <div className="mt-1 text-sm font-semibold">
                          {row.source}
                        </div>
                      </div>
                      <div className="rounded-lg border bg-background/60 p-4">
                        <div className="text-xs text-muted-foreground">
                          Kind
                        </div>
                        <div className="mt-1 text-sm font-semibold">
                          {labelize(row.kind)}
                        </div>
                      </div>
                    </div>
                  </div>
                ),
              },
              {
                id: "activity",
                label: "Activity & Rules",
                content: (
                  <div className="space-y-3">
                    <div className="rounded-lg border bg-background/60 p-4">
                      <h4 className="text-sm font-semibold">
                        Review this surface in the timeline
                      </h4>
                      <p className="mt-2 text-sm leading-6 text-muted-foreground">
                        Open Activity to see which AI app used this file,
                        website, app, command, model, or tool. Open Rules to
                        review allowed and blocked behavior related to this
                        surface.
                      </p>
                    </div>
                    <div className="flex flex-wrap gap-2">
                      <Link
                        to={`/activity?q=${encodeURIComponent(row.title)}`}
                        className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
                      >
                        <ExternalLink className="h-4 w-4" />
                        Activity
                      </Link>
                      <Link
                        to={`/allowed-blocked?q=${encodeURIComponent(row.title)}`}
                        className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
                      >
                        <ShieldCheck className="h-4 w-4" />
                        Rules
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
                        <Collapsible title="Raw Data">
                          <pre className="overflow-auto rounded-none border-0 bg-transparent p-0 text-[11px]">
                            {JSON.stringify(row.raw ?? row, null, 2)}
                          </pre>
                        </Collapsible>
                      ),
                    },
                  ]
                : []),
            ]}
          />
        </section>

        <aside className="space-y-3 md:col-span-2 lg:col-span-1">
          <section className="rounded-lg border bg-card/50 p-4">
            <h3 className="text-sm font-semibold">Related Records</h3>
            <p className="mt-1 text-xs leading-5 text-muted-foreground">
              Use these links to inspect the activity timeline and matching
              rules for this data, app, website, command, model, or tool.
            </p>
            <div className="mt-3 flex flex-wrap gap-2">
              <Link
                to={`/activity?q=${encodeURIComponent(row.title)}`}
                className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
              >
                <ExternalLink className="h-4 w-4" />
                Activity
              </Link>
              <Link
                to={`/allowed-blocked?q=${encodeURIComponent(row.title)}`}
                className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
              >
                <ShieldCheck className="h-4 w-4" />
                Rules
              </Link>
            </div>
          </section>

          <section className="rounded-lg border bg-card/50 p-4">
            <h3 className="text-sm font-semibold">Observation Note</h3>
            <p className="mt-2 text-sm leading-6 text-muted-foreground">
              Pollek shows metadata for this surface. Exact read/write,
              contents, or blocking proof depends on the current OS capability,
              connector setup, and whether the AI app routes activity through a
              visible control point.
            </p>
          </section>
        </aside>
      </div>
    </div>
  );
}

export function DataAndAppsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const { mode } = useMode();
  const showTechnicalDetails = isAdvanceMode(mode);
  const [resources, setResources] = useState<Resource[]>([]);
  const [tools, setTools] = useState<Tool[]>([]);
  const [resourceInventory, setResourceInventory] = useState<any[]>([]);
  const [toolInventory, setToolInventory] = useState<any[]>([]);
  const [activity, setActivity] = useState<any[]>([]);
  const [tab, setTab] = useState<Tab>("all");
  const [search, setSearch] = useState("");
  const selectedId = searchParams.get("selected") ?? undefined;

  const fetchItems = () => {
    void RegistryApi.listResources()
      .then(setResources)
      .catch(() => setResources([]));
    void RegistryApi.listTools()
      .then(setTools)
      .catch(() => setTools([]));
    void TelemetryApi.listResourceInventory()
      .then((data: any) =>
        setResourceInventory(data.items ?? data.resources ?? data ?? []),
      )
      .catch(() => setResourceInventory([]));
    void TelemetryApi.listToolInventory()
      .then((data: any) =>
        setToolInventory(data.items ?? data.tools ?? data ?? []),
      )
      .catch(() => setToolInventory([]));
    void Promise.all([
      UserActivityApi.list({ limit: 300 }),
      RegistryApi.listAgents().catch(() => []),
      RegistryApi.listDiscoveryCandidates().catch(() => []),
      UsageApi.getEvents({ limit: 300 }).catch(() => ({ items: [] })),
    ])
      .then(([data, agents, candidates, usageEvents]) => {
        const names = buildAgentNameMap(agents, candidates);
        addUsageEventAgentAliases(names, usageEvents.items ?? []);
        setActivity(resolveActivityAgentNames(data.items ?? [], names));
      })
      .catch(() => setActivity([]));
  };

  useEffect(() => {
    fetchItems();
  }, []);

  const rows = useMemo<DataAppRow[]>(() => {
    const fromResources = resources.map((resource) => {
      const category = resourceCategory(resource);
      return {
        id: `resource:${resourceName(resource)}`,
        tab: category,
        title: resourceName(resource),
        subtitle: labelize(
          (resource as any).resource_type ??
            (resource as any).kind ??
            "file or folder",
        ),
        category: categoryForTab(category),
        source: "registry",
        kind: "resource" as const,
        raw: resource,
      };
    });
    const fromTools = tools.map((tool) => {
      const category = toolCategory(tool);
      return {
        id: `tool:${toolName(tool)}`,
        tab: category,
        title: toolName(tool),
        subtitle: labelize(
          (tool as any).tool_type ?? (tool as any).kind ?? "tool",
        ),
        category: categoryForTab(category),
        source: "registry",
        kind: "tool" as const,
        raw: tool,
      };
    });
    const observedResources = resourceInventory.map((row, index) => {
      const title =
        row.label ??
        row.target_redacted ??
        row.resource_id ??
        row.uri ??
        `Observed data ${index + 1}`;
      const text = JSON.stringify(row).toLowerCase();
      const category: Tab =
        text.includes("http") || text.includes("domain")
          ? "web"
          : text.includes("email") || text.includes("calendar")
            ? "email"
            : text.includes("prompt") ||
                text.includes("injection") ||
                text.includes("pii") ||
                text.includes("secret") ||
                text.includes("redact") ||
                text.includes("guard")
              ? "safety"
              : text.includes("model") ||
                  text.includes("llm") ||
                  text.includes("token")
                ? "ai_models"
                : text.includes("command")
                  ? "commands"
                  : text.includes("app")
                    ? "apps"
                    : "files";
      return {
        id: `observed-resource:${title}:${index}`,
        tab: category,
        title,
        subtitle: `${row.read_count ?? row.access_count ?? 0} observed touches`,
        category: categoryForTab(category),
        source: "telemetry",
        kind: "observed_resource" as const,
        raw: row,
      };
    });
    const observedTools = toolInventory.map((row, index) => {
      const title =
        row.label ?? row.tool_id ?? row.name ?? `Observed tool ${index + 1}`;
      return {
        id: `observed-tool:${title}:${index}`,
        tab: "tools" as Tab,
        title,
        subtitle: `${row.call_count ?? row.invocation_count ?? 0} observed calls`,
        category: "tools" as UserActivityCategory,
        source: "telemetry",
        kind: "observed_tool" as const,
        raw: row,
      };
    });
    const observedActivity = activity
      .filter((event) =>
        [
          "files",
          "web",
          "email",
          "apps",
          "commands",
          "ai_models",
          "tools",
          "safety",
          "cost",
        ].includes(event.category),
      )
      .map((event) => {
        const category = event.category as Tab;
        const title =
          event.target_label || event.plain_summary || "Observed AI activity";
        return {
          id: `activity:${event.event_id}`,
          tab: category,
          title,
          subtitle: `${event.agent_name} - ${event.result_label}`,
          category: event.category as UserActivityCategory,
          source: "activity timeline",
          kind: "observed_activity" as const,
          raw: event,
        };
      });

    return [
      ...fromResources,
      ...fromTools,
      ...observedResources,
      ...observedTools,
      ...observedActivity,
    ];
  }, [activity, resourceInventory, resources, toolInventory, tools]);

  const filtered = rows.filter((row) => {
    const matchesTab = tab === "all" || row.tab === tab;
    const query = search.trim().toLowerCase();
    const matchesSearch =
      !query ||
      [row.title, row.subtitle, row.source]
        .join(" ")
        .toLowerCase()
        .includes(query);
    return matchesTab && matchesSearch;
  });

  const handleSelect = (rowId: string) => {
    const next = new URLSearchParams(searchParams);
    if (rowId) next.set("selected", rowId);
    else next.delete("selected");
    setSearchParams(next, { replace: true });
  };

  return (
    <div className="space-y-5">
      {!selectedId && (
        <>
          <PageHeader
            title="Data & Apps"
            subtitle="The files, websites, email, apps, and tools your AI apps have touched — grouped so you can see what matters."
            icon={Database}
          />

          <section className="rounded-lg border bg-card/60 p-4">
            <div className="flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
              <div className="flex flex-wrap gap-2">
                {tabs.map((item) => {
                  const Icon = item.icon;
                  return (
                    <button
                      key={item.id}
                      type="button"
                      onClick={() => setTab(item.id)}
                      className={cn(
                        "inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted",
                        tab === item.id && "bg-primary/10 text-primary",
                      )}
                    >
                      <Icon className="h-4 w-4" />
                      {item.label}
                    </button>
                  );
                })}
              </div>
              <label className="relative block min-w-0 xl:w-80">
                <span className="sr-only">Search data and apps</span>
                <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
                <input
                  value={search}
                  onChange={(event) => setSearch(event.target.value)}
                  placeholder="Search data, website, app, tool..."
                  className="h-9 w-full rounded-md border bg-background pl-9 pr-3 text-sm"
                />
              </label>
            </div>
          </section>
        </>
      )}

      <MasterDetailLayout
        items={filtered}
        selectedId={selectedId}
        onSelect={handleSelect}
        idSelector={(row) => row.id}
        masterLayout="grid"
        masterListClassName="grid gap-4 lg:grid-cols-2 2xl:grid-cols-3"
        detailBackLabel="Back to all data and apps"
        emptyState={
          <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
            No data or app records match this view yet.
          </div>
        }
        renderCard={(row, selected) => (
          <DataCard row={row} selected={selected} />
        )}
        renderDetail={(row) => (
          <DataDetail
            key={row.id}
            row={row}
            showTechnicalDetails={showTechnicalDetails}
            onRefresh={() => {
              const next = new URLSearchParams(searchParams);
              next.delete("selected");
              setSearchParams(next, { replace: true });
              fetchItems();
            }}
          />
        )}
      />
    </div>
  );
}
