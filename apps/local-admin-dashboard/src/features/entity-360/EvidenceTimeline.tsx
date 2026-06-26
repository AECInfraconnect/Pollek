import { useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  Clock,
  FileKey,
  ShieldAlert,
  ShieldCheck,
  ShieldX,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { ActivityTimelineItem } from "../entity-graph/types";
import { formatMoney, formatNumber, labelForMode, toneForStatus } from "../entity-graph/graphUtils";

function DecisionIcon({ decision }: { decision: string }) {
  if (decision === "deny") return <ShieldX className="h-4 w-4 text-red-500" />;
  if (decision === "allow" || decision === "ok") {
    return <ShieldCheck className="h-4 w-4 text-emerald-500" />;
  }
  return <ShieldAlert className="h-4 w-4 text-amber-500" />;
}

export function EvidenceTimeline({
  items,
  compact = false,
}: {
  items: ActivityTimelineItem[];
  compact?: boolean;
}) {
  if (!items.length) {
    return (
      <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
        No activity yet. Run an agent or simulator to generate policy evidence.
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {items.slice(0, compact ? 5 : 30).map((item) => (
        <EvidenceRow key={item.event_id} item={item} />
      ))}
    </div>
  );
}

function EvidenceRow({ item }: { item: ActivityTimelineItem }) {
  const [open, setOpen] = useState(false);
  const tone = toneForStatus(item.decision);

  return (
    <div className="rounded-lg border bg-card/60 p-4">
      <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <DecisionIcon decision={item.decision} />
            <span className="font-medium">
              {item.actor?.label || "Unknown actor"} {item.action}
            </span>
            {item.tool && (
              <span className="rounded-full border bg-background px-2 py-0.5 text-xs">
                Tool: {item.tool.label}
              </span>
            )}
            {item.resource && (
              <span className="rounded-full border bg-background px-2 py-0.5 text-xs">
                Resource: {item.resource.label}
              </span>
            )}
          </div>
          <div className="mt-2 flex flex-wrap gap-2 text-xs text-muted-foreground">
            <span className="inline-flex items-center gap-1">
              <Clock className="h-3.5 w-3.5" />
              {new Date(item.timestamp).toLocaleString()}
            </span>
            <span
              className={cn(
                "rounded-full border px-2 py-0.5",
                tone === "danger"
                  ? "border-red-500/25 bg-red-500/10 text-red-600"
                  : tone === "success"
                    ? "border-emerald-500/25 bg-emerald-500/10 text-emerald-600"
                    : "border-amber-500/25 bg-amber-500/10 text-amber-600",
              )}
            >
              {item.decision}
            </span>
            <span>{labelForMode(item.enforcement_mode)}</span>
            {item.pep_plane && <span>PEP: {item.pep_plane}</span>}
            {item.pdp_engine && <span>PDP: {item.pdp_engine}</span>}
            {item.cost?.total_tokens ? (
              <span>
                {formatNumber(item.cost.total_tokens)} tokens
                {item.cost.total_cost_usd
                  ? ` - ${formatMoney(item.cost.total_cost_usd)}`
                  : ""}
              </span>
            ) : null}
          </div>
          {item.policies.length > 0 && (
            <div className="mt-3 flex flex-wrap gap-2">
              {item.policies.map((policy) => (
                <span
                  key={policy.id}
                  className="inline-flex items-center gap-1 rounded-full border bg-primary/10 px-2 py-1 text-xs text-primary"
                >
                  <FileKey className="h-3 w-3" />
                  {policy.label}
                </span>
              ))}
            </div>
          )}
          {item.explanation && (
            <p className="mt-3 text-sm text-muted-foreground">{item.explanation}</p>
          )}
        </div>
        <button
          type="button"
          onClick={() => setOpen((value) => !value)}
          className="inline-flex items-center gap-1 rounded-md border px-2 py-1 text-xs text-muted-foreground hover:bg-muted"
        >
          {open ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
          Evidence
        </button>
      </div>
      {open && (
        <div className="mt-4 rounded-md border bg-background p-3">
          <dl className="grid gap-2 text-xs md:grid-cols-2">
            <div>
              <dt className="text-muted-foreground">Event ID</dt>
              <dd className="break-all font-mono">{item.event_id}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground">Trace ID</dt>
              <dd className="break-all font-mono">{item.trace_id || "-"}</dd>
            </div>
          </dl>
          <details className="mt-3">
            <summary className="cursor-pointer text-xs text-muted-foreground">
              Raw JSON
            </summary>
            <pre className="mt-2 overflow-x-auto rounded bg-muted/40 p-3 text-[10px]">
              {JSON.stringify(item.raw ?? item, null, 2)}
            </pre>
          </details>
        </div>
      )}
    </div>
  );
}
