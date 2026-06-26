import { CircleDollarSign, Zap } from "lucide-react";
import type { ActivityTimelineItem } from "../entity-graph/types";
import { formatMoney, formatNumber } from "../entity-graph/graphUtils";

export function CostTokenPanel({ items }: { items: ActivityTimelineItem[] }) {
  const totalTokens = items.reduce(
    (sum, item) => sum + (item.cost?.total_tokens || 0),
    0,
  );
  const totalCost = items.reduce(
    (sum, item) => sum + (item.cost?.total_cost_usd || 0),
    0,
  );
  const providers = new Map<string, { tokens: number; cost: number }>();
  for (const item of items) {
    const provider = item.cost?.provider || "unknown";
    const row = providers.get(provider) ?? { tokens: 0, cost: 0 };
    row.tokens += item.cost?.total_tokens || 0;
    row.cost += item.cost?.total_cost_usd || 0;
    providers.set(provider, row);
  }

  return (
    <div className="space-y-4">
      <div className="grid gap-3 md:grid-cols-2">
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="flex items-center justify-between text-sm text-muted-foreground">
            <span>Tokens</span>
            <Zap className="h-4 w-4" />
          </div>
          <div className="mt-2 text-2xl font-semibold tabular-nums">
            {formatNumber(totalTokens)}
          </div>
        </div>
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="flex items-center justify-between text-sm text-muted-foreground">
            <span>Cost</span>
            <CircleDollarSign className="h-4 w-4" />
          </div>
          <div className="mt-2 text-2xl font-semibold tabular-nums">
            {formatMoney(totalCost)}
          </div>
        </div>
      </div>
      <div className="space-y-2">
        {[...providers.entries()].map(([provider, row]) => (
          <div
            key={provider}
            className="flex items-center justify-between rounded-lg border p-3 text-sm"
          >
            <span className="font-medium">{provider}</span>
            <span className="text-muted-foreground">
              {formatNumber(row.tokens)} tokens - {formatMoney(row.cost)}
            </span>
          </div>
        ))}
        {providers.size === 0 && (
          <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
            No cost or token evidence for this entity yet.
          </div>
        )}
      </div>
    </div>
  );
}
