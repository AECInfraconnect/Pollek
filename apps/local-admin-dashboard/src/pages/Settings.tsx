import { useState, useEffect } from "react";
import {
  LOCAL_CONTROL_PLANE_DEFAULT_ORIGIN,
  MOCK_CLOUD_DEFAULT_ORIGIN,
  switchProfile,
  defaultClient,
} from "../services/api";
import type { ContractDiscoveryResponse } from "../services/api";
import { toast } from "sonner";
import { Activity, CheckCircle2, CloudOff, FileCode2, ShieldAlert } from "lucide-react";

export function Settings() {
  const [profile, setProfile] = useState<"local" | "mock-cloud">("local");
  const [discovery, setDiscovery] = useState<ContractDiscoveryResponse | null>(
    null,
  );
  const [discoveryError, setDiscoveryError] = useState<string | null>(null);
  const [defVersion, setDefVersion] = useState<string>("20260621000");
  const [checkingUpdates, setCheckingUpdates] = useState(false);

  useEffect(() => {
    const p = localStorage.getItem("dek_admin_profile");
    if (p === "mock-cloud") setProfile("mock-cloud");
    loadDiscovery();
  }, []);

  const loadDiscovery = async () => {
    try {
      setDiscoveryError(null);
      const res = await defaultClient.getContractDiscovery();
      setDiscovery(res);
    } catch (e: any) {
      setDiscoveryError(e.message || String(e));
    }
  };

  const handleProfileChange = (newProfile: "local" | "mock-cloud") => {
    setProfile(newProfile);
    switchProfile(newProfile); // This will reload the page
  };

  const checkUpdates = () => {
    if (checkingUpdates) return;
    setCheckingUpdates(true);
    setTimeout(() => {
      setDefVersion("20260621001 (Hot-reloaded)");
      setCheckingUpdates(false);
      toast.success("Definitions updated successfully via dek-bundle-sync");
    }, 1500);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold tracking-tight">Settings</h2>
          <p className="text-muted-foreground">
            Configure local service settings and synchronization profiles.
          </p>
        </div>
      </div>

      <div className="glass p-6 rounded-xl space-y-6">
        <h3 className="text-lg font-medium">Local Service Profile</h3>

        <div className="space-y-4 max-w-md">
          <div className="grid gap-2">
            <label htmlFor="settings-active-profile" className="text-sm font-medium">
              Active Profile
            </label>
            <select
              id="settings-active-profile"
              value={profile}
              onChange={(e) => handleProfileChange(e.target.value as any)}
              className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="local">
                Local service (127.0.0.1:43891)
              </option>
              <option value="mock-cloud">
                Mock Pollek Cloud (127.0.0.1:43892)
              </option>
            </select>
          </div>
          <div className="grid gap-2">
            <label htmlFor="settings-api-endpoint" className="text-sm font-medium">
              API Endpoint
            </label>
            <input
              id="settings-api-endpoint"
              type="text"
              className="flex h-10 w-full rounded-md border border-input bg-muted/50 px-3 py-2 text-sm text-muted-foreground"
              value={
                profile === "mock-cloud"
                  ? MOCK_CLOUD_DEFAULT_ORIGIN
                  : defaultClient.rootUrl ||
                    `same origin (${LOCAL_CONTROL_PLANE_DEFAULT_ORIGIN})`
              }
              disabled
            />
          </div>
          <div className="grid gap-2">
            <label htmlFor="settings-mock-role" className="text-sm font-medium">
              Mock Role
            </label>
            <input
              id="settings-mock-role"
              type="text"
              className="flex h-10 w-full rounded-md border border-input bg-muted/50 px-3 py-2 text-sm text-muted-foreground"
              value={profile === "mock-cloud" ? "admin" : ""}
              disabled
            />
          </div>
        </div>
      </div>

      <div className="glass p-6 rounded-xl space-y-6">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-medium">Contract Discovery</h3>
          <button
            onClick={loadDiscovery}
            className="px-3 py-1 bg-secondary text-secondary-foreground rounded text-xs hover:opacity-80"
          >
            Refresh
          </button>
        </div>

        {discoveryError ? (
          <div className="text-sm text-red-500 bg-red-500/10 p-4 rounded-md">
            Failed to load discovery: {discoveryError}
          </div>
        ) : discovery ? (
          <div className="space-y-4">
            <ContractHubStatusCard discovery={discovery} />

            <div className="grid grid-cols-2 gap-4 text-sm">
              <div className="space-y-1">
                <span className="text-muted-foreground block">
                  Preferred Contract
                </span>
                <span className="font-medium bg-primary/10 text-primary px-2 py-1 rounded inline-block">
                  {discovery.preferred}
                </span>
              </div>
              <div className="space-y-1">
                <span className="text-muted-foreground block">
                  Schema Version
                </span>
                <span className="font-medium">{discovery.schema_version}</span>
              </div>
            </div>

            <div className="space-y-2">
              <span className="text-sm text-muted-foreground block">
                Supported Contracts
              </span>
              <div className="flex flex-wrap gap-2">
                {discovery.supported.map((s) => (
                  <span
                    key={s}
                    className="text-xs bg-muted px-2 py-1 rounded-full"
                  >
                    {s}
                  </span>
                ))}
              </div>
            </div>

            <div className="space-y-2">
              <span className="text-sm text-muted-foreground block">
                Capabilities
              </span>
              <div className="flex flex-wrap gap-2">
                {discovery.capabilities.map((c) => (
                  <span
                    key={c}
                    className="text-xs bg-muted px-2 py-1 rounded-full"
                  >
                    {c}
                  </span>
                ))}
              </div>
            </div>
          </div>
        ) : (
          <div className="text-sm text-muted-foreground">Loading...</div>
        )}
      </div>

      <div className="glass p-6 rounded-xl space-y-6">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-lg font-medium">Definition Updates</h3>
            <p className="text-sm text-muted-foreground mt-1">
              POLLEK agent signatures and definitions are updated automatically via dek-bundle-sync.
            </p>
          </div>
          <button
            onClick={checkUpdates}
            disabled={checkingUpdates}
            className="flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 shadow-sm disabled:opacity-50"
          >
            {checkingUpdates ? <Activity className="h-4 w-4 animate-spin" /> : null}
            Check for updates
          </button>
        </div>
        <div className="grid gap-2 text-sm">
          <div className="space-y-1">
            <span className="text-muted-foreground block">
              Current Definition Version
            </span>
            <span className="font-medium bg-secondary/20 text-secondary-foreground px-2 py-1 rounded inline-block">
              {defVersion}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}

function ContractHubStatusCard({
  discovery,
}: {
  discovery: ContractDiscoveryResponse;
}) {
  const hasCloudInterface = Boolean(
    (discovery as any)?.interfaces?.["pollek.cloud.telemetry"],
  );
  return (
    <section className="rounded-lg border bg-card/60 p-4">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0">
          <h4 className="text-sm font-semibold">Contract Hub status</h4>
          <p className="mt-1 max-w-3xl text-sm leading-6 text-muted-foreground">
            The dashboard is using the shared Pollek contract discovery endpoint
            to understand local routes, generated types, and optional cloud sync
            capability. Local observation remains usable even when cloud
            interfaces are not advertised.
          </p>
        </div>
        <span className="inline-flex items-center gap-1 rounded-full bg-emerald-500/10 px-2.5 py-1 text-xs font-medium text-emerald-700">
          <CheckCircle2 className="h-3.5 w-3.5" />
          Local contract reachable
        </span>
      </div>
      <div className="mt-4 grid gap-3 md:grid-cols-3">
        <div className="rounded-md border bg-background/60 p-3">
          <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            <FileCode2 className="h-3.5 w-3.5" />
            Dashboard types
          </div>
          <div className="mt-1 text-sm font-semibold">
            Generated/shared API client
          </div>
          <p className="mt-1 text-xs leading-5 text-muted-foreground">
            UI reads contract-shaped responses instead of separate ad-hoc
            dashboard models.
          </p>
        </div>
        <div className="rounded-md border bg-background/60 p-3">
          <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            <ShieldAlert className="h-3.5 w-3.5" />
            Preferred contract
          </div>
          <div className="mt-1 break-words text-sm font-semibold">
            {discovery.preferred}
          </div>
          <p className="mt-1 text-xs leading-5 text-muted-foreground">
            Schema version {discovery.schema_version}
          </p>
        </div>
        <div className="rounded-md border bg-background/60 p-3">
          <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            <CloudOff className="h-3.5 w-3.5" />
            Cloud sync
          </div>
          <div className="mt-1 text-sm font-semibold">
            {hasCloudInterface ? "Available if enabled" : "Optional / not required"}
          </div>
          <p className="mt-1 text-xs leading-5 text-muted-foreground">
            Local history and Observe views do not require Pollek Cloud.
          </p>
        </div>
      </div>
    </section>
  );
}
