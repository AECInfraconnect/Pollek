import { CheckCircle2, Circle, Eye, ShieldCheck } from "lucide-react";

export function RelationshipEdgeLegend() {
  return (
    <div className="flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
      <span className="inline-flex items-center gap-1">
        <Circle className="h-3 w-3 text-border" />
        Registered
      </span>
      <span className="inline-flex items-center gap-1">
        <Eye className="h-3 w-3 text-blue-500" />
        Observed
      </span>
      <span className="inline-flex items-center gap-1">
        <ShieldCheck className="h-3 w-3 text-emerald-500" />
        Enforced
      </span>
      <span className="inline-flex items-center gap-1">
        <CheckCircle2 className="h-3 w-3 text-primary" />
        Policy match
      </span>
    </div>
  );
}
