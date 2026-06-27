import { useCallback, useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import {
  AppWindow,
  Activity,
  AlertTriangle,
  ArrowRight,
  Clock3,
  Database,
  Download,
  Eye,
  FileText,
  FolderOpen,
  Globe2,
  Info,
  Mail,
  RefreshCw,
  Search,
  ShieldCheck,
  ShieldX,
  Sparkles,
  Terminal,
  Wrench,
} from "lucide-react";
import { UserActivityApi } from "../features/user-activity/api";
import { MasterDetailLayout } from "../components/master-detail/MasterDetailLayout";
import { EntityCard } from "../components/master-detail/EntityCard";
import { DetailPane } from "../components/master-detail/DetailPane";
import { ReferenceIntelGuide } from "../components/reference/ReferenceIntelGuide";
import {
  categoryLabel,
  formatDateTime,
  summarizeActivities,
} from "../features/user-activity/userActivityModel";
import type {
  UserActivityCategory,
  UserActivityResult,
  UserFriendlyActivityEvent,
} from "../features/user-activity/types";
import {
  LocalObserveApi,
  type LocalObserveRefreshResponse,
} from "../services/api";
import { findAgentReferenceIntel } from "../lib/entityReferenceIntel";
import type { UiStatus } from "../lib/status";
import { useMode } from "../context/ModeContext";
import { isAdvanceMode } from "../lib/modes";
import { cn } from "@/lib/utils";

type Filters = {
  search: string;
  category: "" | UserActivityCategory;
  result: "" | UserActivityResult;
  agent: string;
};

const categories: UserActivityCategory[] = [
  "files",
  "web",
  "email",
  "apps",
  "commands",
  "safety",
  "ai_models",
  "tools",
  "cost",
  "unknown",
];

const categoryIcons: Record<UserActivityCategory, any> = {
  files: FolderOpen,
  web: Globe2,
  email: Mail,
  apps: AppWindow,
  commands: Terminal,
  ai_models: Sparkles,
  tools: Wrench,
  safety: ShieldCheck,
  cost: Database,
  unknown: Activity,
};

function statusForResult(result: UserActivityResult): UiStatus {
  if (result === "blocked" || result === "asked_and_denied") return "failed";
  if (result === "warned" || result === "asked_first") return "degraded";
  if (
    result === "allowed" ||
    result === "asked_and_allowed" ||
    result === "redacted"
  ) {
    return "ok";
  }
  if (result === "error") return "failed";
  return "info";
}

function formatShortTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function observedTermsForActivity(item: UserFriendlyActivityEvent) {
  return [
    item.agent_name,
    item.category,
    item.action,
    item.access_mode,
    item.target_kind,
    item.target_label,
    item.plain_summary,
    item.capability_note,
    item.next_step,
    item.rule_label,
    item.result,
    item.result_label,
    item.advanced?.decision,
    item.advanced?.mode,
    item.advanced?.pep_plane,
  ];
}

function actionPhrase(item: UserFriendlyActivityEvent) {
  const action = item.action.replace(/_/g, " ");
  if (item.access_mode === "write") return `tried to change ${item.target_label}`;
  if (item.access_mode === "read") return `read or inspected ${item.target_label}`;
  if (item.access_mode === "connect") return `connected to ${item.target_label}`;
  if (item.access_mode === "run") return `ran ${item.target_label}`;
  if (item.access_mode === "send") return `sent data through ${item.target_label}`;
  return `${action} ${item.target_label}`;
}

function resultExplanation(item: UserFriendlyActivityEvent) {
  if (item.result === "blocked" || item.result === "asked_and_denied") {
    return "Pollek stopped this action from continuing.";
  }
  if (item.result === "asked_first") {
    return "Pollek identified an action that should ask before continuing.";
  }
  if (item.result === "warned") {
    return "Pollek let the action continue and raised a warning for review.";
  }
  if (item.result === "redacted") {
    return "Pollek masked or removed sensitive data before the action continued.";
  }
  if (item.result === "watched_only") {
    return "Pollek observed this action and recorded it for the timeline.";
  }
  if (item.result === "error") {
    return "Pollek could not fully classify this action. Review the evidence before creating a rule.";
  }
  return "Pollek observed this action and it was allowed.";
}

function setupHint(item: UserFriendlyActivityEvent) {
  if (item.result === "blocked" || item.result === "redacted") {
    return "This path is already controlled by a local rule or guard.";
  }
  if (item.category === "files") {
    return "To control this, use a folder rule in Pollek when supported, or restrict file access in the AI app settings.";
  }
  if (item.category === "web") {
    return "To control this, set allowed websites here when supported, or limit web/network access in the AI app settings.";
  }
  if (item.category === "commands" || item.category === "apps") {
    return "To control this, ask before commands here when supported, or disable command execution in the AI app.";
  }
  if (item.category === "email") {
    return "To control this, review connector permissions in the AI app and keep email access opt-in.";
  }
  return "Keep observing first, then create a rule when the same activity matters enough to control.";
}

function exportJson(items: UserFriendlyActivityEvent[]) {
  const blob = new Blob([JSON.stringify(items, null, 2)], {
    type: "application/json",
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = "pollek-ai-activity.json";
  link.click();
  URL.revokeObjectURL(url);
}

function exportCsv(items: UserFriendlyActivityEvent[]) {
  const header = [
    "timestamp",
    "ai_app",
    "category",
    "action",
    "target",
    "result",
    "rule",
    "capability_note",
    "next_step",
  ];
  const rows = items.map((item) => [
    item.timestamp,
    item.agent_name,
    item.category,
    item.action,
    item.target_label,
    item.result,
    item.rule_label ?? "",
    item.capability_note,
    item.next_step,
  ]);
  const csv = [header, ...rows]
    .map((row) =>
      row.map((cell) => `"${String(cell).replaceAll('"', '""')}"`).join(","),
    )
    .join("\n");
  const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = "pollek-ai-activity.csv";
  link.click();
  URL.revokeObjectURL(url);
}

function ActivityResultIcon({ result }: { result: UserActivityResult }) {
  if (result === "blocked" || result === "asked_and_denied") {
    return <ShieldX className="h-4 w-4 text-red-500" />;
  }
  if (result === "allowed" || result === "asked_and_allowed") {
    return <ShieldCheck className="h-4 w-4 text-emerald-500" />;
  }
  if (result === "redacted")
    return <ShieldCheck className="h-4 w-4 text-violet-500" />;
  if (result === "error")
    return <AlertTriangle className="h-4 w-4 text-red-500" />;
  return <Eye className="h-4 w-4 text-blue-500" />;
}

function SummaryTile({
  label,
  value,
}: {
  label: string;
  value: string | number;
}) {
  return (
    <div className="rounded-lg border bg-card/60 p-4">
      <div className="text-2xl font-semibold">{value}</div>
      <p className="mt-1 text-xs font-medium text-muted-foreground">{label}</p>
    </div>
  );
}

function ActivityDetail({
  item,
  showTechnicalDetails,
}: {
  item: UserFriendlyActivityEvent;
  showTechnicalDetails: boolean;
}) {
  const reference = findAgentReferenceIntel({
    name: item.agent_name,
    agentType: item.category,
  })[0];
  const observedTerms = observedTermsForActivity(item);
  const status = statusForResult(item.result);
  const Icon = categoryIcons[item.category] ?? Activity;

  return (
    <DetailPane
      title={item.plain_summary}
      subtitle={`${item.agent_name} - ${formatDateTime(item.timestamp)}`}
      status={status}
      statusLabel={item.result_label}
      tabs={[
        {
          id: "overview",
          label: "Overview",
          content: (
            <div className="space-y-4">
              <div className="rounded-lg border bg-background/60 p-4">
                <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  What happened
                </div>
                <p className="mt-2 text-sm leading-6">
                  <span className="font-semibold">{item.agent_name}</span>{" "}
                  {actionPhrase(item)}.
                </p>
                <p className="mt-2 text-sm leading-6 text-muted-foreground">
                  {resultExplanation(item)}
                </p>
              </div>

              <div className="grid gap-3 md:grid-cols-3">
                <div className="rounded-lg border bg-background/60 p-4">
                  <div className="flex items-center gap-2 text-xs text-muted-foreground">
                    <ActivityResultIcon result={item.result} />
                    Result
                  </div>
                  <div className="mt-2 text-sm font-semibold">
                    {item.result_label}
                  </div>
                </div>
                <div className="rounded-lg border bg-background/60 p-4">
                  <div className="flex items-center gap-2 text-xs text-muted-foreground">
                    <Icon className="h-4 w-4" />
                    Activity type
                  </div>
                  <div className="mt-2 text-sm font-semibold">
                    {categoryLabel(item.category)}
                  </div>
                </div>
                <div className="rounded-lg border bg-background/60 p-4">
                  <div className="flex items-center gap-2 text-xs text-muted-foreground">
                    <Clock3 className="h-4 w-4" />
                    Time
                  </div>
                  <div className="mt-2 text-sm font-semibold">
                    {formatShortTime(item.timestamp)}
                  </div>
                </div>
              </div>

              <div className="grid gap-3 md:grid-cols-2">
                <div className="rounded-lg border bg-background/60 p-4">
                  <div className="text-xs text-muted-foreground">AI app</div>
                  <div className="mt-1 break-words text-sm font-semibold">
                    {item.agent_name}
                  </div>
                </div>
                <div className="rounded-lg border bg-background/60 p-4">
                  <div className="text-xs text-muted-foreground">
                    Touched or used
                  </div>
                  <div className="mt-1 break-words text-sm font-semibold">
                    {item.target_label}
                  </div>
                  <div className="mt-1 text-xs text-muted-foreground">
                    {item.target_kind} - {item.access_mode}
                  </div>
                </div>
              </div>

              <div className="rounded-lg border bg-background/60 p-4">
                <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  What you can do next
                </div>
                <p className="mt-2 text-sm leading-6 text-muted-foreground">
                  {setupHint(item)}
                </p>
              </div>

              <ReferenceIntelGuide
                reference={reference}
                observedTerms={observedTerms}
              />
            </div>
          ),
        },
        {
          id: "evidence",
          label: "Evidence",
          content: (
            <div className="space-y-3">
              <div className="rounded-lg border border-blue-500/20 bg-blue-500/10 p-4 text-sm text-blue-700">
                {item.capability_note}
              </div>
              <div className="grid gap-3 md:grid-cols-2">
                <div className="rounded-lg border bg-background/60 p-4">
                  <h4 className="text-sm font-semibold">What Pollek saw</h4>
                  <dl className="mt-3 space-y-2 text-sm">
                    <div className="flex justify-between gap-3">
                      <dt className="text-muted-foreground">Action</dt>
                      <dd className="text-right font-medium">
                        {item.action.replace(/_/g, " ")}
                      </dd>
                    </div>
                    <div className="flex justify-between gap-3">
                      <dt className="text-muted-foreground">Access</dt>
                      <dd className="text-right font-medium">
                        {item.access_mode}
                      </dd>
                    </div>
                    <div className="flex justify-between gap-3">
                      <dt className="text-muted-foreground">Category</dt>
                      <dd className="font-medium">
                        {categoryLabel(item.category)}
                      </dd>
                    </div>
                    <div className="flex justify-between gap-3">
                      <dt className="text-muted-foreground">Trace</dt>
                      <dd className="break-all text-right font-medium">
                        {item.trace_id ?? "Not linked"}
                      </dd>
                    </div>
                  </dl>
                </div>
                <div className="rounded-lg border bg-background/60 p-4">
                  <h4 className="text-sm font-semibold">Policy result</h4>
                  <dl className="mt-3 space-y-2 text-sm">
                    <div className="flex justify-between gap-3">
                      <dt className="text-muted-foreground">Rule</dt>
                      <dd className="break-words text-right font-medium">
                        {item.rule_label ?? "No rule matched"}
                      </dd>
                    </div>
                    <div className="flex justify-between gap-3">
                      <dt className="text-muted-foreground">Decision</dt>
                      <dd className="font-medium">
                        {item.advanced?.decision ?? item.result}
                      </dd>
                    </div>
                    <div className="flex justify-between gap-3">
                      <dt className="text-muted-foreground">Mode</dt>
                      <dd className="font-medium">
                        {item.advanced?.mode ?? "watch"}
                      </dd>
                    </div>
                    <div className="flex justify-between gap-3">
                      <dt className="text-muted-foreground">Source</dt>
                      <dd className="break-words text-right font-medium">
                        {item.advanced?.pep_plane ?? "local observe"}
                      </dd>
                    </div>
                  </dl>
                </div>
              </div>
              <p className="rounded-lg border bg-background/60 p-4 text-xs leading-5 text-muted-foreground">
                {item.privacy_note}
              </p>
            </div>
          ),
        },
        {
          id: "next",
          label: "Next Steps",
          content: (
            <div className="space-y-3">
              <div className="rounded-lg border bg-background/60 p-4">
                <h4 className="flex items-center gap-2 text-sm font-semibold">
                  <ArrowRight className="h-4 w-4 text-primary" />
                  Suggested action
                </h4>
                <p className="mt-2 text-sm leading-6 text-muted-foreground">
                  {item.next_step}
                </p>
              </div>
              <div className="rounded-lg border bg-background/60 p-4">
                <h4 className="flex items-center gap-2 text-sm font-semibold">
                  <Info className="h-4 w-4 text-primary" />
                  Plain explanation
                </h4>
                <p className="mt-2 text-sm leading-6 text-muted-foreground">
                  This event means {item.agent_name} {actionPhrase(item)}. You
                  can keep watching it, ask before similar actions, or block
                  this kind of activity where your device supports that control.
                </p>
              </div>
              <div className="rounded-lg border bg-background/60 p-4">
                <h4 className="flex items-center gap-2 text-sm font-semibold">
                  <ShieldCheck className="h-4 w-4 text-primary" />
                  Agent-side option
                </h4>
                <p className="mt-2 text-sm leading-6 text-muted-foreground">
                  When Pollek can only observe this path, use this record as a
                  checklist for the AI app's own settings: file permissions,
                  web access, connector permissions, terminal access, and model
                  usage controls.
                </p>
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
                  <div className="space-y-3">
                    <div className="grid gap-3 md:grid-cols-3">
                      <div className="rounded-lg border bg-background/60 p-4">
                        <div className="text-xs text-muted-foreground">
                          Event ID
                        </div>
                        <div className="mt-1 break-all text-sm font-semibold">
                          {item.event_id}
                        </div>
                      </div>
                      <div className="rounded-lg border bg-background/60 p-4">
                        <div className="text-xs text-muted-foreground">
                          Agent ID
                        </div>
                        <div className="mt-1 break-all text-sm font-semibold">
                          {item.agent_id ?? "Not linked"}
                        </div>
                      </div>
                      <div className="rounded-lg border bg-background/60 p-4">
                        <div className="text-xs text-muted-foreground">
                          Trace ID
                        </div>
                        <div className="mt-1 break-all text-sm font-semibold">
                          {item.trace_id ?? "Not linked"}
                        </div>
                      </div>
                    </div>

                    <div className="grid gap-3 md:grid-cols-2">
                      <div className="rounded-lg border bg-background/60 p-4">
                        <h4 className="text-sm font-semibold">
                          Decision metadata
                        </h4>
                        <dl className="mt-3 space-y-2 text-sm">
                          <div className="flex justify-between gap-3">
                            <dt className="text-muted-foreground">Decision</dt>
                            <dd className="font-medium">
                              {item.advanced?.decision ?? item.result}
                            </dd>
                          </div>
                          <div className="flex justify-between gap-3">
                            <dt className="text-muted-foreground">Mode</dt>
                            <dd className="font-medium">
                              {item.advanced?.mode ?? "unknown"}
                            </dd>
                          </div>
                          <div className="flex justify-between gap-3">
                            <dt className="text-muted-foreground">PEP plane</dt>
                            <dd className="break-words text-right font-medium">
                              {item.advanced?.pep_plane ?? "unknown"}
                            </dd>
                          </div>
                          <div className="flex justify-between gap-3">
                            <dt className="text-muted-foreground">
                              PDP engine
                            </dt>
                            <dd className="break-words text-right font-medium">
                              {item.advanced?.pdp_engine ?? "unknown"}
                            </dd>
                          </div>
                        </dl>
                      </div>
                      <div className="rounded-lg border bg-background/60 p-4">
                        <h4 className="text-sm font-semibold">Usage fields</h4>
                        <dl className="mt-3 space-y-2 text-sm">
                          <div className="flex justify-between gap-3">
                            <dt className="text-muted-foreground">Tokens</dt>
                            <dd className="font-medium">
                              {item.tokens?.toLocaleString() ?? "Not reported"}
                            </dd>
                          </div>
                          <div className="flex justify-between gap-3">
                            <dt className="text-muted-foreground">Cost</dt>
                            <dd className="font-medium">
                              {item.cost_usd
                                ? `$${item.cost_usd.toFixed(4)}`
                                : "None"}
                            </dd>
                          </div>
                          <div className="flex justify-between gap-3">
                            <dt className="text-muted-foreground">
                              Schema version
                            </dt>
                            <dd className="font-medium">
                              {item.schema_version}
                            </dd>
                          </div>
                        </dl>
                      </div>
                    </div>

                    <pre className="overflow-auto rounded-lg border bg-muted/40 p-4 text-[11px]">
                      {JSON.stringify(item.advanced ?? {}, null, 2)}
                    </pre>
                  </div>
                ),
              },
            ]
          : []),
      ]}
    />
  );
}

export function AiActivityPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const { mode } = useMode();
  const showTechnicalDetails = isAdvanceMode(mode);
  const selectedEventId =
    searchParams.get("selected") ?? searchParams.get("event") ?? undefined;
  const [items, setItems] = useState<UserFriendlyActivityEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const [observing, setObserving] = useState(false);
  const [observeResult, setObserveResult] =
    useState<LocalObserveRefreshResponse | null>(null);
  const [filters, setFilters] = useState<Filters>({
    search: searchParams.get("q") ?? "",
    category: "",
    result: "",
    agent: "",
  });

  const load = useCallback(() => {
    setLoading(true);
    UserActivityApi.list({ limit: 300 })
      .then((response) => {
        setItems(response.items ?? []);
        setError(null);
      })
      .catch((err) =>
        setError(err instanceof Error ? err : new Error(String(err))),
      )
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    load();
    const timer = window.setInterval(load, 15000);
    return () => window.clearInterval(timer);
  }, [load]);

  const observeNow = useCallback(async () => {
    setObserving(true);
    try {
      const result = await LocalObserveApi.refresh({ include_estimates: true });
      setObserveResult(result);
      setError(null);
      toast.success(
        `Observed ${result.candidates_found} AI app(s), ${result.resource_events} resource event(s), and ${result.tool_events} tool event(s).`,
      );
      load();
    } catch (err) {
      const nextError = err instanceof Error ? err : new Error(String(err));
      setError(nextError);
      toast.error(nextError.message || "Local observe refresh failed");
    } finally {
      setObserving(false);
    }
  }, [load]);

  const agentOptions = useMemo(
    () => Array.from(new Set(items.map((item) => item.agent_name))).sort(),
    [items],
  );
  const filtered = useMemo(() => {
    const query = filters.search.trim().toLowerCase();
    return items.filter((item) => {
      if (filters.category && item.category !== filters.category) return false;
      if (filters.result && item.result !== filters.result) return false;
      if (filters.agent && item.agent_name !== filters.agent) return false;
      if (!query) return true;
      return [
        item.agent_name,
        item.target_label,
        item.plain_summary,
        item.rule_label,
        item.capability_note,
        item.next_step,
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase()
        .includes(query);
    });
  }, [filters, items]);
  const summary = useMemo(() => summarizeActivities(filtered), [filtered]);
  const handleSelectEvent = useCallback(
    (eventId: string) => {
      const next = new URLSearchParams(searchParams);
      if (eventId) {
        next.set("selected", eventId);
      } else {
        next.delete("selected");
      }
      next.delete("event");
      setSearchParams(next, { replace: true });
    },
    [searchParams, setSearchParams],
  );

  return (
    <div className="space-y-5">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <h2 className="flex items-center gap-2 text-2xl font-bold tracking-tight">
            <Activity className="h-6 w-6 text-primary" />
            AI Activity
          </h2>
          <p className="text-sm text-muted-foreground">
            Files, websites, tools, commands, model usage, and decisions in
            plain language.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={observeNow}
            disabled={observing}
            className="inline-flex h-9 items-center gap-2 rounded-md bg-primary px-3 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-60"
          >
            <Eye className={cn("h-4 w-4", observing && "animate-pulse")} />
            {observing ? "Observing" : "Observe now"}
          </button>
          <button
            type="button"
            onClick={() => exportCsv(filtered)}
            className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
          >
            <FileText className="h-4 w-4" />
            CSV
          </button>
          <button
            type="button"
            onClick={() => exportJson(filtered)}
            className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
          >
            <Download className="h-4 w-4" />
            JSON
          </button>
          <button
            type="button"
            onClick={load}
            className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
          >
            <RefreshCw className={cn("h-4 w-4", loading && "animate-spin")} />
            Refresh
          </button>
        </div>
      </div>

      {observeResult && (
        <section className="rounded-lg border bg-card/60 p-4">
          <div className="flex flex-col gap-3 xl:flex-row xl:items-start xl:justify-between">
            <div>
              <h3 className="text-sm font-semibold">
                Latest local observe refresh
              </h3>
              <p className="mt-1 max-w-3xl text-xs leading-5 text-muted-foreground">
                Pollek records activity metadata only here: redacted paths,
                domains, tools, model usage fields, decisions, and timestamps.
                It does not store file contents, email bodies, raw prompts, or
                raw responses in this timeline.
              </p>
            </div>
            <span className="rounded-full border bg-background px-2.5 py-1 text-xs text-muted-foreground">
              Scan {observeResult.scan_id}
            </span>
          </div>
          <div className="mt-4 grid gap-2 sm:grid-cols-2 xl:grid-cols-6">
            <SummaryTile
              label="AI apps observed"
              value={observeResult.candidates_found}
            />
            <SummaryTile
              label="Resources"
              value={observeResult.resource_events}
            />
            <SummaryTile label="Tools" value={observeResult.tool_events} />
            <SummaryTile
              label="Identities"
              value={observeResult.identity_events}
            />
            <SummaryTile
              label="Exact usage"
              value={observeResult.exact_usage_events}
            />
            <SummaryTile
              label="Estimated usage"
              value={observeResult.estimated_usage_events}
            />
          </div>
          {(observeResult.capture_quality.length > 0 ||
            observeResult.limitations.length > 0) && (
            <div className="mt-3 grid gap-2 lg:grid-cols-2">
              <div className="rounded-md border bg-background/60 p-3">
                <div className="text-xs font-medium">Capture quality</div>
                <p className="mt-1 text-xs text-muted-foreground">
                  {observeResult.capture_quality.length > 0
                    ? observeResult.capture_quality.join(", ")
                    : "Metadata observed; no exact usage source reported yet."}
                </p>
              </div>
              <div className="rounded-md border bg-background/60 p-3">
                <div className="text-xs font-medium">What may need setup</div>
                <ul className="mt-1 space-y-1 text-xs text-muted-foreground">
                  {observeResult.limitations.slice(0, 3).map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                  {observeResult.limitations.length === 0 && (
                    <li>No limitations were reported by this refresh.</li>
                  )}
                </ul>
              </div>
            </div>
          )}
        </section>
      )}

      <section className="grid gap-3 sm:grid-cols-2 xl:grid-cols-7">
        <SummaryTile label="Events" value={summary.total} />
        <SummaryTile label="File activity" value={summary.files} />
        <SummaryTile label="Web activity" value={summary.web} />
        <SummaryTile label="Commands" value={summary.commands} />
        <SummaryTile label="Safety" value={summary.safety} />
        <SummaryTile label="Blocked" value={summary.blocked} />
        <SummaryTile
          label="Estimated cost"
          value={`$${summary.costUsd.toFixed(2)}`}
        />
      </section>

      <section className="rounded-lg border bg-card/60 p-4">
        <div className="grid gap-3 lg:grid-cols-[1.5fr_0.9fr_0.9fr_0.9fr]">
          <label className="relative block">
            <span className="sr-only">Search activity</span>
            <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
            <input
              value={filters.search}
              onChange={(event) =>
                setFilters((current) => ({
                  ...current,
                  search: event.target.value,
                }))
              }
              placeholder="Search AI app, file, folder, website, command..."
              className="h-9 w-full rounded-md border bg-background pl-9 pr-3 text-sm"
            />
          </label>
          <select
            value={filters.agent}
            onChange={(event) =>
              setFilters((current) => ({
                ...current,
                agent: event.target.value,
              }))
            }
            className="h-9 rounded-md border bg-background px-3 text-sm"
          >
            <option value="">All AI apps</option>
            {agentOptions.map((agent) => (
              <option key={agent} value={agent}>
                {agent}
              </option>
            ))}
          </select>
          <select
            value={filters.category}
            onChange={(event) =>
              setFilters((current) => ({
                ...current,
                category: event.target.value as Filters["category"],
              }))
            }
            className="h-9 rounded-md border bg-background px-3 text-sm"
          >
            <option value="">All activity</option>
            {categories.map((category) => (
              <option key={category} value={category}>
                {categoryLabel(category)}
              </option>
            ))}
          </select>
          <select
            value={filters.result}
            onChange={(event) =>
              setFilters((current) => ({
                ...current,
                result: event.target.value as Filters["result"],
              }))
            }
            className="h-9 rounded-md border bg-background px-3 text-sm"
          >
            <option value="">All results</option>
            <option value="watched_only">Watched only</option>
            <option value="allowed">Allowed</option>
            <option value="blocked">Blocked</option>
            <option value="asked_first">Ask first</option>
            <option value="warned">Warned</option>
            <option value="redacted">Redacted</option>
            <option value="error">Error</option>
          </select>
        </div>
      </section>

      {error && (
        <div className="rounded-lg border border-amber-500/20 bg-amber-500/10 p-4 text-sm text-amber-700">
          {error.message}
        </div>
      )}

      <MasterDetailLayout
        items={filtered}
        selectedId={selectedEventId}
        onSelect={handleSelectEvent}
        idSelector={(item) => item.event_id}
        loading={loading && items.length === 0}
        emptyState={
          <div className="rounded-lg border border-dashed p-8 text-center">
            <Activity className="mx-auto h-8 w-8 text-muted-foreground/60" />
            <p className="mt-3 text-sm font-medium">
              No AI activity matches this view yet
            </p>
            <p className="mx-auto mt-2 max-w-md text-sm leading-6 text-muted-foreground">
              Run Observe now while ChatGPT, Claude, Codex, DeepSeek, Manus, or
              Antigravity is active. Pollek will record metadata about files,
              websites, tools, commands, model usage, and decisions when the
              local host can see them.
            </p>
            <button
              type="button"
              onClick={observeNow}
              disabled={observing}
              className="mt-4 inline-flex h-9 items-center gap-2 rounded-md bg-primary px-3 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-60"
            >
              <Eye className={cn("h-4 w-4", observing && "animate-pulse")} />
              {observing ? "Observing" : "Observe now"}
            </button>
          </div>
        }
        renderGroupHeader={(item, index, prevItem) => {
          const day = new Date(item.timestamp).toDateString();
          const prevDay = prevItem
            ? new Date(prevItem.timestamp).toDateString()
            : null;
          if (index > 0 && day === prevDay) return null;
          return (
            <div className="px-2 py-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              {formatShortTime(item.timestamp).split(",")[0] || day}
            </div>
          );
        }}
        renderCard={(item, selected) => {
          const Icon = categoryIcons[item.category] ?? Activity;
          return (
            <EntityCard
              title={item.plain_summary}
              subtitle={`${item.agent_name} - ${formatShortTime(
                item.timestamp,
              )}`}
              summary={`${resultExplanation(item)} Target: ${item.target_label}`}
              icon={Icon}
              status={statusForResult(item.result)}
              statusLabel={item.result_label}
              meta={[
                { label: "AI app", value: item.agent_name },
                { label: "Type", value: categoryLabel(item.category) },
                { label: "Access", value: item.access_mode },
                ...(item.rule_label
                  ? [{ label: "Rule", value: item.rule_label }]
                  : []),
              ]}
              selected={selected}
            />
          );
        }}
        renderDetail={(item) => (
          <ActivityDetail
            key={item.event_id}
            item={item}
            showTechnicalDetails={showTechnicalDetails}
          />
        )}
      />
    </div>
  );
}
