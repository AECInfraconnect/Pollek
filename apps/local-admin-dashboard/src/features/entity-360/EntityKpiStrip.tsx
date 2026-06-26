import type { ActivityTimelineItem, Entity360Response } from "../entity-graph/types";
import { formatMoney, formatNumber } from "../entity-graph/graphUtils";

function decisionCount(items: ActivityTimelineItem[], decision: string) {
  return items.filter((item) => item.decision === decision).length;
}

export function EntityKpiStrip({ data }: { data: Entity360Response }) {
  const relatedCount = Math.max(data.graph.nodes.length - 1, 0);
  const totalTokens = data.activity.reduce(
    (sum, item) => sum + (item.cost?.total_tokens || 0),
    0,
  );
  const totalCost = data.activity.reduce(
    (sum, item) => sum + (item.cost?.total_cost_usd || 0),
    0,
  );
  const policies = data.graph.nodes.filter((node) => node.type === "policy").length;

  const items = [
    { label: "Related entities", value: formatNumber(relatedCount) },
    { label: "Policies", value: formatNumber(policies) },
    { label: "Activity", value: formatNumber(data.activity.length) },
    { label: "Denied", value: formatNumber(decisionCount(data.activity, "deny")) },
    { label: "Tokens", value: formatNumber(totalTokens) },
    { label: "Cost", value: formatMoney(totalCost) },
  ];

  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-6">
      {items.map((item) => (
        <div key={item.label} className="rounded-lg border bg-card/60 p-4">
          <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            {item.label}
          </div>
          <div className="mt-2 truncate text-xl font-semibold tabular-nums">
            {item.value}
          </div>
        </div>
      ))}
    </div>
  );
}
