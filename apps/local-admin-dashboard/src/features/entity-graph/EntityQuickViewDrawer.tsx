import { X } from "lucide-react";
import { useNavigate } from "react-router-dom";
import type { GraphNode } from "./types";
import { entityIcon, entityRoute } from "./graphUtils";

export function EntityQuickViewDrawer({
  node,
  onClose,
}: {
  node: GraphNode | null;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  if (!node) return null;
  const Icon = entityIcon(node.type);

  return (
    <aside
      className="absolute bottom-0 right-0 top-0 z-10 w-full max-w-md border-l bg-card/95 p-5 shadow-xl backdrop-blur"
      aria-label="Entity quick view"
    >
      <div className="flex items-start justify-between gap-4">
        <div className="flex min-w-0 items-start gap-3">
          <div className="rounded-lg border bg-background p-2">
            <Icon className="h-5 w-5 text-primary" />
          </div>
          <div className="min-w-0">
            <h3 className="break-words text-lg font-semibold">{node.label}</h3>
            <p className="mt-1 text-sm text-muted-foreground">
              {node.type} - {node.status}
            </p>
          </div>
        </div>
        <button
          type="button"
          onClick={onClose}
          className="rounded-md p-2 text-muted-foreground hover:bg-muted"
          aria-label="Close quick view"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      {node.subtitle && <p className="mt-4 text-sm text-muted-foreground">{node.subtitle}</p>}

      <div className="mt-5 grid gap-3 sm:grid-cols-2">
        {node.metrics.map((metric) => (
          <div key={metric.label} className="rounded-lg border p-3">
            <div className="text-xs text-muted-foreground">{metric.label}</div>
            <div className="mt-1 text-lg font-semibold">{metric.value}</div>
          </div>
        ))}
      </div>

      <div className="mt-5 flex flex-wrap gap-2">
        {node.badges.map((badge) => (
          <span
            key={badge}
            className="rounded-full border bg-background px-2.5 py-1 text-xs text-muted-foreground"
          >
            {badge}
          </span>
        ))}
      </div>

      <button
        type="button"
        onClick={() => navigate(entityRoute(node))}
        className="mt-6 w-full rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
      >
        Open Entity 360
      </button>
    </aside>
  );
}
