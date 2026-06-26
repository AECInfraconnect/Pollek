import { X } from "lucide-react";
import type { ActivityTimelineItem } from "../entity-graph/types";
import {
  formatMoney,
  formatNumber,
  labelForMode,
} from "../entity-graph/graphUtils";

function Field({
  label,
  value,
}: {
  label: string;
  value?: string | number | null;
}) {
  return (
    <div className="rounded-md border bg-background/70 p-3">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="mt-1 break-words text-sm">{value || "-"}</dd>
    </div>
  );
}

export function DecisionEvidenceDrawer({
  item,
  onClose,
}: {
  item: ActivityTimelineItem | null;
  onClose: () => void;
}) {
  if (!item) return null;

  return (
    <div className="fixed inset-0 z-50 flex justify-end bg-background/55 backdrop-blur-sm">
      <aside className="h-full w-full max-w-2xl overflow-y-auto border-l bg-background shadow-xl">
        <div className="sticky top-0 z-10 flex items-start justify-between gap-3 border-b bg-background/95 p-5 backdrop-blur">
          <div className="min-w-0">
            <h2 className="text-lg font-semibold">Decision Evidence</h2>
            <p className="mt-1 break-all font-mono text-xs text-muted-foreground">
              {item.event_id}
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-md p-2 text-muted-foreground hover:bg-muted hover:text-foreground"
            aria-label="Close evidence drawer"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        <div className="space-y-5 p-5">
          <section className="space-y-3">
            <h3 className="text-sm font-semibold">Summary</h3>
            <dl className="grid gap-3 md:grid-cols-2">
              <Field label="Decision" value={item.decision} />
              <Field label="Mode" value={labelForMode(item.enforcement_mode)} />
              <Field label="Action" value={item.action} />
              <Field
                label="Observed at"
                value={new Date(item.timestamp).toLocaleString()}
              />
              <Field label="Actor" value={item.actor?.label} />
              <Field label="Trace ID" value={item.trace_id} />
              <Field label="PEP plane" value={item.pep_plane} />
              <Field label="PDP engine" value={item.pdp_engine || "local"} />
            </dl>
          </section>

          <section className="space-y-3">
            <h3 className="text-sm font-semibold">Touched Entities</h3>
            <dl className="grid gap-3 md:grid-cols-2">
              <Field label="Tool" value={item.tool?.label} />
              <Field label="Resource" value={item.resource?.label} />
              <Field
                label="Policies"
                value={
                  item.policies.length
                    ? item.policies.map((policy) => policy.label).join(", ")
                    : "No policy matched"
                }
              />
              <Field label="Explanation" value={item.explanation} />
            </dl>
          </section>

          <section className="space-y-3">
            <h3 className="text-sm font-semibold">Cost And Tokens</h3>
            <dl className="grid gap-3 md:grid-cols-2">
              <Field label="Provider" value={item.cost?.provider} />
              <Field label="Model" value={item.cost?.model} />
              <Field
                label="Tokens"
                value={
                  item.cost?.total_tokens
                    ? formatNumber(item.cost.total_tokens)
                    : undefined
                }
              />
              <Field
                label="Cost"
                value={
                  item.cost?.total_cost_usd
                    ? formatMoney(item.cost.total_cost_usd)
                    : undefined
                }
              />
            </dl>
          </section>

          <section className="space-y-3">
            <h3 className="text-sm font-semibold">Raw Event</h3>
            <pre className="max-h-[520px] overflow-auto rounded-lg border bg-muted/40 p-4 text-xs">
              {JSON.stringify(item.raw ?? item, null, 2)}
            </pre>
          </section>
        </div>
      </aside>
    </div>
  );
}
