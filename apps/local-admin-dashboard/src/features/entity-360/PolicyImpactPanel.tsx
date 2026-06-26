import { FileKey, ShieldAlert, ShieldCheck, ShieldX } from "lucide-react";
import type { Entity360Response } from "../entity-graph/types";
import { RelatedEntityCards } from "./RelatedEntityCards";

export function PolicyImpactPanel({ data }: { data: Entity360Response }) {
  const allow = data.activity.filter((item) => item.decision === "allow").length;
  const deny = data.activity.filter((item) => item.decision === "deny").length;
  const warn = data.activity.filter((item) =>
    ["warn", "warning", "require_approval"].includes(item.decision),
  ).length;

  return (
    <div className="space-y-4">
      <div className="grid gap-3 md:grid-cols-3">
        <ImpactMetric icon={ShieldCheck} label="Allowed" value={allow} />
        <ImpactMetric icon={ShieldX} label="Denied" value={deny} tone="danger" />
        <ImpactMetric icon={ShieldAlert} label="Warned or approval" value={warn} tone="warning" />
      </div>
      <section className="space-y-3">
        <div className="flex items-center gap-2">
          <FileKey className="h-4 w-4 text-primary" />
          <h4 className="font-medium">Affected entities</h4>
        </div>
        <RelatedEntityCards nodes={data.graph.nodes} centerId={data.entity.id} />
      </section>
    </div>
  );
}

function ImpactMetric({
  icon: Icon,
  label,
  value,
  tone = "success",
}: {
  icon: typeof ShieldCheck;
  label: string;
  value: number;
  tone?: "success" | "warning" | "danger";
}) {
  const toneClass =
    tone === "danger"
      ? "text-red-600 bg-red-500/10 border-red-500/20"
      : tone === "warning"
        ? "text-amber-600 bg-amber-500/10 border-amber-500/20"
        : "text-emerald-600 bg-emerald-500/10 border-emerald-500/20";
  return (
    <div className={`rounded-lg border p-4 ${toneClass}`}>
      <div className="flex items-center justify-between text-sm">
        <span>{label}</span>
        <Icon className="h-4 w-4" />
      </div>
      <div className="mt-2 text-2xl font-semibold">{value}</div>
    </div>
  );
}
