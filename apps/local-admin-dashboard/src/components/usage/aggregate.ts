import type { AiUsageEventPage } from "../../services/api";
import type { AgentUsageRow } from "./AgentUsageComparison";

type AiUsageEvent = AiUsageEventPage["items"][number];

function eventIsEstimated(event: AiUsageEvent) {
  return Boolean(event.tokens?.estimated || event.cost?.estimated);
}

function normalizedProvider(provider?: string | null) {
  return (provider || "").trim().toLowerCase();
}

/**
 * Lightweight per-agent usage aggregation shared by the surfaces that only need
 * the friendly rows (Overview, per-agent panels) — not the cost ledger's full
 * provider/model pool breakdown. Credits are derived per event from the
 * observed cost at the provider's configured credit value.
 */
export function aggregateAgentUsageRows(
  events: AiUsageEvent[],
  options: {
    nameFor?: (agentId: string) => { name: string; kind?: string } | undefined;
    creditRates?: Map<string, number>;
  } = {},
): AgentUsageRow[] {
  const { nameFor, creditRates } = options;
  const rows = new Map<string, AgentUsageRow & { surfaceSet: Set<string> }>();

  for (const event of events) {
    const agentKey =
      event.agent_id || event.surface || event.agent_type || "unknown-agent";
    const resolved = nameFor?.(agentKey);
    const existing =
      rows.get(agentKey) ??
      ({
        agentKey,
        agentName: resolved?.name || agentKey,
        agentType: resolved?.kind || event.agent_type || undefined,
        inputTokens: 0,
        outputTokens: 0,
        cachedTokens: 0,
        totalTokens: 0,
        cost: 0,
        credit: 0,
        calls: 0,
        exact: 0,
        estimated: 0,
        surfaces: [],
        surfaceSet: new Set<string>(),
      } as AgentUsageRow & { surfaceSet: Set<string> });

    existing.calls += 1;
    existing.inputTokens += event.tokens?.input_tokens ?? 0;
    existing.outputTokens += event.tokens?.output_tokens ?? 0;
    existing.cachedTokens += event.tokens?.cached_input_tokens ?? 0;
    existing.totalTokens += event.tokens?.total_tokens ?? 0;
    const cost = event.cost?.total_cost ?? 0;
    existing.cost += cost;
    const rate = creditRates?.get(normalizedProvider(event.provider));
    if (rate && rate > 0) {
      existing.credit = (existing.credit ?? 0) + cost / rate;
    }
    if (eventIsEstimated(event)) existing.estimated += 1;
    else existing.exact += 1;
    if (event.surface) existing.surfaceSet.add(event.surface);

    rows.set(agentKey, existing);
  }

  return Array.from(rows.values())
    .map(({ surfaceSet, ...row }) => ({
      ...row,
      surfaces: Array.from(surfaceSet).sort(),
    }))
    .sort(
      (a, b) =>
        b.cost - a.cost ||
        b.totalTokens - a.totalTokens ||
        a.agentName.localeCompare(b.agentName),
    );
}
