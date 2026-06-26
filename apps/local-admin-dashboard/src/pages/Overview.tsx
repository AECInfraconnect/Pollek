import { useEffect, useState } from "react";
import { Activity, ShieldAlert, Server, Users, Info } from "lucide-react";
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

  const stats = [
    {
      name: "Active Agents",
      value: metrics.agents.toString(),
      icon: Users,
      change: "Live",
      changeType: "neutral",
    },
    {
      name: "Connected MCPs",
      value: metrics.mcps.toString(),
      icon: Server,
      change: "Live",
      changeType: "neutral",
    },
    {
      name: "Registered Tools",
      value: metrics.tools.toString(),
      icon: Activity,
      change: "Live",
      changeType: "neutral",
    },
    {
      name: "Known Resources",
      value: metrics.resources.toString(),
      icon: ShieldAlert,
      change: "Live",
      changeType: "neutral",
    },
  ];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">
            Dashboard Overview
          </h2>
          <p className="text-muted-foreground">
            Real-time metrics and system health for your local Pollek Local
            Enforcement Kit.
          </p>
        </div>
        <div className="flex items-center gap-2 px-3 py-1 bg-green-500/10 text-green-500 border border-green-500/20 rounded-full">
          <ShieldAlert className="w-4 h-4" />
          <span className="text-xs font-semibold uppercase tracking-wider">
            Local-Only Ready
          </span>
        </div>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        {stats.map((stat) => (
          <div
            key={stat.name}
            className="glass rounded-xl p-6 relative overflow-hidden group"
          >
            <div className="absolute inset-0 bg-gradient-to-br from-primary/10 to-transparent opacity-0 transition-opacity duration-300 group-hover:opacity-100" />
            <div className="relative flex items-center justify-between">
              <span className="text-sm font-medium text-muted-foreground">
                {stat.name}
              </span>
              <stat.icon className="h-4 w-4 text-muted-foreground" />
            </div>
            <div className="mt-4 flex items-baseline gap-2">
              <span className="text-3xl font-bold">{stat.value}</span>
              <span
                className={`text-xs font-medium ${stat.changeType === "positive" ? "text-emerald-500" : stat.changeType === "negative" ? "text-destructive" : "text-muted-foreground"}`}
              >
                {stat.change}
              </span>
            </div>
          </div>
        ))}
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-7">
        <div className="glass col-span-3 rounded-xl p-6 overflow-hidden">
          <h3 className="font-semibold mb-4 flex items-center justify-between">
            <span>What POLLEK can do</span>
            {snapshotLoading && (
              <span className="text-muted-foreground text-sm">Scanning...</span>
            )}
          </h3>
          {snapshot && !snapshotLoading && (
            <div className="space-y-4">
              <div className="flex flex-col gap-2 p-3 bg-secondary/20 rounded-lg">
                <span className="text-sm font-semibold">
                  Local Device: {snapshot.device_id || "local"}
                </span>
                <span className="text-xs text-muted-foreground">
                  Agents Found: {snapshot.agents?.length || metrics.agents || 0}
                </span>
              </div>
              <div className="flex flex-col gap-2">
                <h4 className="text-sm font-semibold mt-2">Control Methods</h4>
                {(
                  (snapshot as any).methods || (snapshot as any).control_methods
                )?.length > 0 ? (
                  (
                    (snapshot as any).methods ||
                    (snapshot as any).control_methods
                  ).map((m: any, idx: number) => (
                    <div
                      key={idx}
                      className="flex items-center justify-between p-2 text-sm border-b border-muted/20"
                    >
                      <div className="flex flex-col">
                        <span className="font-medium capitalize">
                          {(m.method || m.id || "").replace(/_/g, " ")}
                        </span>
                        {m.next_action && (
                          <span className="text-xs text-muted-foreground">
                            {m.next_action.label?.en || "Action Required"}
                          </span>
                        )}
                        {m.domains && (
                          <span className="text-xs text-muted-foreground">
                            {m.domains.join(", ")}
                          </span>
                        )}
                      </div>
                      <span
                        className={`px-2 py-0.5 rounded-full text-xs font-semibold ${
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
                  <span className="text-sm text-muted-foreground p-2">
                    No control methods available
                  </span>
                )}
              </div>
              <div className="mt-4 p-3 bg-blue-500/10 border border-blue-500/20 rounded-lg flex items-start gap-2">
                <Info className="w-4 h-4 text-blue-400 mt-0.5" />
                <p className="text-xs text-blue-300">
                  POLLEK dynamically selects the best control method for each
                  policy. Setup may be required for advanced network controls.
                </p>
              </div>
            </div>
          )}
        </div>
        <div className="glass col-span-4 rounded-xl p-6">
          <h3 className="font-semibold mb-4">Recent Audit Activity</h3>
          <div className="space-y-4">
            {activities.length > 0 ? (
              activities
                .flatMap((set: any) => set.items)
                .slice(0, 5)
                .map((item: any, i: number) => (
                  <div key={i} className="flex items-center gap-4">
                    <div className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center">
                      <Activity className="h-4 w-4 text-primary" />
                    </div>
                    <div className="flex-1 space-y-1">
                      <p className="text-sm font-medium leading-none">
                        {item.event_type} - {item.decision}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        Target: {item.resource} | Reason: {item.reason}
                      </p>
                    </div>
                    <div className="text-xs text-muted-foreground">
                      {new Date(item.timestamp).toLocaleTimeString()}
                    </div>
                  </div>
                ))
            ) : (
              <p className="text-sm text-muted-foreground">
                No recent activity.
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
