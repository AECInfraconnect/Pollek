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
import { RegistryApi, TelemetryApi } from "../services/api";
import type { Resource, Tool } from "../services/types";
import {
  categoryLabel,
  labelize,
} from "../features/user-activity/userActivityModel";
import type { UserActivityCategory } from "../features/user-activity/types";
import { MasterDetailLayout } from "../components/master-detail/MasterDetailLayout";
import { DetailPane } from "../components/master-detail/DetailPane";
import type { UiStatus } from "../lib/status";
import { useMode } from "../context/ModeContext";
import { isAdvanceMode } from "../lib/modes";
import { cn } from "@/lib/utils";

type Tab =
  | "files"
  | "web"
  | "email"
  | "apps"
  | "commands"
  | "ai_models"
  | "tools"
  | "safety";

const tabs: Array<{ id: Tab; label: string; icon: any }> = [
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
  kind: "resource" | "tool" | "observed_resource" | "observed_tool";
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
        selected && "border-primary/50 bg-card shadow-md ring-1 ring-primary/50",
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
}: {
  row: DataAppRow;
  showTechnicalDetails: boolean;
}) {
  const Icon = tabs.find((tab) => tab.id === row.tab)?.icon ?? Database;

  return (
    <DetailPane
      title={row.title}
      subtitle={`${categoryLabel(row.category)} / ${row.subtitle}`}
      status={rowStatus(row)}
      statusLabel={rowStatusLabel(row)}
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
                  This is a data, app, tool, or service surface that Pollek can
                  show in one place so you can check whether an AI app touched
                  it, then decide whether to keep watching, ask first, or block
                  similar activity where supported.
                </p>
              </div>

              <div className="grid gap-3 md:grid-cols-3">
                <div className="rounded-lg border bg-background/60 p-4">
                  <div className="text-xs text-muted-foreground">Category</div>
                  <div className="mt-1 text-sm font-semibold">
                    {categoryLabel(row.category)}
                  </div>
                </div>
                <div className="rounded-lg border bg-background/60 p-4">
                  <div className="text-xs text-muted-foreground">Source</div>
                  <div className="mt-1 text-sm font-semibold">
                    {row.source}
                  </div>
                </div>
                <div className="rounded-lg border bg-background/60 p-4">
                  <div className="text-xs text-muted-foreground">Kind</div>
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
                  Open Activity to see which AI app used this file, website,
                  app, command, model, or tool. Open Rules to review allowed and
                  blocked behavior related to this surface.
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
                  <pre className="overflow-auto rounded-lg border bg-muted/40 p-4 text-[11px]">
                    {JSON.stringify(row.raw ?? row, null, 2)}
                  </pre>
                ),
              },
            ]
          : []),
      ]}
    />
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
  const [tab, setTab] = useState<Tab>("files");
  const [search, setSearch] = useState("");
  const selectedId = searchParams.get("selected") ?? undefined;

  useEffect(() => {
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

    return [
      ...fromResources,
      ...fromTools,
      ...observedResources,
      ...observedTools,
    ];
  }, [resourceInventory, resources, toolInventory, tools]);

  const filtered = rows.filter((row) => {
    const matchesTab = row.tab === tab;
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
      <div>
        <h2 className="flex items-center gap-2 text-2xl font-bold tracking-tight">
          <Database className="h-6 w-6 text-primary" />
          Data & Apps
        </h2>
        <p className="text-sm text-muted-foreground">
          Files, folders, websites, email, apps, commands, AI APIs, safety
          guards, and tools touched by AI apps.
        </p>
      </div>

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

      <MasterDetailLayout
        items={filtered}
        selectedId={selectedId}
        onSelect={handleSelect}
        idSelector={(row) => row.id}
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
          />
        )}
      />
    </div>
  );
}
