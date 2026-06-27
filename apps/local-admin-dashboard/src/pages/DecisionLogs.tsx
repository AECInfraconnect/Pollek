import { useConfirm } from "../components/ui/ConfirmDialog";
import { toast } from "sonner";
import { useState, useEffect, useCallback } from "react";
import {
  Activity,
  RefreshCw,
  ShieldCheck,
  ShieldX,
  ShieldAlert,
  Download,
} from "lucide-react";
import {
  TelemetryApi,
  ActivityApi,
  type TelemetryEventEnvelope,
  type DecisionResult,
  type DecisionEffect,
} from "../services/api";

const EFFECT_STYLE: Record<
  DecisionEffect,
  { cls: string; Icon: typeof ShieldCheck }
> = {
  allow: { cls: "text-emerald-400", Icon: ShieldCheck },
  deny: { cls: "text-red-400", Icon: ShieldX },
  redact: { cls: "text-amber-400", Icon: ShieldAlert },
  mask: { cls: "text-amber-400", Icon: ShieldAlert },
  warn: { cls: "text-amber-400", Icon: ShieldAlert },
  require_approval: { cls: "text-blue-400", Icon: ShieldAlert },
  break_glass_allow: { cls: "text-pink-400", Icon: ShieldAlert },
};

export function DecisionLogs() {
  const { confirm } = useConfirm();

  const [events, setEvents] = useState<TelemetryEventEnvelope[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<"all" | "allow" | "deny">("all");

  const load = useCallback(() => {
    setLoading(true);
    TelemetryApi.listDecisionLogs()
      .then((data: TelemetryEventEnvelope[]) => {
        if (data && data.length > 0) {
          setEvents(data);
        } else {
          ActivityApi.getActivity()
            .then((res: any) => {
              const mockEvents = (res.activity_sets || [])
                .flatMap((set: any) => set.items)
                .map((item: any) => ({
                  timestamp: item.timestamp,
                  event_id: `mock_${Math.random().toString(36).substr(2, 9)}`,
                  payload: {
                    decision: item.decision,
                    reason: item.reason,
                    request_id: "req_mock",
                    matched_policy_ids: [],
                    latency_ms: 10,
                    resource: item.resource,
                  },
                }));
              setEvents(mockEvents);
            })
            .catch(console.error);
        }
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    load();
    const t = setInterval(load, 5000); // live-ish refresh
    return () => clearInterval(t);
  }, [load]);

  const decisions = events
    .map((e) => ({ env: e, d: e.payload as DecisionResult }))
    .filter(({ d }) => filter === "all" || d.decision === filter);

  const allowCount = events.filter(
    (e) => (e.payload as DecisionResult)?.decision === "allow",
  ).length;
  const denyCount = events.filter(
    (e) => (e.payload as DecisionResult)?.decision === "deny",
  ).length;

  const exportJSON = () => {
    const dataStr = JSON.stringify(
      decisions.map((d) => d.env),
      null,
      2,
    );
    const blob = new Blob([dataStr], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "decision_logs.json";
    a.click();
    URL.revokeObjectURL(url);
  };

  const clearHistory = async () => {
    if (
      !(await confirm({
        title: "Confirm",
        description: "Are you sure you want to drop all logs?",
        danger: true,
      }))
    )
      return;
    try {
      await TelemetryApi.clearDecisionLogs();
      load();
    } catch (e) {
      console.error("Failed to clear decision logs:", e);
      toast.error("Failed to clear decision logs");
    }
  };

  const exportCSV = () => {
    const header = [
      "Timestamp",
      "Event ID",
      "Request ID",
      "Decision",
      "Reason",
      "Matched Policies",
      "Latency (ms)",
    ];
    const rows = decisions.map(({ env, d }) => [
      new Date(env.timestamp).toISOString(),
      env.event_id,
      d?.request_id ?? "",
      d?.decision ?? "",
      d?.reason?.replace(/"/g, '""') ?? "",
      d?.matched_policy_ids?.join(";") ?? "",
      String(d?.latency_ms ?? 0),
    ]);

    const csvContent = [header, ...rows]
      .map((e) => e.map((field) => `"${field}"`).join(","))
      .join("\n");

    const blob = new Blob([csvContent], { type: "text/csv;charset=utf-8;" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "decision_logs.csv";
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold tracking-tight flex items-center gap-2">
            <Activity className="h-5 w-5 text-primary" /> Audit & Decision
            Activity
          </h2>
          <p className="text-muted-foreground">
            Every authorization decision the Local Enforcement Kit enforced
            (local workspace).
          </p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={clearHistory}
            className="bg-red-500/10 text-red-400 hover:bg-red-500/20 px-3 py-1.5 rounded-lg text-sm transition-colors border border-red-500/20"
          >
            Clear History
          </button>
          <button
            onClick={exportJSON}
            className="flex items-center gap-2 bg-slate-800 hover:bg-slate-700 px-3 py-1.5 rounded-lg text-sm transition-colors border border-slate-700"
          >
            <Download className="w-4 h-4" /> JSON
          </button>
          <button
            onClick={exportCSV}
            className="flex items-center gap-2 bg-slate-800 hover:bg-slate-700 px-3 py-1.5 rounded-lg text-sm transition-colors border border-slate-700"
          >
            <Download className="w-4 h-4" /> CSV
          </button>
          <button
            onClick={load}
            className="p-2 hover:bg-slate-800 rounded-lg text-slate-400 hover:text-slate-200 transition-colors"
            title="Refresh"
          >
            <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin" : ""}`} />
          </button>
        </div>
      </div>

      {/* Compact inline decision summary */}
      <div className="flex items-center gap-3 text-sm">
        <span className="inline-flex items-center gap-1.5 rounded-md border border-border/60 bg-card/50 px-2.5 py-1">
          <span className="font-semibold tabular-nums">{events.length}</span>
          <span className="text-xs text-muted-foreground">total</span>
        </span>
        <span className="inline-flex items-center gap-1.5 rounded-md border border-emerald-500/30 bg-emerald-500/10 px-2.5 py-1">
          <span className="font-semibold tabular-nums text-emerald-400">{allowCount}</span>
          <span className="text-xs text-emerald-400/80">allowed</span>
        </span>
        <span className="inline-flex items-center gap-1.5 rounded-md border border-red-500/30 bg-red-500/10 px-2.5 py-1">
          <span className="font-semibold tabular-nums text-red-400">{denyCount}</span>
          <span className="text-xs text-red-400/80">denied</span>
        </span>
      </div>

      <div className="flex gap-2">
        {(["all", "allow", "deny"] as const).map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={`rounded-md px-3 py-1.5 text-xs font-medium border transition-colors ${filter === f ? "bg-primary text-primary-foreground" : "hover:bg-muted/50"}`}
          >
            {f}
          </button>
        ))}
      </div>

      <div className="glass rounded-xl overflow-hidden border">
        <table className="w-full text-sm text-left">
          <thead className="bg-muted/50 text-muted-foreground">
            <tr>
              <th className="px-6 py-4 font-medium">Time</th>
              <th className="px-6 py-4 font-medium">Decision</th>
              <th className="px-6 py-4 font-medium">Request</th>
              <th className="px-6 py-4 font-medium">Reason</th>
              <th className="px-6 py-4 font-medium">Policies</th>
              <th className="px-6 py-4 font-medium text-right">Latency</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {loading && events.length === 0 ? (
              <tr>
                <td
                  colSpan={6}
                  className="px-6 py-8 text-center text-muted-foreground"
                >
                  Loading decision logs...
                </td>
              </tr>
            ) : decisions.length === 0 ? (
              <tr>
                <td
                  colSpan={6}
                  className="px-6 py-8 text-center text-muted-foreground"
                >
                  No decisions recorded yet.
                </td>
              </tr>
            ) : (
              decisions.map(({ env, d }) => {
                const style = EFFECT_STYLE[d?.decision] ?? EFFECT_STYLE.deny;
                const Icon = style.Icon;
                return (
                  <tr
                    key={env.event_id}
                    className="hover:bg-muted/30 transition-colors"
                  >
                    <td className="px-6 py-4 text-muted-foreground whitespace-nowrap">
                      {new Date(env.timestamp).toLocaleTimeString()}
                    </td>
                    <td className="px-6 py-4">
                      <span
                        className={`inline-flex items-center gap-1.5 font-medium ${style.cls}`}
                      >
                        <Icon className="h-4 w-4" /> {d?.decision ?? "unknown"}
                      </span>
                    </td>
                    <td className="px-6 py-4 font-mono text-xs">
                      {d?.request_id ?? "-"}
                    </td>
                    <td
                      className="px-6 py-4 text-muted-foreground max-w-xs truncate"
                      title={d?.reason}
                    >
                      {d?.reason ?? "-"}
                    </td>
                    <td className="px-6 py-4 text-xs text-muted-foreground">
                      {d?.matched_policy_ids?.length
                        ? d.matched_policy_ids.join(", ")
                        : "—"}
                    </td>
                    <td className="px-6 py-4 text-right text-muted-foreground">
                      {d?.latency_ms ?? 0}ms
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

// StatCard removed - replaced by compact inline badges above
