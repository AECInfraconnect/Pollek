import { useEffect, useMemo, useState } from "react";
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  Cpu,
  Database,
  Eye,
  Monitor,
  RefreshCw,
  Server,
  ShieldAlert,
  ShieldCheck,
  Users,
  Wrench,
} from "lucide-react";
import {
  ActivityApi,
  CapabilityApi,
  RegistryApi,
} from "../services/api";
import type {
  ControlMethodCapabilityV2,
  LocalCapabilitySnapshotV2,
  ObservationSourceCapabilityV2,
  SetupActionV2,
} from "../services/types";
import { cn } from "@/lib/utils";
import { ContextualHelp } from "../components/help/ContextualHelp";
import { formatDisplayValue, renderDisplayValue } from "../lib/displayValue";

type Metrics = {
  agents: number;
  mcps: number;
  tools: number;
  resources: number;
};

const readinessTone: Record<string, string> = {
  available: "bg-emerald-500/10 text-emerald-500 border-emerald-500/20",
  degraded: "bg-amber-500/10 text-amber-500 border-amber-500/20",
  needs_install: "bg-blue-500/10 text-blue-500 border-blue-500/20",
  needs_permission: "bg-amber-500/10 text-amber-500 border-amber-500/20",
  needs_configuration: "bg-blue-500/10 text-blue-500 border-blue-500/20",
  simulator_only: "bg-purple-500/10 text-purple-500 border-purple-500/20",
  unsupported: "bg-muted text-muted-foreground border-border",
  failed: "bg-red-500/10 text-red-500 border-red-500/20",
};

function pretty(value?: unknown) {
  if (!value) return "-";
  return formatDisplayValue(value).replace(/_/g, " ");
}

function formatDateTime(value?: string) {
  if (!value) return "Not recorded";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

function readinessClass(status?: string) {
  return readinessTone[status ?? ""] ?? "bg-muted text-muted-foreground border-border";
}

function methodCanEnforce(method: ControlMethodCapabilityV2) {
  return method.max_level === "enforce" || method.max_level === "strict_deny";
}

function isControlMethod(
  row: ControlMethodCapabilityV2 | ObservationSourceCapabilityV2,
): row is ControlMethodCapabilityV2 {
  return "method_id" in row;
}

function SnapshotSummary({
  snapshot,
  loading,
  onRefresh,
}: {
  snapshot: LocalCapabilitySnapshotV2 | null;
  loading: boolean;
  onRefresh: () => void;
}) {
  const deviceName =
    snapshot?.device_id ||
    (typeof navigator !== "undefined" ? navigator.userAgent.split(" ")[0] : "local");

  return (
    <section className="rounded-lg border border-border/70 bg-card/50 p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex items-start gap-3">
          <div className="rounded-lg bg-primary/10 p-2 text-primary">
            <Monitor className="h-5 w-5" />
          </div>
          <div>
            <p
              className="text-xs font-semibold uppercase tracking-wide text-muted-foreground"
              data-testid="current-device-label"
            >
              <span className="inline-flex items-center gap-1.5">
                Current Device
                <ContextualHelp topicId="overview.current_device" />
              </span>
            </p>
            <h1 className="text-2xl font-bold">{deviceName}</h1>
            <p className="mt-1 text-sm text-muted-foreground">
              {snapshot
                ? `${snapshot.os.family} ${snapshot.os.version} (${snapshot.os.arch})`
                : "Loading local capability snapshot"}
            </p>
          </div>
        </div>
        <button
          type="button"
          onClick={onRefresh}
          disabled={loading}
          className="inline-flex h-9 items-center gap-2 rounded-lg border bg-background px-3 text-sm font-medium hover:bg-muted disabled:opacity-50"
        >
          <RefreshCw className={cn("h-4 w-4", loading && "animate-spin")} />
          Refresh
        </button>
      </div>

      <div className="mt-4 grid gap-3 md:grid-cols-4">
        <Fact label="Mode" value={pretty(snapshot?.mode)} source="dashboard mode" />
        <Fact
          label="Privilege"
          value={snapshot?.os.elevated ? "Elevated" : "User level"}
          source="OS probe"
        />
        <Fact
          label="Contract"
          value={snapshot?.contract.status ?? "-"}
          source={snapshot?.contract.local_contract_version ?? "contract hub"}
        />
        <Fact
          label="Generated"
          value={formatDateTime(snapshot?.generated_at)}
          source="capability snapshot v2"
        />
      </div>
    </section>
  );
}

function Fact({
  label,
  value,
  source,
}: {
  label: string;
  value: unknown;
  source: unknown;
}) {
  return (
    <div className="rounded-lg border border-border/60 bg-background/40 p-3">
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className="mt-1 truncate text-sm font-semibold capitalize">
        {formatDisplayValue(value)}
      </p>
      <p className="mt-1 truncate text-[11px] text-muted-foreground">
        Source: {formatDisplayValue(source)}
      </p>
    </div>
  );
}

function MetricStrip({ metrics }: { metrics: Metrics }) {
  const items = [
    { label: "Agents", value: metrics.agents, icon: Users },
    { label: "MCP Servers", value: metrics.mcps, icon: Server },
    { label: "Tools", value: metrics.tools, icon: Wrench },
    { label: "Resources", value: metrics.resources, icon: Database },
  ];

  return (
    <section className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
      {items.map((item) => {
        const Icon = item.icon;
        return (
          <div
            key={item.label}
            className="rounded-lg border border-border/70 bg-card/50 p-4"
          >
            <div className="flex items-center justify-between">
              <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                {item.label}
              </p>
              <Icon className="h-4 w-4 text-muted-foreground" />
            </div>
            <p className="mt-2 text-2xl font-bold">{item.value}</p>
            <p className="mt-1 text-xs text-muted-foreground">
              Source: registry endpoint
            </p>
          </div>
        );
      })}
    </section>
  );
}

function CapabilityList({
  title,
  icon: Icon,
  methods,
  sources,
  helpTopicId,
}: {
  title: string;
  icon: any;
  methods?: ControlMethodCapabilityV2[];
  sources?: ObservationSourceCapabilityV2[];
  helpTopicId?: string;
}) {
  const rows: Array<ControlMethodCapabilityV2 | ObservationSourceCapabilityV2> =
    methods ?? sources ?? [];
  return (
    <section className="rounded-lg border border-border/70 bg-card/50">
      <div className="border-b border-border/50 px-4 py-3">
        <div className="flex items-center gap-2">
          <Icon className="h-4 w-4 text-primary" />
          <h2 className="text-sm font-semibold">{title}</h2>
          <ContextualHelp topicId={helpTopicId} />
        </div>
      </div>
      <div className="divide-y divide-border/30">
        {rows.slice(0, 8).map((row) => {
          const controlMethod = isControlMethod(row);
          return (
            <div
              key={controlMethod ? row.method_id : row.source_id}
              className="p-4"
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div>
                  <p className="text-sm font-semibold">
                    {renderDisplayValue(
                      controlMethod ? row.display_name_en : row.display_name_en,
                    )}
                  </p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    {renderDisplayValue(row.domains.map(pretty).join(", "))}
                  </p>
                </div>
                <span
                  className={cn(
                    "rounded-full border px-2 py-0.5 text-[11px] font-semibold capitalize",
                    readinessClass(row.status),
                  )}
                >
                  {pretty(row.status)}
                </span>
              </div>
              {controlMethod && (
                <div className="mt-2 flex flex-wrap gap-2 text-[11px] text-muted-foreground">
                  <span>Max: {pretty(row.max_level)}</span>
                  <span>Maturity: {pretty(row.maturity)}</span>
                  <span>Install: {pretty(row.install_state)}</span>
                </div>
              )}
              {controlMethod && row.limitations_en.length > 0 && (
                <p className="mt-2 text-xs leading-5 text-muted-foreground">
                  {renderDisplayValue(row.limitations_en[0])}
                </p>
              )}
              {!controlMethod && (
                <p className="mt-2 text-xs leading-5 text-muted-foreground">
                  {renderDisplayValue(row.privacy_note_en)}
                </p>
              )}
            </div>
          );
        })}
        {rows.length === 0 && (
          <div className="p-6 text-center text-sm text-muted-foreground">
            No capabilities reported yet.
          </div>
        )}
      </div>
    </section>
  );
}

function SetupActions({ actions }: { actions: SetupActionV2[] }) {
  return (
    <section className="rounded-lg border border-border/70 bg-card/50">
      <div className="border-b border-border/50 px-4 py-3">
        <div className="flex items-center gap-2">
          <AlertTriangle className="h-4 w-4 text-amber-500" />
          <h2 className="text-sm font-semibold">Setup Needed</h2>
        </div>
      </div>
      <div className="divide-y divide-border/30">
        {actions.slice(0, 6).map((action) => (
          <div key={action.action_id} className="p-4">
            <div className="flex flex-wrap items-start justify-between gap-2">
              <div>
                <p className="text-sm font-semibold">
                  {renderDisplayValue(action.title_en)}
                </p>
                <p className="mt-1 text-xs leading-5 text-muted-foreground">
                  {renderDisplayValue(action.detail_en)}
                </p>
              </div>
              <span className="rounded-full border border-border px-2 py-0.5 text-[11px] text-muted-foreground">
                {action.estimated_minutes} min
              </span>
            </div>
            <div className="mt-2 flex flex-wrap gap-2 text-[11px] text-muted-foreground">
              {action.requires_admin && <span>Requires admin</span>}
              {action.requires_restart && <span>Requires restart</span>}
              {action.safe_to_skip && <span>Safe to skip</span>}
            </div>
          </div>
        ))}
        {actions.length === 0 && (
          <div className="flex items-center gap-2 p-4 text-sm text-emerald-500">
            <CheckCircle2 className="h-4 w-4" />
            No required setup actions in the latest snapshot.
          </div>
        )}
      </div>
    </section>
  );
}

export function Overview() {
  const [metrics, setMetrics] = useState<Metrics>({
    agents: 0,
    mcps: 0,
    tools: 0,
    resources: 0,
  });
  const [snapshot, setSnapshot] = useState<LocalCapabilitySnapshotV2 | null>(
    null,
  );
  const [snapshotLoading, setSnapshotLoading] = useState(true);
  const [activities, setActivities] = useState<any[]>([]);

  const loadSnapshot = async (refresh = false) => {
    setSnapshotLoading(true);
    try {
      const next = refresh
        ? await CapabilityApi.refreshSnapshotV2("desktop_advanced")
        : await CapabilityApi.getSnapshotV2("desktop_advanced");
      setSnapshot(next);
    } catch {
      setSnapshot(null);
    } finally {
      setSnapshotLoading(false);
    }
  };

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
      .catch(() => {
        setMetrics({ agents: 0, mcps: 0, tools: 0, resources: 0 });
      });

    void loadSnapshot();

    ActivityApi.getActivity()
      .then((res: any) => setActivities(res.activity_sets || []))
      .catch(() => setActivities([]));
  }, []);

  const capabilitySummary = useMemo(() => {
    const methods = snapshot?.control_methods ?? [];
    const enforceable = methods.filter(methodCanEnforce).length;
    const available = methods.filter((method) => method.status === "available").length;
    const observeSources = snapshot?.observation_sources.filter(
      (source) => source.status === "available" || source.status === "degraded",
    ).length ?? 0;
    return { enforceable, available, observeSources };
  }, [snapshot]);

  const recentItems = activities.flatMap((set: any) => set.items ?? []).slice(0, 8);

  return (
    <div className="space-y-5">
      <div>
        <h2 className="text-lg font-semibold tracking-tight">
          <span className="inline-flex items-center gap-2">
            Dashboard Overview
            <ContextualHelp topicId="overview.dashboard" />
          </span>
        </h2>
        <p className="text-sm text-muted-foreground">
          Local device posture, registered entities, observation sources, and
          control capability readiness from the active local service.
        </p>
      </div>

      <SnapshotSummary
        snapshot={snapshot}
        loading={snapshotLoading}
        onRefresh={() => void loadSnapshot(true)}
      />

      <MetricStrip metrics={metrics} />

      <section className="grid gap-3 md:grid-cols-3">
        <div className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 p-4">
          <ShieldCheck className="h-4 w-4 text-emerald-500" />
          <p className="mt-3 text-2xl font-bold">{capabilitySummary.enforceable}</p>
          <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Enforce-capable methods
          </p>
        </div>
        <div className="rounded-lg border border-blue-500/20 bg-blue-500/10 p-4">
          <Eye className="h-4 w-4 text-blue-500" />
          <p className="mt-3 text-2xl font-bold">{capabilitySummary.observeSources}</p>
          <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Observation sources
          </p>
        </div>
        <div className="rounded-lg border border-purple-500/20 bg-purple-500/10 p-4">
          <Cpu className="h-4 w-4 text-purple-500" />
          <p className="mt-3 text-2xl font-bold">{capabilitySummary.available}</p>
          <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Ready methods
          </p>
        </div>
      </section>

      <div className="grid gap-4 xl:grid-cols-[1fr_1fr]">
        <CapabilityList
          title="Control Capabilities"
          icon={ShieldAlert}
          methods={snapshot?.control_methods ?? []}
          helpTopicId="capability.control_methods"
        />
        <CapabilityList
          title="Observation Sources"
          icon={Activity}
          sources={snapshot?.observation_sources ?? []}
          helpTopicId="activity.timeline"
        />
      </div>

      <div className="grid gap-4 xl:grid-cols-[1fr_1fr]">
        <SetupActions actions={snapshot?.setup_actions ?? []} />
        <section className="rounded-lg border border-border/70 bg-card/50">
          <div className="border-b border-border/50 px-4 py-3">
            <h2 className="text-sm font-semibold">Recent Audit Activity</h2>
            <p className="text-xs text-muted-foreground">
              Source: local activity endpoint
            </p>
          </div>
          <div className="divide-y divide-border/30">
            {recentItems.length > 0 ? (
              recentItems.map((item: any, index: number) => (
                <div key={`${item.event_id ?? index}`} className="flex gap-3 p-4">
                  <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary/10">
                    <Activity className="h-4 w-4 text-primary" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium">
                      {renderDisplayValue(
                        `${formatDisplayValue(item.event_type ?? item.action ?? "activity")} - ${formatDisplayValue(
                          item.decision ?? "observed",
                        )}`,
                      )}
                    </p>
                    <p className="mt-1 truncate text-xs text-muted-foreground">
                      {renderDisplayValue(
                        item.resource ?? item.reason ?? "No resource detail",
                      )}
                    </p>
                  </div>
                  <span className="shrink-0 text-xs text-muted-foreground">
                    {formatDateTime(item.timestamp)}
                  </span>
                </div>
              ))
            ) : (
              <div className="p-6 text-center text-sm text-muted-foreground">
                No recent activity.
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
