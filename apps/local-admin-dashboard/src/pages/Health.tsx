import { useEffect, useState } from "react";
import { Activity, CheckCircle2, Cloud, Cpu, ShieldAlert } from "lucide-react";
import { CapabilityApi, defaultClient } from "../services/api";
import type { ContractDiscoveryResponse, LocalCapabilitySnapshotV2 } from "../services/api";
import { statusToken, type UiStatus } from "../lib/status";
import { cn } from "@/lib/utils";
import { useMode } from "../context/ModeContext";
import type { RuntimeModeV2 } from "../services/types";
import { appModeToRuntimeMode } from "../lib/modes";

function statusFor(ok: boolean, degraded = false): UiStatus {
  if (ok) return "ok";
  if (degraded) return "degraded";
  return "failed";
}

function HealthTile({
  title,
  detail,
  status,
  icon: Icon,
}: {
  title: string;
  detail: string;
  status: UiStatus;
  icon: any;
}) {
  const token = statusToken(status);
  return (
    <div className="rounded-lg border bg-card/60 p-4">
      <div className="flex items-start gap-3">
        <div className={cn("rounded-lg p-2", token.bg)}>
          <Icon className={cn("h-4 w-4", token.text)} />
        </div>
        <div className="min-w-0">
          <h3 className="text-sm font-semibold">{title}</h3>
          <p className="mt-1 text-xs text-muted-foreground">{detail}</p>
        </div>
      </div>
    </div>
  );
}

export function Health() {
  const { mode } = useMode();
  const runtimeMode: RuntimeModeV2 = appModeToRuntimeMode(mode);
  const [contract, setContract] = useState<ContractDiscoveryResponse | null>(
    null,
  );
  const [snapshot, setSnapshot] = useState<LocalCapabilitySnapshotV2 | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([
      defaultClient.getContractDiscovery(),
      CapabilityApi.getSnapshotV2(runtimeMode),
    ])
      .then(([contractData, snapshotData]) => {
        setContract(contractData);
        setSnapshot(snapshotData);
        setError(null);
      })
      .catch((err) => {
        console.error(err);
        setError(String(err?.message ?? err));
      });
  }, [runtimeMode]);

  const readyMethods =
    snapshot?.control_methods.filter((method) => method.status === "available")
      .length ?? 0;
  const observeSignals = snapshot?.observation_sources.length ?? 0;
  const hasCloudContract = Boolean(
    (contract as any)?.interfaces?.["pollek.cloud.telemetry"],
  );

  return (
    <div className="p-6 md:p-8 space-y-6">
      <div>
        <h2 className="text-lg font-semibold tracking-tight">Health</h2>
        <p className="text-sm text-muted-foreground">
          {snapshot
            ? `${snapshot.device_id} / contract ${snapshot.contract.local_contract_version}`
            : "Local Control Plane"}
        </p>
      </div>

      {error && (
        <div className="rounded-lg border border-rose-500/30 bg-rose-500/10 p-4 text-sm text-rose-500">
          {error}
        </div>
      )}

      <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
        <HealthTile
          icon={Activity}
          title="Local API"
          detail={contract ? "Contract discovery responded" : "Waiting for discovery"}
          status={statusFor(Boolean(contract))}
        />
        <HealthTile
          icon={Cpu}
          title="Host Capabilities"
          detail={`${readyMethods} ready methods, ${observeSignals} observation sources`}
          status={statusFor(Boolean(snapshot), readyMethods === 0)}
        />
        <HealthTile
          icon={Cloud}
          title="Cloud Contract"
          detail={
            hasCloudContract
              ? "Cloud telemetry and hot-reload interfaces advertised"
              : "Local dashboard remains usable without cloud"
          }
          status={statusFor(hasCloudContract, true)}
        />
        <HealthTile
          icon={CheckCircle2}
          title="Contract Compatibility"
          detail={snapshot?.contract.status ?? "Unknown"}
          status={statusFor(snapshot?.contract.status === "compatible", true)}
        />
      </div>

      <section className="space-y-3">
        <h3 className="text-sm font-semibold">Advertised Interfaces</h3>
        <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
          {Object.entries((contract as any)?.interfaces ?? {}).map(
            ([name, iface]: [string, any]) => (
              <div key={name} className="rounded-lg border bg-card/60 p-4">
                <div className="flex items-start gap-3">
                  <div className="rounded-lg bg-sky-500/10 p-2 text-sky-500">
                    <ShieldAlert className="h-4 w-4" />
                  </div>
                  <div className="min-w-0">
                    <h4 className="text-sm font-semibold">{name}</h4>
                    <p className="mt-1 text-xs text-muted-foreground">
                      {iface.schema} / {iface.direction}
                    </p>
                    <div className="mt-3 flex flex-wrap gap-1.5 text-[11px] text-muted-foreground">
                      {iface.hot_reload && (
                        <span className="rounded-md border bg-background px-2 py-0.5">
                          Hot reload
                        </span>
                      )}
                      {iface.requires_spiffe && (
                        <span className="rounded-md border bg-background px-2 py-0.5">
                          SPIFFE
                        </span>
                      )}
                      {iface.requires_oauth && (
                        <span className="rounded-md border bg-background px-2 py-0.5">
                          OAuth
                        </span>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            ),
          )}
        </div>
      </section>
    </div>
  );
}
