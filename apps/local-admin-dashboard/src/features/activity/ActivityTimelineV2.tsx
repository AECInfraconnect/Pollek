import { useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import {
  Bot,
  Database,
  Download,
  FileKey,
  FolderOpen,
  Network,
  ShieldAlert,
  ShieldCheck,
  ShieldX,
  Wrench,
} from "lucide-react";
import { Link, useSearchParams } from "react-router-dom";
import { EntityGraphApi } from "../../services/entityGraphApi";
import { RelationshipSummaryCards } from "../entity-360/RelationshipSummaryCards";
import { ContextualHelp } from "../../components/help/ContextualHelp";
import type {
  ActivityTimelineItem,
  ActivityTimelineResponse,
  GraphRef,
  RelationshipSummary,
} from "../entity-graph/types";
import { ActivityFilters, type TimelineFilters } from "./ActivityFilters";
import { MasterDetailLayout } from "../../components/master-detail/MasterDetailLayout";
import { EntityCard } from "../../components/master-detail/EntityCard";
import { DetailPane } from "../../components/master-detail/DetailPane";
import type { UiStatus } from "../../lib/status";
import {
  formatMoney,
  formatNumber,
  labelForMode,
  toneForStatus,
} from "../entity-graph/graphUtils";
import { useMode } from "../../context/ModeContext";
import { isAdvanceMode } from "../../lib/modes";

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

function refRoute(ref?: GraphRef | null) {
  if (!ref) return "";
  const selected = encodeURIComponent(ref.entity_id);
  if (ref.type === "agent") return `/agents?id=${selected}`;
  if (ref.type === "tool") return `/tools?id=${selected}`;
  if (ref.type === "resource") return `/resources?id=${selected}`;
  if (ref.type === "policy") return `/policies?id=${selected}`;
  if (ref.type === "identity") return `/identities?id=${selected}`;
  return `/entity-graph?selected=${selected}`;
}

function RefLink({ item }: { item?: GraphRef | null }) {
  if (!item) return <span className="text-muted-foreground">Not linked</span>;
  return (
    <Link
      to={refRoute(item)}
      className="break-words text-primary underline-offset-4 hover:underline"
    >
      {item.label}
    </Link>
  );
}

function statusForDecision(decision: string): UiStatus {
  const normalized = decision.toLowerCase();
  if (["deny", "denied", "blocked", "error"].includes(normalized)) {
    return "failed";
  }
  if (["allow", "allowed", "ok", "redact", "redacted"].includes(normalized)) {
    return "ok";
  }
  if (["warn", "warning", "ask"].includes(normalized)) return "degraded";
  return "info";
}

function friendlyDecisionLabel(decision: string) {
  const normalized = decision.toLowerCase();
  if (normalized === "allow" || normalized === "ok") return "Allowed";
  if (normalized === "deny" || normalized === "blocked") return "Blocked";
  if (normalized === "redact" || normalized === "redacted") return "Redacted";
  if (normalized === "warn" || normalized === "warning") return "Warned";
  if (normalized === "ask") return "Ask first";
  if (normalized === "error") return "Needs review";
  return decision.replace(/_/g, " ");
}

function friendlyAction(item: ActivityTimelineItem) {
  const action = item.action.replace(/[_.:-]+/g, " ");
  const actor = item.actor?.label ?? "Unknown AI app";
  const resource = item.resource?.label;
  const tool = item.tool?.label;
  if (resource && tool) return `${actor} used ${tool} on ${resource}`;
  if (resource) return `${actor} touched ${resource}`;
  if (tool) return `${actor} used ${tool}`;
  return `${actor} ${action}`;
}

function friendlyDecisionExplanation(item: ActivityTimelineItem) {
  const decision = item.decision.toLowerCase();
  if (decision === "allow" || decision === "ok") {
    return "Pollek recorded this activity and the action was allowed.";
  }
  if (decision === "deny" || decision === "blocked") {
    return "Pollek recorded this activity and the action was blocked.";
  }
  if (decision === "redact" || decision === "redacted") {
    return "Pollek masked or removed sensitive content before it continued.";
  }
  if (decision === "warn" || decision === "warning") {
    return "Pollek let the action continue and raised a warning for review.";
  }
  return "Pollek recorded this activity for review.";
}

function eventKind(item: ActivityTimelineItem) {
  if (item.resource?.type === "resource") return "Data or file";
  if (item.resource?.label?.toLowerCase().includes("prompt")) {
    return "Prompt safety";
  }
  if (item.tool) return "Tool use";
  if (item.cost?.total_tokens || item.cost?.total_cost_usd) return "AI usage";
  return "Activity";
}

function technicalRouteLabel(item: ActivityTimelineItem) {
  const parts = [item.pep_plane, item.pdp_engine].filter(Boolean);
  return parts.length ? parts.join(" / ") : "Local path";
}

function TimelineField({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="rounded-lg border bg-background/60 p-3">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-1 min-w-0 text-sm font-medium">{children}</div>
    </div>
  );
}

function RelatedRecordCard({
  title,
  icon: Icon,
  children,
}: {
  title: string;
  icon: typeof Bot;
  children: ReactNode;
}) {
  return (
    <section className="rounded-lg border bg-background/50 p-4">
      <h3 className="flex items-center gap-2 text-sm font-semibold">
        <Icon className="h-4 w-4 text-primary" />
        {title}
      </h3>
      <div className="mt-3 space-y-2 text-sm">{children}</div>
    </section>
  );
}

function TimelineDetail({
  item,
  showTechnicalDetails,
}: {
  item: ActivityTimelineItem;
  showTechnicalDetails: boolean;
}) {
  const decisionStatus = statusForDecision(item.decision);
  const costText = item.cost?.total_tokens
    ? `${formatNumber(item.cost.total_tokens)} tokens${
        item.cost.total_cost_usd ? `, ${formatMoney(item.cost.total_cost_usd)}` : ""
      }`
    : "No token or cost data on this event";

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 border-b pb-4 xl:flex-row xl:items-start xl:justify-between">
        <div className="min-w-0">
          <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Activity event
          </div>
          <h2 className="mt-1 break-words text-xl font-semibold">
            {friendlyAction(item)}
          </h2>
          <p className="mt-1 text-sm leading-6 text-muted-foreground">
            {friendlyDecisionExplanation(item)}
          </p>
        </div>
        <span className="inline-flex h-8 items-center rounded-full border bg-background px-3 text-sm font-medium">
          {friendlyDecisionLabel(item.decision)}
        </span>
      </div>

      <div className="grid gap-4 lg:grid-cols-[280px_minmax(0,1fr)] 2xl:grid-cols-[280px_minmax(0,1fr)_300px]">
        <aside className="space-y-3">
          <section className="rounded-lg border bg-card/50 p-4">
            <h3 className="mb-3 flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              <span className="h-1.5 w-1.5 rounded-full bg-primary" />
              Event summary
            </h3>
            <div className="space-y-2 text-sm">
              <TimelineField label="AI app">
                <RefLink item={item.actor} />
              </TimelineField>
              <TimelineField label="What happened">
                {item.action.replace(/[_.:-]+/g, " ")}
              </TimelineField>
              <TimelineField label="Touched or used">
                {item.resource?.label || item.tool?.label || "Not linked"}
              </TimelineField>
              <TimelineField label="When">
                {new Date(item.timestamp).toLocaleString()}
              </TimelineField>
            </div>
          </section>
        </aside>

        <section className="min-w-0">
          <DetailPane
            title="Detail Workspace"
            subtitle="Plain-language evidence first, technical routing only in advanced details."
            status={decisionStatus}
            statusLabel={friendlyDecisionLabel(item.decision)}
            tabs={[
              {
                id: "overview",
                label: "Overview",
                content: (
                  <div className="space-y-4">
                    <div className="grid gap-3 md:grid-cols-3">
                      <TimelineField label="Result">
                        {friendlyDecisionLabel(item.decision)}
                      </TimelineField>
                      <TimelineField label="Activity type">
                        {eventKind(item)}
                      </TimelineField>
                      <TimelineField label="Watch mode">
                        {labelForMode(item.enforcement_mode)}
                      </TimelineField>
                    </div>
                    <div className="rounded-lg border bg-background/60 p-4">
                      <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        What this means
                      </div>
                      <p className="mt-2 text-sm leading-6">
                        {friendlyDecisionExplanation(item)}
                      </p>
                      <p className="mt-2 text-sm leading-6 text-muted-foreground">
                        If this is not what you want, create or edit a rule for
                        this AI app, resource, website, command, or tool.
                      </p>
                    </div>
                    <div className="flex flex-wrap gap-2">
                      {item.actor && (
                        <Link
                          to={`/protect?agent=${encodeURIComponent(
                            item.actor.entity_id,
                          )}&event=${encodeURIComponent(item.event_id)}`}
                          className="inline-flex h-9 items-center gap-2 rounded-md bg-primary px-3 text-sm font-medium text-primary-foreground hover:bg-primary/90"
                        >
                          <ShieldCheck className="h-4 w-4" />
                          Create rule from event
                        </Link>
                      )}
                      <Link
                        to="/setup"
                        className="inline-flex h-9 items-center gap-2 rounded-md border bg-background px-3 text-sm hover:bg-muted"
                      >
                        <Wrench className="h-4 w-4" />
                        Check setup
                      </Link>
                    </div>
                  </div>
                ),
              },
              {
                id: "evidence",
                label: "Evidence",
                content: (
                  <div className="space-y-4">
                    <div className="grid gap-3 md:grid-cols-2">
                      <TimelineField label="Tool">
                        <RefLink item={item.tool} />
                      </TimelineField>
                      <TimelineField label="Resource">
                        <RefLink item={item.resource} />
                      </TimelineField>
                      <TimelineField label="Policy">
                        {item.policies.length ? (
                          <div className="space-y-1">
                            {item.policies.map((policy) => (
                              <div key={policy.id}>
                                <RefLink item={policy} />
                              </div>
                            ))}
                          </div>
                        ) : (
                          "No rule matched"
                        )}
                      </TimelineField>
                      <TimelineField label="Cost and tokens">
                        {costText}
                      </TimelineField>
                    </div>
                    <div className="rounded-lg border bg-background/60 p-4">
                      <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Explanation
                      </div>
                      <p className="mt-2 text-sm leading-6 text-muted-foreground">
                        {item.explanation ||
                          "No extra explanation was attached to this event."}
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
                        <div className="space-y-4">
                          <div className="grid gap-3 md:grid-cols-2">
                            <TimelineField label="Route">
                              {technicalRouteLabel(item)}
                            </TimelineField>
                            <TimelineField label="Trace">
                              <span className="break-all font-mono text-xs">
                                {item.trace_id || "Not linked"}
                              </span>
                            </TimelineField>
                            <TimelineField label="Event ID">
                              <span className="break-all font-mono text-xs">
                                {item.event_id}
                              </span>
                            </TimelineField>
                            <TimelineField label="Mode">
                              {item.enforcement_mode}
                            </TimelineField>
                          </div>
                          <pre className="max-h-[520px] overflow-auto rounded-lg border bg-muted/40 p-4 text-xs leading-5">
                            {JSON.stringify(item.raw ?? item, null, 2)}
                          </pre>
                        </div>
                      ),
                    },
                  ]
                : []),
            ]}
          />
        </section>

        <aside className="space-y-3 lg:col-span-2 2xl:col-span-1">
          <RelatedRecordCard title="AI app" icon={Bot}>
            <RefLink item={item.actor} />
          </RelatedRecordCard>
          <RelatedRecordCard title="Tool" icon={Wrench}>
            <RefLink item={item.tool} />
          </RelatedRecordCard>
          <RelatedRecordCard title="Resource" icon={Database}>
            <RefLink item={item.resource} />
          </RelatedRecordCard>
          <RelatedRecordCard title="Rules" icon={FileKey}>
            {item.policies.length ? (
              item.policies.map((policy) => (
                <div key={policy.id}>
                  <RefLink item={policy} />
                </div>
              ))
            ) : (
              <span className="text-muted-foreground">No rule matched</span>
            )}
          </RelatedRecordCard>
        </aside>
      </div>
    </div>
  );
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
  const { mode } = useMode();
  const showTechnicalDetails = isAdvanceMode(mode);
  const [params, setParams] = useSearchParams();
  const [filters, setFilters] = useState<TimelineFilters>(() =>
    initialFilters(params),
  );
  const [data, setData] = useState<ActivityTimelineResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const selectedId = params.get("selected") ?? undefined;

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
    if (selectedId) nextParams.set("selected", selectedId);
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
  const selectEvent = (eventId: string) => {
    const nextParams = new URLSearchParams(params);
    if (eventId) nextParams.set("selected", eventId);
    else nextParams.delete("selected");
    setParams(nextParams, { replace: true });
  };

  return (
    <div className="space-y-5">
      {!selectedId && (
        <>
          <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
            <div>
              <h2 className="flex items-center gap-2 text-2xl font-bold tracking-tight">
                <ShieldCheck className="h-6 w-6 text-primary" />
                Activity Timeline
                <ContextualHelp topicId="activity.timeline" />
              </h2>
              <p className="text-sm text-muted-foreground">
                A detailed event ledger for AI apps, files, websites, tools,
                rules, decisions, cost, and trace evidence.
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
        </>
      )}

      {error && (
        <div className="rounded-lg border border-red-500/20 bg-red-500/10 p-4 text-sm text-red-600">
          {error.message}
        </div>
      )}

      <MasterDetailLayout
        items={items}
        selectedId={selectedId}
        onSelect={selectEvent}
        idSelector={(item) => item.event_id}
        loading={loading && !data}
        detailBackLabel="Back to all timeline events"
        emptyState={
          <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
            No activity matches this filter yet.
          </div>
        }
        renderGroupHeader={(item, index, prevItem) => {
          const day = new Date(item.timestamp).toDateString();
          const prevDay = prevItem
            ? new Date(prevItem.timestamp).toDateString()
            : null;
          if (index > 0 && day === prevDay) return null;
          return (
            <div className="px-1 py-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              {new Date(item.timestamp).toLocaleDateString(undefined, {
                month: "short",
                day: "numeric",
              })}
            </div>
          );
        }}
        renderCard={(item, selected) => {
          const status = statusForDecision(item.decision);
          const decisionTone = toneForStatus(item.decision);
          const Icon =
            item.decision === "deny" || item.decision === "error"
              ? ShieldX
              : item.decision === "warn"
                ? ShieldAlert
                : item.resource
                  ? FolderOpen
                  : item.tool
                    ? Wrench
                    : Network;
          return (
            <EntityCard
              title={friendlyAction(item)}
              subtitle={`${new Date(item.timestamp).toLocaleString()} - ${eventKind(
                item,
              )}`}
              summary={item.explanation || friendlyDecisionExplanation(item)}
              icon={Icon}
              status={status}
              statusLabel={friendlyDecisionLabel(item.decision)}
              meta={[
                { label: "AI app", value: item.actor?.label ?? "Unknown" },
                {
                  label: "Touched",
                  value: item.resource?.label || item.tool?.label || "None",
                },
                ...(item.policies[0]
                  ? [{ label: "Rule", value: item.policies[0].label }]
                  : []),
                { label: "Mode", value: labelForMode(item.enforcement_mode) },
                ...(showTechnicalDetails
                  ? [{ label: "Route", value: technicalRouteLabel(item) }]
                  : []),
              ]}
              selected={selected}
              className={
                decisionTone === "danger" ? "border-red-500/30" : undefined
              }
            />
          );
        }}
        renderDetail={(item) => (
          <TimelineDetail
            key={item.event_id}
            item={item}
            showTechnicalDetails={showTechnicalDetails}
          />
        )}
      />
    </div>
  );
}
