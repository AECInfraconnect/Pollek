import { useEffect, useState } from "react";
import { Activity, ShieldAlert, Server, Users } from "lucide-react";
import { RegistryApi } from "../services/api";

export function Overview() {
  const [metrics, setMetrics] = useState({
    agents: 0,
    mcps: 0,
    tools: 0,
    resources: 0,
  });

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
      <div>
        <h2 className="text-2xl font-bold tracking-tight">
          Dashboard Overview
        </h2>
        <p className="text-muted-foreground">
          Real-time metrics and system health for your local Pollen DEK.
        </p>
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
        <div className="glass col-span-4 rounded-xl p-6">
          <h3 className="font-semibold mb-4">Traffic Overview</h3>
          <div className="h-[300px] flex items-center justify-center border-2 border-dashed border-muted rounded-lg">
            <span className="text-muted-foreground">
              Chart placeholder (Recharts to be added)
            </span>
          </div>
        </div>
        <div className="glass col-span-3 rounded-xl p-6">
          <h3 className="font-semibold mb-4">Recent Audit Events</h3>
          <div className="space-y-4">
            {[1, 2, 3, 4, 5].map((i) => (
              <div key={i} className="flex items-center gap-4">
                <div className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center">
                  <Activity className="h-4 w-4 text-primary" />
                </div>
                <div className="flex-1 space-y-1">
                  <p className="text-sm font-medium leading-none">
                    Decision Denied
                  </p>
                  <p className="text-xs text-muted-foreground">
                    Agent &apos;dev-bot&apos; accessed restricted resource.
                  </p>
                </div>
                <div className="text-xs text-muted-foreground">2m ago</div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
