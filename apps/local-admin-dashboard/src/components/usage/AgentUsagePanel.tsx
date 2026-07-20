import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { CircleDollarSign } from "lucide-react";
import { UsageApi, type AiUsageEventPage } from "../../services/api";
import { AgentUsageComparison, type AgentUsageRow } from "./AgentUsageComparison";

type AiUsageEvent = AiUsageEventPage["items"][number];

function eventIsEstimated(event: AiUsageEvent) {
  return Boolean(event.tokens?.estimated || event.cost?.estimated);
}

function aggregate(
  events: AiUsageEvent[],
  agentId: string,
  agentName: string,
  agentType?: string,
): { row: AgentUsageRow | null; currency: string } {
  let currency = "USD";
  const row: AgentUsageRow = {
    agentKey: agentId,
    agentName,
    agentType,
    inputTokens: 0,
    outputTokens: 0,
    cachedTokens: 0,
    totalTokens: 0,
    cost: 0,
    calls: 0,
    exact: 0,
    estimated: 0,
    surfaces: [],
  };
  const surfaces = new Set<string>();

  for (const event of events) {
    row.calls += 1;
    row.inputTokens += event.tokens?.input_tokens ?? 0;
    row.outputTokens += event.tokens?.output_tokens ?? 0;
    row.cachedTokens += event.tokens?.cached_input_tokens ?? 0;
    row.totalTokens += event.tokens?.total_tokens ?? 0;
    row.cost += event.cost?.total_cost ?? 0;
    if (event.cost?.currency) currency = event.cost.currency;
    if (eventIsEstimated(event)) row.estimated += 1;
    else row.exact += 1;
    if (event.surface) surfaces.add(event.surface);
  }
  row.surfaces = Array.from(surfaces).sort();

  return { row: row.calls > 0 ? row : null, currency };
}

/**
 * Per-agent usage view: the same side-by-side Usage Bar as the cost ledger, but
 * scoped to one AI app. Lets the per-agent detail answer "how much did just this
 * app use, and split input vs output" without leaving the agent.
 */
export function AgentUsagePanel({
  agentId,
  agentName,
  agentType,
  fromDaysAgo = 7,
}: {
  agentId: string;
  agentName: string;
  agentType?: string;
  fromDaysAgo?: number;
}) {
  const [events, setEvents] = useState<AiUsageEvent[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    const from = new Date(
      Date.now() - fromDaysAgo * 24 * 60 * 60 * 1000,
    ).toISOString();
    UsageApi.getEvents({ from, agent_id: agentId, limit: 200 })
      .then((page) => {
        if (!cancelled) setEvents(page.items ?? []);
      })
      .catch(() => {
        if (!cancelled) setEvents([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [agentId, fromDaysAgo]);

  const { row, currency } = useMemo(
    () => aggregate(events, agentId, agentName, agentType),
    [events, agentId, agentName, agentType],
  );

  if (loading) {
    return (
      <div className="p-4 text-sm text-muted-foreground">Loading usage…</div>
    );
  }

  if (!row) {
    return (
      <div className="flex flex-col items-center justify-center rounded-lg border border-dashed p-8 text-center text-muted-foreground">
        <CircleDollarSign className="mb-2 h-8 w-8 opacity-50" />
        <p className="text-sm font-medium">No usage recorded for this AI app</p>
        <p className="mt-1 max-w-md text-xs leading-5">
          Exact tokens usually need provider telemetry, a wrapper/proxy, local
          logs, or a plugin connector. Browser-only apps may only produce
          estimates.
        </p>
        <Link
          to="/cost-ledger"
          className="mt-4 inline-flex h-9 items-center rounded-md border bg-background px-3 text-sm hover:bg-muted"
        >
          Open AI Usage &amp; Cost
        </Link>
      </div>
    );
  }

  return (
    <AgentUsageComparison
      rows={[row]}
      currency={currency}
      title="This AI app's usage"
      description="Input vs output tokens and cost for this AI app over the last few days. Switch the metric to see the split you care about."
      headerEnd={
        <Link
          to="/cost-ledger"
          className="inline-flex h-9 items-center rounded-md border bg-background px-3 text-sm hover:bg-muted"
        >
          Full ledger
        </Link>
      }
    />
  );
}
