import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import {
  Activity,
  BarChart3,
  CalendarClock,
  CalendarDays,
  CloudOff,
  Download,
  FileText,
  HardDrive,
  History,
  RefreshCw,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import { UserActivityApi } from "../features/user-activity/api";
import {
  categoryLabel,
  summarizeActivities,
} from "../features/user-activity/userActivityModel";
import type { UserFriendlyActivityEvent } from "../features/user-activity/types";
import { useConfirm } from "../components/ui/ConfirmDialog";
import { PageHeader } from "../components/layout/PageHeader";
import { cn } from "@/lib/utils";

type Range = "7d" | "30d" | "all";
type RetentionPreference = "7d" | "30d" | "until_deleted" | "no_history";

function exportReport(
  items: UserFriendlyActivityEvent[],
  format: "json" | "csv",
) {
  if (format === "json") {
    const blob = new Blob([JSON.stringify(items, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = "pollek-ai-history.json";
    link.click();
    URL.revokeObjectURL(url);
    return;
  }

  const rows = items.map((item) => [
    item.timestamp,
    item.agent_name,
    item.category,
    item.action,
    item.target_label,
    item.result_label,
    item.rule_label ?? "",
    item.capability_note,
  ]);
  const csv = [
    [
      "timestamp",
      "ai_app",
      "category",
      "action",
      "target",
      "result",
      "rule",
      "capability_note",
    ],
    ...rows,
  ]
    .map((row) =>
      row.map((cell) => `"${String(cell).replaceAll('"', '""')}"`).join(","),
    )
    .join("\n");
  const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = "pollek-ai-history.csv";
  link.click();
  URL.revokeObjectURL(url);
}

function inRange(item: UserFriendlyActivityEvent, range: Range) {
  if (range === "all") return true;
  const date = new Date(item.timestamp);
  if (Number.isNaN(date.getTime())) return true;
  const days = range === "7d" ? 7 : 30;
  return Date.now() - date.getTime() <= days * 24 * 60 * 60 * 1000;
}

function countBy<T extends string>(
  items: UserFriendlyActivityEvent[],
  select: (item: UserFriendlyActivityEvent) => T,
) {
  const counts = new Map<T, number>();
  for (const item of items) {
    const key = select(item);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return Array.from(counts, ([key, count]) => ({ key, count })).sort(
    (left, right) => right.count - left.count,
  );
}

function ReportRow({
  label,
  count,
  total,
}: {
  label: string;
  count: number;
  total: number;
}) {
  const percent = total > 0 ? Math.round((count / total) * 100) : 0;
  return (
    <div className="rounded-md border bg-background/60 p-3">
      <div className="flex items-center justify-between gap-3 text-sm">
        <span className="min-w-0 truncate font-medium">{label}</span>
        <span className="shrink-0 text-muted-foreground">{count}</span>
      </div>
      <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-muted">
        <div
          className="h-full rounded-full bg-primary"
          style={{ width: `${percent}%` }}
        />
      </div>
    </div>
  );
}

export function HistoryReportsPage() {
  const { confirm } = useConfirm();
  const [allItems, setAllItems] = useState<UserFriendlyActivityEvent[]>([]);
  const [range, setRange] = useState<Range>("7d");
  const [retentionPreference, setRetentionPreference] =
    useState<RetentionPreference>(() => {
      const saved = localStorage.getItem("pollek.history.retentionPreference");
      return saved === "7d" ||
        saved === "30d" ||
        saved === "until_deleted" ||
        saved === "no_history"
        ? saved
        : "30d";
    });
  const [loading, setLoading] = useState(true);

  const load = useCallback(() => {
    setLoading(true);
    UserActivityApi.list({ limit: 1000 })
      .then((response) => setAllItems(response.items ?? []))
      .catch(() => setAllItems([]))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    localStorage.setItem(
      "pollek.history.retentionPreference",
      retentionPreference,
    );
  }, [retentionPreference]);

  const clearLocalHistory = useCallback(async () => {
    if (
      !(await confirm({
        title: "Delete local activity history",
        description:
          "This clears local activity, decision, Prompt Guard, and plugin audit history used by AI Activity. It does not delete exported files or separate cloud records.",
        danger: true,
        confirmText: "Delete history",
      }))
    ) {
      return;
    }

    try {
      const result = await UserActivityApi.clearLocalHistory();
      setAllItems([]);
      const deleted =
        result.observation_events +
        result.decision_logs +
        result.decisions +
        (result.guard_incidents ?? 0) +
        (result.guard_events ?? 0) +
        (result.plugin_audit ?? 0);
      toast.success(`Deleted ${deleted} local history record(s).`);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Failed to delete history";
      toast.error(message);
    }
  }, [confirm]);

  const items = useMemo(
    () => allItems.filter((item) => inRange(item, range)),
    [allItems, range],
  );

  const exportWithDisclosure = useCallback(
    async (format: "json" | "csv") => {
      if (items.length === 0) {
        toast.info("No history records match this view.");
        return;
      }
      if (
        !(await confirm({
          title: `Export ${format.toUpperCase()} history`,
          description:
            "This creates a file outside Pollek's protected local history. Anyone with access to that exported file may see AI app names, file or website labels, results, timestamps, and rule names. Keep exports on this device unless you intentionally share them.",
          confirmText: `Export ${format.toUpperCase()}`,
        }))
      ) {
        return;
      }
      exportReport(items, format);
    },
    [confirm, items],
  );

  const summary = useMemo(() => summarizeActivities(items), [items]);
  const byAgent = useMemo(
    () => countBy(items, (item) => item.agent_name).slice(0, 8),
    [items],
  );
  const byCategory = useMemo(
    () => countBy(items, (item) => categoryLabel(item.category)).slice(0, 8),
    [items],
  );
  const byResult = useMemo(
    () => countBy(items, (item) => item.result_label).slice(0, 8),
    [items],
  );

  return (
    <div className="space-y-5">
      <PageHeader
        title="History"
        subtitle="Review what each AI app did, what was allowed, and what was blocked."
        icon={History}
        actions={
          <>
            <div className="inline-flex h-9 overflow-hidden rounded-md border bg-background">
              {(["7d", "30d", "all"] as Range[]).map((option) => (
                <button
                  key={option}
                  type="button"
                  onClick={() => setRange(option)}
                  className={cn(
                    "px-3 text-sm hover:bg-muted",
                    range === option && "bg-muted text-foreground",
                  )}
                >
                  {option === "all" ? "All" : option.toUpperCase()}
                </button>
              ))}
            </div>
            <button
              type="button"
              onClick={() => void exportWithDisclosure("csv")}
              className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
            >
              <FileText className="h-4 w-4" />
              CSV
            </button>
            <button
              type="button"
              onClick={() => void exportWithDisclosure("json")}
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
            <button
              type="button"
              onClick={() => void clearLocalHistory()}
              className="inline-flex h-9 items-center gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-3 text-sm font-medium text-red-700 hover:bg-red-500/15"
            >
              <Trash2 className="h-4 w-4" />
              Delete local history
            </button>
          </>
        }
      />

      <section className="rounded-lg border bg-card/60 p-4">
        <div className="flex items-start gap-3">
          <div className="rounded-lg bg-emerald-500/10 p-2 text-emerald-700">
            <ShieldCheck className="h-4 w-4" />
          </div>
          <div>
            <h3 className="text-sm font-semibold">Privacy and retention</h3>
            <p className="mt-1 text-sm leading-6 text-muted-foreground">
              This view shows activity metadata such as AI app, file or website
              label, result, timestamp, and rule. It does not display file
              contents, email bodies, raw prompts, or raw responses. Use the
              range selector for review, export CSV/JSON only when you need a
              copy, or delete local observation and decision history from this
              device.
            </p>
            <div className="mt-4 grid gap-3 lg:grid-cols-2">
              <div className="rounded-md border bg-background/60 p-3">
                <div className="flex items-start gap-2">
                  <HardDrive className="mt-0.5 h-4 w-4 text-primary" />
                  <div>
                    <div className="text-sm font-medium">
                      Stored locally by default
                    </div>
                    <p className="mt-1 text-xs leading-5 text-muted-foreground">
                      AI Activity, Prompt Guard, decisions, and plugin audit
                      history are read from local records on this device. Pollek
                      Cloud sync is optional and separate from this local view.
                    </p>
                  </div>
                </div>
              </div>
              <div className="rounded-md border bg-background/60 p-3">
                <div className="flex items-start gap-2">
                  <CloudOff className="mt-0.5 h-4 w-4 text-primary" />
                  <div>
                    <div className="text-sm font-medium">
                      Export may leave this app
                    </div>
                    <p className="mt-1 text-xs leading-5 text-muted-foreground">
                      Exported CSV/JSON files are regular files. They can be
                      copied, synced, emailed, or uploaded by other software, so
                      Pollek asks before creating one.
                    </p>
                  </div>
                </div>
              </div>
            </div>
            <div className="mt-3 rounded-md border bg-background/60 p-3">
              <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
                <div className="flex items-start gap-2">
                  <CalendarClock className="mt-0.5 h-4 w-4 text-primary" />
                  <div>
                    <div className="text-sm font-medium">
                      Local retention preference
                    </div>
                    <p className="mt-1 text-xs leading-5 text-muted-foreground">
                      Saved on this dashboard so review/export defaults are
                      clear. No-history means use Delete local history now and
                      keep future review windows empty where the local API
                      supports it.
                    </p>
                  </div>
                </div>
                <div className="inline-flex h-9 overflow-hidden rounded-md border bg-background">
                  {(
                    [
                      ["7d", "7 days"],
                      ["30d", "30 days"],
                      ["until_deleted", "Keep until deleted"],
                      ["no_history", "No history"],
                    ] as Array<[RetentionPreference, string]>
                  ).map(([value, label]) => (
                    <button
                      key={value}
                      type="button"
                      onClick={() => setRetentionPreference(value)}
                      className={cn(
                        "px-3 text-xs hover:bg-muted sm:text-sm",
                        retentionPreference === value &&
                          "bg-muted text-foreground",
                      )}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              </div>
            </div>
            {retentionPreference === "no_history" && (
              <div className="mt-3 rounded-md border border-amber-500/25 bg-amber-500/10 p-3 text-sm text-amber-900 dark:text-amber-100">
                No-history preference is selected. Delete local history now to
                clear existing records, and use AI Activity for live observation
                rather than long-term review.
              </div>
            )}
          </div>
        </div>
      </section>

      <section className="grid gap-3 sm:grid-cols-2 xl:grid-cols-7">
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="text-2xl font-semibold">{summary.total}</div>
          <p className="mt-1 text-xs text-muted-foreground">Events</p>
        </div>
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="text-2xl font-semibold">{summary.files}</div>
          <p className="mt-1 text-xs text-muted-foreground">Files</p>
        </div>
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="text-2xl font-semibold">{summary.web}</div>
          <p className="mt-1 text-xs text-muted-foreground">Websites</p>
        </div>
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="text-2xl font-semibold">{summary.blocked}</div>
          <p className="mt-1 text-xs text-muted-foreground">Blocked</p>
        </div>
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="text-2xl font-semibold">{summary.plugins}</div>
          <p className="mt-1 text-xs text-muted-foreground">Plugins</p>
        </div>
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="text-2xl font-semibold">{summary.safety}</div>
          <p className="mt-1 text-xs text-muted-foreground">Safety</p>
        </div>
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="text-2xl font-semibold">
            ${summary.costUsd.toFixed(2)}
          </div>
          <p className="mt-1 text-xs text-muted-foreground">Estimated cost</p>
        </div>
      </section>

      <section className="grid gap-3 xl:grid-cols-3">
        <div className="rounded-lg border bg-card/60 p-4">
          <h3 className="flex items-center gap-2 text-sm font-semibold">
            <Activity className="h-4 w-4 text-primary" />
            By AI app
          </h3>
          <div className="mt-3 space-y-2">
            {byAgent.length > 0 ? (
              byAgent.map((row) => (
                <ReportRow
                  key={row.key}
                  label={row.key}
                  count={row.count}
                  total={summary.total}
                />
              ))
            ) : (
              <p className="text-sm text-muted-foreground">No activity yet.</p>
            )}
          </div>
        </div>
        <div className="rounded-lg border bg-card/60 p-4">
          <h3 className="flex items-center gap-2 text-sm font-semibold">
            <BarChart3 className="h-4 w-4 text-primary" />
            By activity type
          </h3>
          <div className="mt-3 space-y-2">
            {byCategory.length > 0 ? (
              byCategory.map((row) => (
                <ReportRow
                  key={row.key}
                  label={row.key}
                  count={row.count}
                  total={summary.total}
                />
              ))
            ) : (
              <p className="text-sm text-muted-foreground">No activity yet.</p>
            )}
          </div>
        </div>
        <div className="rounded-lg border bg-card/60 p-4">
          <h3 className="flex items-center gap-2 text-sm font-semibold">
            <CalendarDays className="h-4 w-4 text-primary" />
            By result
          </h3>
          <div className="mt-3 space-y-2">
            {byResult.length > 0 ? (
              byResult.map((row) => (
                <ReportRow
                  key={row.key}
                  label={row.key}
                  count={row.count}
                  total={summary.total}
                />
              ))
            ) : (
              <p className="text-sm text-muted-foreground">No activity yet.</p>
            )}
          </div>
        </div>
      </section>

      <section className="rounded-lg border bg-card/60 p-4">
        <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
          <div>
            <h3 className="text-sm font-semibold">Need the full timeline?</h3>
            <p className="mt-1 text-sm text-muted-foreground">
              Open AI Activity to inspect individual file, website, command, and
              tool events.
            </p>
          </div>
          <Link
            to="/activity"
            className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
          >
            <Activity className="h-4 w-4" />
            Open activity
          </Link>
        </div>
      </section>
    </div>
  );
}
