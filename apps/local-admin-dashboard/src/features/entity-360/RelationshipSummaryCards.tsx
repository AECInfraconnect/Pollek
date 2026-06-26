import { cn } from "@/lib/utils";
import type { RelationshipSummary } from "../entity-graph/types";

const toneClasses: Record<string, string> = {
  neutral: "border-border bg-card",
  info: "border-blue-500/20 bg-blue-500/10",
  success: "border-emerald-500/20 bg-emerald-500/10",
  warning: "border-amber-500/20 bg-amber-500/10",
  danger: "border-red-500/20 bg-red-500/10",
};

export function RelationshipSummaryCards({
  items,
}: {
  items: RelationshipSummary[];
}) {
  if (!items.length) return null;
  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
      {items.slice(0, 8).map((item) => (
        <div
          key={item.kind}
          className={cn(
            "rounded-lg border p-4",
            toneClasses[item.tone] ?? toneClasses.neutral,
          )}
        >
          <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            {item.label}
          </div>
          <div className="mt-2 text-2xl font-semibold tabular-nums">
            {item.count}
          </div>
        </div>
      ))}
    </div>
  );
}
