import { type ReactNode, useMemo, useState } from "react";
import { Bot, Coins, Zap } from "lucide-react";
import { cn } from "../../lib/utils";
import { TechnicalDetails } from "../ui/TechnicalDetails";
import { UsageBar, type UsageSegment, TOKEN_SEGMENT_COLORS } from "../ui/UsageBar";

export interface AgentUsageRow {
  agentKey: string;
  agentName: string;
  agentType?: string;
  inputTokens: number;
  outputTokens: number;
  cachedTokens: number;
  totalTokens: number;
  cost: number;
  /** Optional provider credit units, when a provider bills in credits. */
  credit?: number;
  calls: number;
  exact: number;
  estimated: number;
  surfaces?: string[];
  /** Technical detail rendered behind a per-row disclosure (pools, ids...). */
  detail?: ReactNode;
}

type Metric = "tokens" | "cost" | "credit";

function intFmt(value: number) {
  return new Intl.NumberFormat().format(Math.round(value || 0));
}

function moneyFmt(value: number, currency = "USD") {
  return new Intl.NumberFormat(undefined, {
    style: "currency",
    currency,
    maximumFractionDigits: value < 1 ? 4 : 2,
  }).format(value || 0);
}

function metricValue(row: AgentUsageRow, metric: Metric) {
  if (metric === "cost") return row.cost;
  if (metric === "credit") return row.credit ?? 0;
  return row.totalTokens;
}

function tokenSegments(row: AgentUsageRow): UsageSegment[] {
  return [
    {
      id: "input",
      label: "Input",
      value: row.inputTokens,
      className: TOKEN_SEGMENT_COLORS.input,
    },
    {
      id: "output",
      label: "Output",
      value: row.outputTokens,
      className: TOKEN_SEGMENT_COLORS.output,
    },
    {
      id: "cached",
      label: "Cached",
      value: row.cachedTokens,
      className: TOKEN_SEGMENT_COLORS.cached,
    },
  ].filter((segment) => segment.value > 0);
}

/**
 * Side-by-side, plain-language usage comparison across AI apps. Each app is one
 * row with a proportional bar; the metric toggle switches the same rows between
 * token input/output split, cost, and provider credit so a non-technical user
 * can compare "who used the most" at a glance. Anything technical (provider and
 * model pools, exact vs estimated ids) stays behind a per-row Technical details
 * panel.
 */
export function AgentUsageComparison({
  rows,
  currency = "USD",
  title = "How much each AI app used",
  description = "Compare AI apps side by side. Switch between tokens, cost, or provider credit — the bars stay proportional so the biggest user is obvious.",
  defaultMetric = "tokens",
  emptyLabel = "No usage recorded in this range yet.",
  className,
  headerEnd,
}: {
  rows: AgentUsageRow[];
  currency?: string;
  title?: string;
  description?: string;
  defaultMetric?: Metric;
  emptyLabel?: string;
  className?: string;
  headerEnd?: ReactNode;
}) {
  const hasCredit = rows.some((row) => (row.credit ?? 0) > 0);
  const [metric, setMetric] = useState<Metric>(defaultMetric);
  const activeMetric: Metric =
    metric === "credit" && !hasCredit ? "tokens" : metric;

  const sorted = useMemo(() => {
    return [...rows].sort(
      (a, b) =>
        metricValue(b, activeMetric) - metricValue(a, activeMetric) ||
        b.totalTokens - a.totalTokens ||
        a.agentName.localeCompare(b.agentName),
    );
  }, [rows, activeMetric]);

  const sharedMax = useMemo(
    () =>
      sorted.reduce(
        (max, row) => Math.max(max, metricValue(row, activeMetric)),
        0,
      ),
    [sorted, activeMetric],
  );

  const metricTabs: Array<{ id: Metric; label: string; icon: typeof Zap }> = [
    { id: "tokens", label: "Tokens", icon: Zap },
    { id: "cost", label: "Cost", icon: Coins },
    ...(hasCredit
      ? [{ id: "credit" as Metric, label: "Credit", icon: Coins }]
      : []),
  ];

  return (
    <section className={cn("rounded-lg border bg-card/60 p-5", className)}>
      <div className="flex flex-col gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <Bot className="h-4 w-4 shrink-0 text-primary" />
            <h3 className="font-semibold">{title}</h3>
          </div>
          <p className="mt-1 max-w-2xl text-sm leading-6 text-muted-foreground">
            {description}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {headerEnd}
          <div
            role="tablist"
            aria-label="Usage metric"
            className="inline-flex h-9 overflow-hidden rounded-md border bg-background"
          >
            {metricTabs.map((tab) => {
              const Icon = tab.icon;
              const active = activeMetric === tab.id;
              return (
                <button
                  key={tab.id}
                  role="tab"
                  aria-selected={active}
                  type="button"
                  onClick={() => setMetric(tab.id)}
                  className={cn(
                    "inline-flex items-center gap-1.5 px-3 text-sm transition-colors hover:bg-muted",
                    active && "bg-muted font-medium text-foreground",
                  )}
                >
                  <Icon className="h-3.5 w-3.5" />
                  {tab.label}
                </button>
              );
            })}
          </div>
        </div>
      </div>

      {sorted.length === 0 ? (
        <div className="mt-4 flex h-28 items-center justify-center rounded-lg border border-dashed text-sm text-muted-foreground">
          {emptyLabel}
        </div>
      ) : (
        <ol className="mt-4 space-y-3">
          {sorted.map((row) => (
            <li key={row.agentKey}>
              <AgentUsageRowView
                row={row}
                metric={activeMetric}
                sharedMax={sharedMax}
                currency={currency}
              />
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

function AgentUsageRowView({
  row,
  metric,
  sharedMax,
  currency,
}: {
  row: AgentUsageRow;
  metric: Metric;
  sharedMax: number;
  currency: string;
}) {
  const totalLabel =
    metric === "cost"
      ? moneyFmt(row.cost, currency)
      : metric === "credit"
        ? `${intFmt(row.credit ?? 0)} credit`
        : `${intFmt(row.totalTokens)} tokens`;

  const segments: UsageSegment[] =
    metric === "tokens"
      ? tokenSegments(row)
      : [
          {
            id: metric,
            label: metric === "cost" ? "Cost" : "Credit",
            value: metricValue(row, metric),
            className: metric === "cost" ? "bg-primary" : "bg-amber-500",
            formatted: totalLabel,
          },
        ];

  return (
    <div className="rounded-lg border bg-background/50 p-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="truncate text-sm font-semibold">{row.agentName}</span>
          <span className="shrink-0 rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground">
            {row.agentType || "AI app"}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-2 text-xs text-muted-foreground">
          <span className="font-semibold tabular-nums text-foreground">
            {totalLabel}
          </span>
          <span>·</span>
          <span>{intFmt(row.calls)} calls</span>
        </div>
      </div>

      <div className="mt-2.5">
        <UsageBar
          segments={segments}
          max={sharedMax}
          height="md"
          showLegend={metric === "tokens"}
          ariaLabel={`${row.agentName}: ${totalLabel}`}
        />
      </div>

      <div className="mt-2 flex flex-wrap items-center gap-1.5">
        {row.estimated > 0 && (
          <span className="rounded-full bg-amber-500/10 px-2 py-0.5 text-[11px] text-amber-700 dark:text-amber-300">
            {intFmt(row.estimated)} estimated
          </span>
        )}
        {row.exact > 0 && (
          <span className="rounded-full bg-emerald-500/10 px-2 py-0.5 text-[11px] text-emerald-700 dark:text-emerald-300">
            {intFmt(row.exact)} exact
          </span>
        )}
        {(row.surfaces ?? []).slice(0, 3).map((surface) => (
          <span
            key={surface}
            className="rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground"
          >
            {surface.replace(/_/g, " ")}
          </span>
        ))}
      </div>

      {row.detail && (
        <TechnicalDetails
          className="mt-3 bg-transparent"
          label="Provider & model detail"
          hint="Exact ids, provider/model pools, and billing notes"
        >
          {row.detail}
        </TechnicalDetails>
      )}
    </div>
  );
}
