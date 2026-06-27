import { useEffect, useMemo, useState } from "react";
import {
  Activity,
  Cpu,
  Eye,
  RefreshCw,
  ShieldCheck,
  ShieldOff,
  Wrench,
} from "lucide-react";
import { toast } from "sonner";
import { CapabilityApi } from "../services/api";
import type {
  ControlMethodCapabilityV2,
  LocalCapabilitySnapshotV2,
  MethodReadinessV2,
  ObservationSourceCapabilityV2,
  RuntimeModeV2,
  SetupActionV2,
} from "../services/types";
import { statusToken, type UiStatus } from "../lib/status";
import { cn } from "@/lib/utils";
import { useMode } from "../context/ModeContext";
import { appModeToRuntimeMode } from "../lib/modes";

type DemoTarget = "host" | "windows" | "linux" | "macos";
type DemoProfile = "ready" | "observe_only" | "needs_setup";

function readinessStatus(status: MethodReadinessV2): UiStatus {
  if (status === "available") return "ok";
  if (status === "simulator_only") return "info";
  if (status === "failed" || status === "unsupported") return "failed";
  if (
    status === "needs_install" ||
    status === "needs_permission" ||
    status === "needs_configuration" ||
    status === "degraded"
  ) {
    return "degraded";
  }
  return "idle";
}

function labelize(value: string) {
  return value
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function CapabilityCard({
  method,
  setupActions,
}: {
  method: ControlMethodCapabilityV2;
  setupActions: SetupActionV2[];
}) {
  const status = readinessStatus(method.status);
  const token = statusToken(status);
  const Icon =
    method.status === "available"
      ? ShieldCheck
      : method.status === "simulator_only"
        ? Eye
        : ShieldOff;
  const actionTitles = method.setup_action_ids
    .map((id) => setupActions.find((action) => action.action_id === id))
    .filter(Boolean)
    .map((action) => action!.title_en);

  return (
    <div className="rounded-lg border bg-card/60 p-4">
      <div className="flex items-start gap-3">
        <div className={cn("rounded-lg p-2", token.bg)}>
          <Icon className={cn("h-4 w-4", token.text)} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-3">
            <h3 className="truncate text-sm font-semibold">
              {method.display_name_en}
            </h3>
            <span
              className={cn(
                "shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium",
                token.bg,
                token.text,
              )}
            >
              {labelize(method.status)}
            </span>
          </div>
          <p className="mt-1 text-xs text-muted-foreground">
            {method.status === "available"
              ? `Can ${method.max_level} these domains on this device.`
              : method.status === "simulator_only"
                ? "Simulation signal only. Real blocking is not enabled."
                : actionTitles[0] ?? "Needs setup before real enforcement."}
          </p>
          <div className="mt-3 flex flex-wrap gap-1.5">
            {method.domains.map((domain) => (
              <span
                key={domain}
                className="rounded-md border bg-background px-2 py-0.5 text-[11px] text-muted-foreground"
              >
                {labelize(domain)}
              </span>
            ))}
          </div>
          <div className="mt-3 grid grid-cols-3 gap-2 text-[11px] text-muted-foreground">
            <div>
              <span className="block text-foreground">{labelize(method.max_level)}</span>
              Max level
            </div>
            <div>
              <span className="block text-foreground">{labelize(method.maturity)}</span>
              Maturity
            </div>
            <div>
              <span className="block text-foreground">{labelize(method.install_state)}</span>
              Install
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function ObservationCard({ source }: { source: ObservationSourceCapabilityV2 }) {
  const status = readinessStatus(source.status);
  const token = statusToken(status);
  return (
    <div className="rounded-lg border bg-card/60 p-4">
      <div className="flex items-start gap-3">
        <div className={cn("rounded-lg p-2", token.bg)}>
          <Activity className={cn("h-4 w-4", token.text)} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-3">
            <h3 className="truncate text-sm font-semibold">
              {source.display_name_en}
            </h3>
            <span className={cn("rounded-full px-2 py-0.5 text-[11px]", token.bg, token.text)}>
              {labelize(source.status)}
            </span>
          </div>
          <p className="mt-1 text-xs text-muted-foreground">
            {source.privacy_note_en}
          </p>
          <div className="mt-3 flex flex-wrap gap-1.5">
            {source.domains.map((domain) => (
              <span
                key={domain}
                className="rounded-md border bg-background px-2 py-0.5 text-[11px] text-muted-foreground"
              >
                {labelize(domain)}
              </span>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

function SetupActionRow({ action }: { action: SetupActionV2 }) {
  return (
    <div className="rounded-lg border bg-card/60 p-4">
      <div className="flex items-start gap-3">
        <div className="rounded-lg bg-amber-500/10 p-2 text-amber-500">
          <Wrench className="h-4 w-4" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-3">
            <h3 className="truncate text-sm font-semibold">{action.title_en}</h3>
            <span className="rounded-full bg-muted px-2 py-0.5 text-[11px] text-muted-foreground">
              {action.requires_admin ? "Admin" : "User"}
            </span>
          </div>
          <p className="mt-1 text-xs text-muted-foreground">{action.detail_en}</p>
        </div>
      </div>
    </div>
  );
}

export function Capabilities() {
  const { mode } = useMode();
  const runtimeMode: RuntimeModeV2 = appModeToRuntimeMode(mode);
  const [snapshot, setSnapshot] = useState<LocalCapabilitySnapshotV2 | null>(
    null,
  );
  const [loading, setLoading] = useState(true);
  const [demoTarget, setDemoTarget] = useState<DemoTarget>("host");
  const [demoProfile, setDemoProfile] = useState<DemoProfile>("ready");

  const load = async (refresh = false) => {
    setLoading(true);
    try {
      const demo =
        demoTarget === "host"
          ? undefined
          : { os: demoTarget, profile: demoProfile };
      const data = refresh
        ? await CapabilityApi.refreshSnapshotV2(runtimeMode, demo)
        : await CapabilityApi.getSnapshotV2(runtimeMode, demo);
      setSnapshot(data);
    } catch (error) {
      console.error(error);
      toast.error("Failed to load local capabilities");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load(false);
  }, [runtimeMode, demoTarget, demoProfile]);

  const counts = useMemo(() => {
    const methods = snapshot?.control_methods ?? [];
    return {
      available: methods.filter((m) => m.status === "available").length,
      setup: methods.filter((m) =>
        ["needs_install", "needs_permission", "needs_configuration"].includes(
          m.status,
        ),
      ).length,
      simulator: methods.filter((m) => m.status === "simulator_only").length,
    };
  }, [snapshot]);

  return (
    <div className="p-6 md:p-8 space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold tracking-tight">
            Capabilities
          </h2>
          <div className="mt-1 flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
            <span>
              {snapshot
                ? `${snapshot.os.family} ${snapshot.os.version} / ${snapshot.device_id}`
                : "Local capability snapshot"}
            </span>
            {snapshot?.contract.reason_code === "demo_fixture" && (
              <span className="rounded-md border border-sky-500/30 bg-sky-500/10 px-2 py-0.5 text-[11px] text-sky-600">
                Demo Fixture
              </span>
            )}
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <div className="inline-flex h-9 overflow-hidden rounded-md border bg-background">
            {(["host", "windows", "linux", "macos"] as DemoTarget[]).map(
              (target) => (
                <button
                  key={target}
                  type="button"
                  onClick={() => setDemoTarget(target)}
                  className={cn(
                    "px-3 text-sm capitalize hover:bg-muted",
                    demoTarget === target && "bg-muted text-foreground",
                  )}
                >
                  {target}
                </button>
              ),
            )}
          </div>
          {demoTarget !== "host" && (
            <select
              value={demoProfile}
              onChange={(event) =>
                setDemoProfile(event.target.value as DemoProfile)
              }
              className="h-9 rounded-md border bg-background px-3 text-sm"
            >
              <option value="ready">Ready</option>
              <option value="observe_only">Observe</option>
              <option value="needs_setup">Setup</option>
            </select>
          )}
          <button
            type="button"
            onClick={() => load(true)}
            disabled={loading}
            className="inline-flex h-9 items-center gap-2 rounded-md border bg-background px-3 text-sm hover:bg-muted disabled:opacity-50"
          >
            <RefreshCw className={cn("h-4 w-4", loading && "animate-spin")} />
            Refresh
          </button>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-3 md:grid-cols-4">
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="flex items-center gap-2 text-sm font-medium">
            <Cpu className="h-4 w-4 text-sky-500" />
            {snapshot?.os.arch ?? "Unknown arch"}
          </div>
          <p className="mt-1 text-xs text-muted-foreground">
            {snapshot?.os.elevated ? "Elevated session" : "User session"}
          </p>
        </div>
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="text-lg font-semibold">{counts.available}</div>
          <p className="text-xs text-muted-foreground">Ready methods</p>
        </div>
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="text-lg font-semibold">{counts.setup}</div>
          <p className="text-xs text-muted-foreground">Need setup</p>
        </div>
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="text-lg font-semibold">{counts.simulator}</div>
          <p className="text-xs text-muted-foreground">Simulator only</p>
        </div>
      </div>

      <section className="space-y-3">
        <h3 className="text-sm font-semibold">Control Methods</h3>
        <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
          {(snapshot?.control_methods ?? []).map((method) => (
            <CapabilityCard
              key={method.method_id}
              method={method}
              setupActions={snapshot?.setup_actions ?? []}
            />
          ))}
        </div>
      </section>

      <section className="space-y-3">
        <h3 className="text-sm font-semibold">Observation Sources</h3>
        <div className="grid grid-cols-1 gap-3 xl:grid-cols-3">
          {(snapshot?.observation_sources ?? []).map((source) => (
            <ObservationCard key={source.source_id} source={source} />
          ))}
        </div>
      </section>

      {(snapshot?.setup_actions.length ?? 0) > 0 && (
        <section className="space-y-3">
          <h3 className="text-sm font-semibold">Setup Actions</h3>
          <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
            {snapshot!.setup_actions.map((action) => (
              <SetupActionRow key={action.action_id} action={action} />
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
