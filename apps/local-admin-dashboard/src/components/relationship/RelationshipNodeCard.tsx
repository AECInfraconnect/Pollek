import { cn } from "@/lib/utils";
import type { GraphNode } from "../../features/entity-graph/types";
import { entityIcon, labelForMode, toneForStatus } from "../../features/entity-graph/graphUtils";

const toneClasses: Record<string, string> = {
  neutral: "border-border bg-muted/40 text-muted-foreground",
  success: "border-emerald-500/25 bg-emerald-500/10 text-emerald-600",
  warning: "border-amber-500/25 bg-amber-500/10 text-amber-600",
  danger: "border-red-500/25 bg-red-500/10 text-red-600",
  info: "border-blue-500/25 bg-blue-500/10 text-blue-600",
};

export function RelationshipNodeCard({
  node,
  selected,
  compact = false,
}: {
  node: GraphNode;
  selected?: boolean;
  compact?: boolean;
}) {
  const Icon = entityIcon(node.type);
  const tone = toneForStatus(node.status);

  return (
    <div
      className={cn(
        "w-full rounded-lg border bg-card/95 p-3 text-left shadow-sm backdrop-blur transition",
        selected
          ? "border-primary shadow-[0_0_0_3px_rgba(59,130,246,0.18)]"
          : "hover:border-primary/60",
      )}
    >
      <div className="flex items-start gap-2">
        <div className="mt-0.5 rounded-md border bg-background p-1.5">
          <Icon className="h-4 w-4 text-primary" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-semibold">{node.label}</div>
          {!compact && node.subtitle && (
            <div className="mt-0.5 truncate text-xs text-muted-foreground">
              {node.subtitle}
            </div>
          )}
        </div>
      </div>
      <div className="mt-3 flex flex-wrap gap-1.5">
        <span
          className={cn(
            "rounded-full border px-2 py-0.5 text-[11px]",
            toneClasses[tone] ?? toneClasses.neutral,
          )}
        >
          {node.status || "unknown"}
        </span>
        {node.mode && (
          <span className="rounded-full border bg-muted/40 px-2 py-0.5 text-[11px] text-muted-foreground">
            {labelForMode(node.mode)}
          </span>
        )}
        {!compact &&
          node.badges.slice(0, 2).map((badge) => (
            <span
              key={badge}
              className="rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground"
            >
              {badge}
            </span>
          ))}
      </div>
    </div>
  );
}
