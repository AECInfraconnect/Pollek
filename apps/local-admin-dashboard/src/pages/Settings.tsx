import { useState, useEffect } from "react";
import { switchProfile, defaultClient } from "../services/api";
import type { ContractDiscoveryResponse } from "../services/api";
import { PdpRuntimeRouting } from "../components/pdp/PdpRuntimeRouting";

export function Settings() {
  const [profile, setProfile] = useState<"local" | "mock-cloud">("local");
  const [discovery, setDiscovery] = useState<ContractDiscoveryResponse | null>(
    null,
  );
  const [discoveryError, setDiscoveryError] = useState<string | null>(null);

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

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">Settings</h2>
          <p className="text-muted-foreground">
            Configure local control plane settings and synchronization profiles.
          </p>
        </div>
      </div>

      <div className="glass p-6 rounded-xl space-y-6">
        <h3 className="text-lg font-medium">Control Plane Profile</h3>

        <div className="space-y-4 max-w-md">
          <div className="grid gap-2">
            <label className="text-sm font-medium">Active Profile</label>
            <select
              value={profile}
              onChange={(e) => handleProfileChange(e.target.value as any)}
              className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
            >
              <option value="local">
                Local Control Plane (127.0.0.1:43890)
              </option>
              <option value="mock-cloud">
                Mock Pollen Cloud (127.0.0.1:43891)
              </option>
            </select>
          </div>
          <div className="grid gap-2">
            <label className="text-sm font-medium">API Endpoint</label>
            <input
              type="text"
              className="flex h-10 w-full rounded-md border border-input bg-muted/50 px-3 py-2 text-sm text-muted-foreground"
              value={
                profile === "mock-cloud"
                  ? "http://localhost:43891"
                  : "http://localhost:43890"
              }
              disabled
            />
          </div>
          <div className="grid gap-2">
            <label className="text-sm font-medium">Mock Role</label>
            <input
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

      <PdpRuntimeRouting />
    </div>
  );
}
