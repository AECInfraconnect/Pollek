import type { ReactNode } from "react";
import { ShieldCheck } from "lucide-react";
import { cn } from "@/lib/utils";
import type { GraphNode } from "../entity-graph/types";
import { entityIcon, labelForMode, toneForStatus } from "../entity-graph/graphUtils";

const toneClasses: Record<string, string> = {
  neutral: "border-border bg-muted/40 text-muted-foreground",
  success: "border-emerald-500/25 bg-emerald-500/10 text-emerald-600",
  warning: "border-amber-500/25 bg-amber-500/10 text-amber-600",
  danger: "border-red-500/25 bg-red-500/10 text-red-600",
  info: "border-blue-500/25 bg-blue-500/10 text-blue-600",
};

export function EntityHeader({
  entity,
  actions,
}: {
  entity: GraphNode;
  actions?: ReactNode;
}) {
  const Icon = entityIcon(entity.type);
  const tone = toneForStatus(entity.status);

  return (
    <div className="rounded-lg border bg-card/70 p-5">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
        <div className="flex min-w-0 gap-4">
          <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-lg border bg-background">
            <Icon className="h-6 w-6 text-primary" />
          </div>
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <h2 className="break-words text-2xl font-semibold tracking-tight">
                {entity.label}
              </h2>
              <span className="rounded-full border bg-background px-2 py-1 text-xs text-muted-foreground">
                {entity.type}
              </span>
            </div>
            {entity.subtitle && (
              <p className="mt-1 break-words text-sm text-muted-foreground">
                {entity.subtitle}
              </p>
            )}
            <div className="mt-3 flex flex-wrap gap-2">
              <span
                className={cn(
                  "rounded-full border px-2.5 py-1 text-xs font-medium",
                  toneClasses[tone] ?? toneClasses.neutral,
                )}
              >
                {entity.status || "unknown"}
              </span>
              <span className="inline-flex items-center gap-1 rounded-full border bg-muted/30 px-2.5 py-1 text-xs text-muted-foreground">
                <ShieldCheck className="h-3.5 w-3.5" />
                {labelForMode(entity.mode)}
              </span>
              {entity.risk && (
                <span className="rounded-full border bg-background px-2.5 py-1 text-xs text-muted-foreground">
                  Risk: {entity.risk}
                </span>
              )}
              {entity.badges.slice(0, 4).map((badge) => (
                <span
                  key={badge}
                  className="rounded-full border bg-background px-2.5 py-1 text-xs text-muted-foreground"
                >
                  {badge}
                </span>
              ))}
            </div>
          </div>
        </div>
        {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
      </div>
    </div>
  );
}
