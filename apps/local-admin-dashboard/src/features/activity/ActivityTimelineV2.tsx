import { useCallback, useEffect, useMemo, useState } from "react";
import { Download, ShieldCheck } from "lucide-react";
import { useSearchParams } from "react-router-dom";
import { EntityGraphApi } from "../../services/entityGraphApi";
import { RelationshipSummaryCards } from "../entity-360/RelationshipSummaryCards";
import type {
  ActivityTimelineItem,
  ActivityTimelineResponse,
  RelationshipSummary,
} from "../entity-graph/types";
import { ActivityEventCard } from "./ActivityEventCard";
import { ActivityFilters, type TimelineFilters } from "./ActivityFilters";
import { DecisionEvidenceDrawer } from "./DecisionEvidenceDrawer";

function initialFilters(params: URLSearchParams): TimelineFilters {
  return {
    search: params.get("q") ?? "",
    decision: params.get("decision") ?? "",
    mode: params.get("mode") ?? "",
    entityType: params.get("entity_type") ?? "",
    entityId: params.get("entity_id") ?? "",
  };
}

function matchesSearch(item: ActivityTimelineItem, query: string) {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  const haystack = [
    item.event_id,
    item.trace_id,
    item.action,
    item.decision,
    item.enforcement_mode,
    item.pep_plane,
    item.pdp_engine,
    item.explanation,
    item.actor?.label,
    item.actor?.entity_id,
    item.tool?.label,
    item.tool?.entity_id,
    item.resource?.label,
    item.resource?.entity_id,
    ...item.policies.flatMap((policy) => [policy.label, policy.entity_id]),
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
  return haystack.includes(needle);
}

function timelineSummaries(
  items: ActivityTimelineItem[],
): RelationshipSummary[] {
  const denied = items.filter((item) => item.decision === "deny").length;
  const allowed = items.filter(
    (item) => item.decision === "allow" || item.decision === "ok",
  ).length;
  const enforced = items.filter(
    (item) => item.enforcement_mode === "enforce",
  ).length;
  const cost = items.reduce(
    (total, item) => total + (item.cost?.total_cost_usd ?? 0),
    0,
  );
  return [
    { kind: "events", label: "Events", count: items.length, tone: "neutral" },
    { kind: "allowed", label: "Allowed", count: allowed, tone: "success" },
    { kind: "denied", label: "Denied", count: denied, tone: "danger" },
    { kind: "enforced", label: "Enforced", count: enforced, tone: "info" },
    {
      kind: "estimated_cost_cents",
      label: "Cost cents",
      count: Math.round(cost * 100),
      tone: cost > 0 ? "warning" : "neutral",
    },
  ];
}

function exportJson(items: ActivityTimelineItem[]) {
  const blob = new Blob([JSON.stringify(items, null, 2)], {
    type: "application/json",
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = "pollek-activity-timeline.json";
  link.click();
  URL.revokeObjectURL(url);
}

function exportCsv(items: ActivityTimelineItem[]) {
  const header = [
    "timestamp",
    "event_id",
    "actor",
    "action",
    "tool",
    "resource",
    "decision",
    "mode",
    "pep",
    "pdp",
    "trace_id",
    "policies",
    "tokens",
    "cost_usd",
  ];
  const rows = items.map((item) => [
    item.timestamp,
    item.event_id,
    item.actor?.label ?? "",
    item.action,
    item.tool?.label ?? "",
    item.resource?.label ?? "",
    item.decision,
    item.enforcement_mode,
    item.pep_plane ?? "",
    item.pdp_engine ?? "",
    item.trace_id ?? "",
    item.policies.map((policy) => policy.label).join(";"),
    String(item.cost?.total_tokens ?? ""),
    String(item.cost?.total_cost_usd ?? ""),
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
  link.download = "pollek-activity-timeline.csv";
  link.click();
  URL.revokeObjectURL(url);
}

export function ActivityTimelineV2() {
  const [params, setParams] = useSearchParams();
  const [filters, setFilters] = useState<TimelineFilters>(() =>
    initialFilters(params),
  );
  const [data, setData] = useState<ActivityTimelineResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const [selected, setSelected] = useState<ActivityTimelineItem | null>(null);

  const apiQuery = useMemo(
    () => ({
      decision: filters.decision || undefined,
      mode: filters.mode || undefined,
      entity_type:
        filters.entityType && filters.entityId ? filters.entityType : undefined,
      entity_id:
        filters.entityType && filters.entityId ? filters.entityId : undefined,
      limit: 250,
    }),
    [filters.decision, filters.entityId, filters.entityType, filters.mode],
  );

  const load = useCallback(() => {
    setLoading(true);
    EntityGraphApi.getActivity(apiQuery)
      .then((response) => {
        setData(response);
        setError(null);
      })
      .catch((err) =>
        setError(err instanceof Error ? err : new Error(String(err))),
      )
      .finally(() => setLoading(false));
  }, [apiQuery]);

  useEffect(() => {
    load();
    const timer = window.setInterval(load, 10000);
    return () => window.clearInterval(timer);
  }, [load]);

  const updateFilters = (next: TimelineFilters) => {
    setFilters(next);
    const nextParams = new URLSearchParams();
    if (next.search) nextParams.set("q", next.search);
    if (next.decision) nextParams.set("decision", next.decision);
    if (next.mode) nextParams.set("mode", next.mode);
    if (next.entityType) nextParams.set("entity_type", next.entityType);
    if (next.entityId) nextParams.set("entity_id", next.entityId);
    setParams(nextParams, { replace: true });
  };

  const items = useMemo(
    () =>
      (data?.items ?? []).filter((item) => matchesSearch(item, filters.search)),
    [data?.items, filters.search],
  );
  const summaries = useMemo(() => timelineSummaries(items), [items]);

  return (
    <div className="space-y-5">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <h2 className="flex items-center gap-2 text-2xl font-bold tracking-tight">
            <ShieldCheck className="h-6 w-6 text-primary" />
            Activity Timeline
          </h2>
          <p className="text-sm text-muted-foreground">
            Real-time agent, resource, tool, policy, PEP, PDP, trace, cost, and
            token evidence from the local control plane.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => exportJson(items)}
            className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
          >
            <Download className="h-4 w-4" />
            JSON
          </button>
          <button
            type="button"
            onClick={() => exportCsv(items)}
            className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
          >
            <Download className="h-4 w-4" />
            CSV
          </button>
        </div>
      </div>

      <ActivityFilters
        value={filters}
        loading={loading}
        onChange={updateFilters}
        onRefresh={load}
      />

      <RelationshipSummaryCards items={summaries} />

      {error && (
        <div className="rounded-lg border border-red-500/20 bg-red-500/10 p-4 text-sm text-red-600">
          {error.message}
        </div>
      )}

      <div className="space-y-3">
        {loading && !data ? (
          <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
            Loading activity timeline...
          </div>
        ) : items.length === 0 ? (
          <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
            No activity matches this filter yet.
          </div>
        ) : (
          items.map((item) => (
            <ActivityEventCard
              key={item.event_id}
              item={item}
              onInspect={setSelected}
            />
          ))
        )}
      </div>

      <DecisionEvidenceDrawer
        item={selected}
        onClose={() => setSelected(null)}
      />
    </div>
  );
}
