import { useEffect, useState } from "react";
import { Activity, Eye } from "lucide-react";
import { TelemetryApi } from "../../services/api";
import { TechnicalDetails } from "../ui/TechnicalDetails";

type Observation = {
  action?: string;
  timestamp?: string;
  agent_id?: string;
  target_redacted?: string;
  target?: string;
  tool_id?: string;
  decision?: string;
  outcome?: string;
  details?: unknown;
  [key: string]: unknown;
};

function labelize(value?: string) {
  if (!value) return "";
  return value
    .replace(/[_.:-]+/g, " ")
    .replace(/\b\w/g, (char) => char.toUpperCase())
    .trim();
}

function observationTitle(ev: Observation) {
  const action = labelize(ev.action) || "Observation";
  const target = ev.target_redacted || ev.target || ev.tool_id;
  return target ? `${action} · ${target}` : action;
}

function decisionTone(decision?: string) {
  const value = (decision || "").toLowerCase();
  if (value.includes("block") || value.includes("deny")) {
    return "bg-red-500/10 text-red-600 dark:text-red-300";
  }
  if (value.includes("ask") || value.includes("warn") || value.includes("approve")) {
    return "bg-amber-500/10 text-amber-600 dark:text-amber-300";
  }
  if (value.includes("allow") || value.includes("observe")) {
    return "bg-emerald-500/10 text-emerald-600 dark:text-emerald-300";
  }
  return "bg-muted text-muted-foreground";
}

export function AgentActivityTab({ agentId }: { agentId: string }) {
  const [events, setEvents] = useState<Observation[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    TelemetryApi.getObservations({ agentId })
      .then((res: { items?: Observation[] }) => {
        if (cancelled) return;
        const sorted = (res.items || []).sort(
          (a, b) =>
            new Date(b.timestamp || 0).getTime() -
            new Date(a.timestamp || 0).getTime(),
        );
        setEvents(sorted.slice(0, 50));
      })
      .catch(console.error)
      .finally(() => !cancelled && setLoading(false));

    // Live updates for this agent only.
    const source = new EventSource(TelemetryApi.streamUrl("observations"));
    const onMessage = (e: MessageEvent) => {
      try {
        const data = JSON.parse(e.data) as Observation;
        if (data.agent_id === agentId) {
          setEvents((prev) => [data, ...prev].slice(0, 50));
        }
      } catch (err) {
        console.error("Failed to parse observation event", err);
      }
    };
    source.addEventListener("message", onMessage);

    return () => {
      cancelled = true;
      source.removeEventListener("message", onMessage);
      source.close();
    };
  }, [agentId]);

  if (loading) {
    return (
      <div className="p-4 text-sm text-muted-foreground">Loading activity…</div>
    );
  }

  if (!events.length) {
    return (
      <div className="flex flex-col items-center justify-center rounded-lg border border-dashed p-8 text-center text-muted-foreground">
        <Eye className="mb-2 h-8 w-8 opacity-50" />
        <p className="text-sm font-medium">Nothing observed yet</p>
        <p className="mt-1 max-w-md text-xs leading-5">
          When this AI app reads a file, calls a tool, or reaches the network,
          Pollek records it here. Run the AI app, or start Observe, to collect
          evidence.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <Activity className="h-3.5 w-3.5" />
        Watching this AI app — {events.length} recent event
        {events.length === 1 ? "" : "s"}. This is observe-only; it does not block.
      </div>
      {events.map((ev, index) => {
        const decision = ev.decision || ev.outcome;
        return (
          <article
            key={`${ev.timestamp ?? index}-${index}`}
            className="rounded-lg border bg-card/60 p-3"
          >
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <h4 className="truncate text-sm font-semibold">
                  {observationTitle(ev)}
                </h4>
                <p className="mt-0.5 text-xs text-muted-foreground">
                  {ev.timestamp
                    ? new Date(ev.timestamp).toLocaleString()
                    : "Time not recorded"}
                </p>
              </div>
              {decision && (
                <span
                  className={`shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium ${decisionTone(
                    decision,
                  )}`}
                >
                  {labelize(decision)}
                </span>
              )}
            </div>
            <TechnicalDetails
              className="mt-3 bg-transparent"
              label="Raw event"
              hint="Full observation payload"
            >
              <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-all rounded-md bg-muted/50 p-3 text-[11px] leading-5">
                {JSON.stringify(ev.details ?? ev, null, 2)}
              </pre>
            </TechnicalDetails>
          </article>
        );
      })}
    </div>
  );
}
