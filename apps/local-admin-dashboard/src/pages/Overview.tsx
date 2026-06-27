import { useEffect, useState } from "react";
import { Activity, ShieldAlert, Server, Users, Info, Dot } from "lucide-react";
import { RegistryApi, ActivityApi, PolicyFirstApi } from "../services/api";
import type { LegacyLocalCapabilitySnapshot } from "../services/types";

export function Overview() {
  const [metrics, setMetrics] = useState({
    agents: 0,
    mcps: 0,
    tools: 0,
    resources: 0,
  });
  const [snapshot, setSnapshot] =
    useState<LegacyLocalCapabilitySnapshot | null>(null);
  const [snapshotLoading, setSnapshotLoading] = useState(true);
  const [activities, setActivities] = useState<any[]>([]);

  useEffect(() => {
    Promise.all([
      RegistryApi.listAgents(),
      RegistryApi.listMcpServers(),
      RegistryApi.listTools(),
      RegistryApi.listResources(),
    ])
      .then(([agents, mcps, tools, resources]) => {
        setMetrics({
          agents: agents.length,
          mcps: mcps.length,
          tools: tools.length,
          resources: resources.length,
        });
      })
      .catch(console.error);

    const fetchSnapshot = async () => {
      try {
        setSnapshotLoading(true);
        await PolicyFirstApi.scan();
        const res = await PolicyFirstApi.getLatestSnapshot();
        setSnapshot(res);
      } catch (err) {
        console.error("Failed to load snapshot:", err);
      } finally {
        setSnapshotLoading(false);
      }
    };

    fetchSnapshot();

    ActivityApi.getActivity()
      .then((res: any) => setActivities(res.activity_sets || []))
      .catch(console.error);
  }, []);

  return (
    <div className="space-y-5">
      {/* Header with compact inline metrics */}
      <div className="flex flex-col gap-3">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold tracking-tight">
            Dashboard Overview
          </h2>
          <div className="flex items-center gap-1.5 px-2.5 py-1 bg-green-500/10 text-green-500 border border-green-500/20 rounded-full">
            <ShieldAlert className="w-3.5 h-3.5" />
            <span className="text-[10px] font-semibold uppercase tracking-wider">
              Local-Only Ready
            </span>
          </div>
        </div>

        {/* Compact inline metric strip - replaces big stat cards */}
        <div className="flex flex-wrap items-center gap-1 text-sm">
          <span className="inline-flex items-center gap-1.5 rounded-md border border-border/60 bg-card/50 px-2.5 py-1">
            <Users className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="font-medium">{metrics.agents}</span>
            <span className="text-muted-foreground text-xs">Agents</span>
          </span>
          <Dot className="h-4 w-4 text-muted-foreground/40" />
          <span className="inline-flex items-center gap-1.5 rounded-md border border-border/60 bg-card/50 px-2.5 py-1">
            <Server className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="font-medium">{metrics.mcps}</span>
            <span className="text-muted-foreground text-xs">MCPs</span>
          </span>
          <Dot className="h-4 w-4 text-muted-foreground/40" />
          <span className="inline-flex items-center gap-1.5 rounded-md border border-border/60 bg-card/50 px-2.5 py-1">
            <Activity className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="font-medium">{metrics.tools}</span>
            <span className="text-muted-foreground text-xs">Tools</span>
          </span>
          <Dot className="h-4 w-4 text-muted-foreground/40" />
          <span className="inline-flex items-center gap-1.5 rounded-md border border-border/60 bg-card/50 px-2.5 py-1">
            <ShieldAlert className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="font-medium">{metrics.resources}</span>
            <span className="text-muted-foreground text-xs">Resources</span>
          </span>
        </div>
      </div>

      {/* Two-column content area */}
      <div className="grid gap-4 lg:grid-cols-[2fr_3fr]">
        {/* Left: Control Methods */}
        <div className="rounded-lg border border-border/60 bg-card/30 p-4 overflow-hidden">
          <h3 className="text-sm font-semibold mb-3 flex items-center justify-between">
            <span>Control Methods</span>
            {snapshotLoading && (
              <span className="text-muted-foreground text-xs animate-pulse">Scanning...</span>
            )}
          </h3>
          {snapshot && !snapshotLoading && (
            <div className="space-y-2">
              <div className="flex items-center gap-2 text-xs text-muted-foreground pb-2 border-b border-border/40">
                <span>Device: {snapshot.device_id || "local"}</span>
                <span className="text-muted-foreground/50">|</span>
                <span>Agents: {snapshot.agents?.length || metrics.agents || 0}</span>
              </div>
              <div className="space-y-1">
                {(
                  (snapshot as any).methods || (snapshot as any).control_methods
                )?.length > 0 ? (
                  (
                    (snapshot as any).methods ||
                    (snapshot as any).control_methods
                  ).map((m: any, idx: number) => (
                    <div
                      key={idx}
                      className="flex items-center justify-between py-1.5 text-xs border-b border-border/20 last:border-0"
                    >
                      <div className="flex flex-col gap-0.5">
                        <span className="font-medium capitalize">
                          {(m.method || m.id || "").replace(/_/g, " ")}
                        </span>
                        {m.domains && (
                          <span className="text-[10px] text-muted-foreground">
                            {m.domains.join(", ")}
                          </span>
                        )}
                      </div>
                      <span
                        className={`px-1.5 py-0.5 rounded text-[10px] font-semibold ${
                          m.status === "ready" ||
                          m.status === "ready_after_approval" ||
                          m.status === "Available" ||
                          m.status === "installed"
                            ? "bg-emerald-500/20 text-emerald-400"
                            : m.status === "installed_inactive" ||
                                m.status === "Degraded"
                              ? "bg-amber-500/20 text-amber-400"
                              : "bg-rose-500/20 text-rose-400"
                        }`}
                      >
                        {(m.status || "unknown").replace(/_/g, " ")}
                      </span>
                    </div>
                  ))
                ) : (
                  <span className="text-xs text-muted-foreground py-2">
                    No control methods available
                  </span>
                )}
              </div>
              <div className="mt-2 p-2 bg-blue-500/10 border border-blue-500/20 rounded flex items-start gap-1.5">
                <Info className="w-3 h-3 text-blue-400 mt-0.5 shrink-0" />
                <p className="text-[10px] text-blue-300 leading-relaxed">
                  POLLEK dynamically selects the best control method for each
                  policy.
                </p>
              </div>
            </div>
          )}
        </div>

        {/* Right: Recent Activity */}
        <div className="rounded-lg border border-border/60 bg-card/30 p-4">
          <h3 className="text-sm font-semibold mb-3">Recent Audit Activity</h3>
          <div className="space-y-2">
            {activities.length > 0 ? (
              activities
                .flatMap((set: any) => set.items)
                .slice(0, 8)
                .map((item: any, i: number) => (
                  <div key={i} className="flex items-center gap-3 py-1.5 border-b border-border/20 last:border-0">
                    <div className="h-6 w-6 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                      <Activity className="h-3 w-3 text-primary" />
                    </div>
                    <div className="flex-1 min-w-0">
                      <p className="text-xs font-medium leading-none truncate">
                        {item.event_type} — {item.decision}
                      </p>
                      <p className="text-[10px] text-muted-foreground mt-0.5 truncate">
                        {item.resource} | {item.reason}
                      </p>
                    </div>
                    <span className="text-[10px] text-muted-foreground shrink-0">
                      {new Date(item.timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                    </span>
                  </div>
                ))
            ) : (
              <p className="text-xs text-muted-foreground py-4 text-center">
                No recent activity.
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
