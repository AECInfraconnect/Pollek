import { useNavigate } from "react-router-dom";
import { cn } from "@/lib/utils";
import type { GraphNode } from "../entity-graph/types";
import { entityIcon, entityRoute, toneForStatus } from "../entity-graph/graphUtils";

const toneClasses: Record<string, string> = {
  neutral: "border-border",
  info: "border-blue-500/25",
  success: "border-emerald-500/25",
  warning: "border-amber-500/25",
  danger: "border-red-500/25",
};

export function RelatedEntityCards({
  nodes,
  centerId,
}: {
  nodes: GraphNode[];
  centerId?: string;
}) {
  const navigate = useNavigate();
  const related = nodes.filter((node) => node.id !== centerId);

  if (!related.length) {
    return (
      <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
        No related entities yet. Activity, policy targets, and registry links will appear here.
      </div>
    );
  }

  return (
    <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
      {related.map((node) => {
        const Icon = entityIcon(node.type);
        const tone = toneForStatus(node.status);
        return (
          <button
            key={node.id}
            type="button"
            onClick={() => navigate(entityRoute(node))}
            className={cn(
              "rounded-lg border bg-card/60 p-4 text-left transition hover:border-primary/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary",
              toneClasses[tone] ?? toneClasses.neutral,
            )}
          >
            <div className="flex items-start gap-3">
              <div className="rounded-md border bg-background p-2">
                <Icon className="h-4 w-4 text-primary" />
              </div>
              <div className="min-w-0">
                <div className="truncate font-medium">{node.label}</div>
                <div className="mt-1 truncate text-xs text-muted-foreground">
                  {node.type} - {node.status}
                </div>
              </div>
            </div>
            {node.subtitle && (
              <p className="mt-3 line-clamp-2 text-sm text-muted-foreground">
                {node.subtitle}
              </p>
            )}
          </button>
        );
      })}
    </div>
  );
}
