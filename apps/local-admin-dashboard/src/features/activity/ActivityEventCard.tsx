import { Link } from "react-router-dom";
import type { ReactNode } from "react";
import {
  Clock,
  Eye,
  FileKey,
  ShieldAlert,
  ShieldCheck,
  ShieldX,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { ActivityTimelineItem, GraphRef } from "../entity-graph/types";
import {
  formatMoney,
  formatNumber,
  labelForMode,
  toneForStatus,
} from "../entity-graph/graphUtils";

function routeForRef(ref: GraphRef) {
  const selected = encodeURIComponent(ref.entity_id);
  if (ref.type === "agent") return `/agents?selected=${selected}`;
  if (ref.type === "tool") return `/tools?selected=${selected}`;
  if (ref.type === "resource") return `/resources?selected=${selected}`;
  if (ref.type === "policy") return `/policies?selected=${selected}`;
  if (ref.type === "identity") return `/identities?selected=${selected}`;
  return `/entity-graph?selected=${selected}`;
}

function DecisionIcon({ decision }: { decision: string }) {
  if (decision === "deny" || decision === "error") {
    return <ShieldX className="h-4 w-4 text-red-500" />;
  }
  if (decision === "allow" || decision === "ok") {
    return <ShieldCheck className="h-4 w-4 text-emerald-500" />;
  }
  return <ShieldAlert className="h-4 w-4 text-amber-500" />;
}

function Chip({
  children,
  tone = "neutral",
}: {
  children: ReactNode;
  tone?: string;
}) {
  const classes: Record<string, string> = {
    neutral: "border-border bg-background text-muted-foreground",
    success: "border-emerald-500/25 bg-emerald-500/10 text-emerald-600",
    warning: "border-amber-500/25 bg-amber-500/10 text-amber-600",
    danger: "border-red-500/25 bg-red-500/10 text-red-600",
    info: "border-blue-500/25 bg-blue-500/10 text-blue-600",
  };
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs",
        classes[tone] ?? classes.neutral,
      )}
    >
      {children}
    </span>
  );
}

function EntityLink({ refItem }: { refItem: GraphRef }) {
  return (
    <Link
      to={routeForRef(refItem)}
      className="rounded-md text-primary underline-offset-4 hover:underline"
    >
      {refItem.label}
    </Link>
  );
}

export function ActivityEventCard({
  item,
  onInspect,
}: {
  item: ActivityTimelineItem;
  onInspect: (item: ActivityTimelineItem) => void;
}) {
  const tone = toneForStatus(item.decision);
  const actor = item.actor;

  return (
    <article className="rounded-lg border bg-card/60 p-4">
      <div className="flex flex-col gap-3 xl:flex-row xl:items-start xl:justify-between">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <DecisionIcon decision={item.decision} />
            <span className="font-medium">
              {actor ? <EntityLink refItem={actor} /> : "Unknown actor"}
              <span className="ml-1 text-muted-foreground">{item.action}</span>
            </span>
            <Chip tone={tone}>{item.decision}</Chip>
            <Chip
              tone={item.enforcement_mode === "enforce" ? "success" : "info"}
            >
              {labelForMode(item.enforcement_mode)}
            </Chip>
          </div>

          <div className="mt-3 grid gap-2 text-sm md:grid-cols-2 xl:grid-cols-4">
            <div className="rounded-md border bg-background/60 p-3">
              <div className="text-xs text-muted-foreground">Tool</div>
              <div className="mt-1 truncate">
                {item.tool ? <EntityLink refItem={item.tool} /> : "No tool"}
              </div>
            </div>
            <div className="rounded-md border bg-background/60 p-3">
              <div className="text-xs text-muted-foreground">Resource</div>
              <div className="mt-1 truncate">
                {item.resource ? (
                  <EntityLink refItem={item.resource} />
                ) : (
                  "No resource"
                )}
              </div>
            </div>
            <div className="rounded-md border bg-background/60 p-3">
              <div className="text-xs text-muted-foreground">PEP / PDP</div>
              <div className="mt-1 truncate">
                {item.pep_plane || "unknown"} / {item.pdp_engine || "local"}
              </div>
            </div>
            <div className="rounded-md border bg-background/60 p-3">
              <div className="text-xs text-muted-foreground">Cost</div>
              <div className="mt-1 truncate">
                {item.cost?.total_tokens
                  ? `${formatNumber(item.cost.total_tokens)} tokens`
                  : "No token cost"}
                {item.cost?.total_cost_usd
                  ? `, ${formatMoney(item.cost.total_cost_usd)}`
                  : ""}
              </div>
            </div>
          </div>

          <div className="mt-3 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <span className="inline-flex items-center gap-1">
              <Clock className="h-3.5 w-3.5" />
              {new Date(item.timestamp).toLocaleString()}
            </span>
            {item.trace_id && (
              <span className="font-mono">trace {item.trace_id}</span>
            )}
            {item.explanation && <span>{item.explanation}</span>}
          </div>

          {item.policies.length > 0 && (
            <div className="mt-3 flex flex-wrap gap-2">
              {item.policies.map((policy) => (
                <Link
                  key={policy.id}
                  to={routeForRef(policy)}
                  className="inline-flex items-center gap-1 rounded-full border bg-primary/10 px-2 py-1 text-xs text-primary hover:bg-primary/15"
                >
                  <FileKey className="h-3 w-3" />
                  {policy.label}
                </Link>
              ))}
            </div>
          )}
        </div>

        <button
          type="button"
          onClick={() => onInspect(item)}
          className="inline-flex h-9 items-center justify-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
        >
          <Eye className="h-4 w-4" />
          Evidence
        </button>
      </div>
    </article>
  );
}
