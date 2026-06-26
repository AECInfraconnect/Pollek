import { useState, useEffect } from "react";
import { PdpCloudApi } from "../../services/api";
import type { CloudPdpProfile } from "../../services/api";

export function CloudPdpTab() {
  const [profile, setProfile] = useState<CloudPdpProfile | null>(null);
  const [loading, setLoading] = useState(true);
  const [endpoint, setEndpoint] = useState("");
  const [tenantId, setTenantId] = useState("");
  const [deviceId, setDeviceId] = useState("");

  const reload = async () => {
    setLoading(true);
    try {
      const data = await PdpCloudApi.get();
      setProfile(data);
      window.dispatchEvent(new Event("pollek-cloud-profile-changed"));
    } catch (e) {
      console.error(e);
      setProfile(null);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    reload();
  }, []);

  useEffect(() => {
    setEndpoint(profile?.pdp_endpoint ?? "");
    setTenantId(profile?.tenant_id ?? "");
    setDeviceId(profile?.device_id ?? "");
  }, [profile]);

  const saveAndProbe = async () => {
    const saved = await PdpCloudApi.update({
      ...(profile ?? {}),
      pdp_endpoint: endpoint.trim() || undefined,
      tenant_id: tenantId.trim() || undefined,
      device_id: deviceId.trim() || undefined,
      auth_method: profile?.auth_method ?? "spiffe-oauth-mtls",
      status: "configured",
      manual_override_enabled: profile?.manual_override_enabled ?? false,
    });
    setProfile(saved);
    await PdpCloudApi.probe();
    await reload();
  };

  const handleLogin = async () => {
    try {
      await PdpCloudApi.login();
      reload();
    } catch (e) {
      console.error(e);
    }
  };

  const handleDiscover = async () => {
    try {
      await PdpCloudApi.discover();
      reload();
    } catch (e) {
      console.error(e);
    }
  };

  const handleProbe = async () => {
    try {
      await PdpCloudApi.probe();
      reload();
    } catch (e) {
      console.error(e);
    }
  };

  if (loading) {
    return (
      <div className="text-sm text-muted-foreground p-8 text-center">
        Loading Cloud PDP Profile...
      </div>
    );
  }

  if (!profile || profile.status !== "connected") {
    return (
      <div className="space-y-6">
        <div className="rounded-lg border bg-card/60 p-5">
          <h4 className="text-lg font-medium text-foreground">
            Connect to Pollek Cloud
          </h4>
          <p className="mt-1 max-w-2xl text-sm text-muted-foreground">
            Enterprise Cloud mode unlocks only after this Local Control Plane
            has a Pollek Cloud endpoint and contract discovery probe succeeds.
          </p>
          {profile?.health?.detail && (
            <p className="mt-3 rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-300">
              {String(profile.health.detail)}
            </p>
          )}
          <div className="mt-5 grid gap-4 text-sm">
            <label className="grid gap-1.5">
              <span className="text-xs font-medium text-muted-foreground">
                Pollek Cloud URL
              </span>
              <input
                value={endpoint}
                onChange={(event) => setEndpoint(event.target.value)}
                placeholder="https://cloud.example.com"
                className="h-10 rounded-md border border-input bg-background px-3 text-sm"
              />
            </label>
            <div className="grid gap-4 md:grid-cols-2">
              <label className="grid gap-1.5">
                <span className="text-xs font-medium text-muted-foreground">
                  Tenant ID
                </span>
                <input
                  value={tenantId}
                  onChange={(event) => setTenantId(event.target.value)}
                  placeholder="tenant-id"
                  className="h-10 rounded-md border border-input bg-background px-3 text-sm"
                />
              </label>
              <label className="grid gap-1.5">
                <span className="text-xs font-medium text-muted-foreground">
                  Device ID
                </span>
                <input
                  value={deviceId}
                  onChange={(event) => setDeviceId(event.target.value)}
                  placeholder="local-device"
                  className="h-10 rounded-md border border-input bg-background px-3 text-sm"
                />
              </label>
            </div>
          </div>
          <div className="mt-5 flex flex-wrap gap-2">
            <button
              onClick={saveAndProbe}
              disabled={!endpoint.trim()}
              className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
            >
              Save & Probe
            </button>
            <button
              onClick={handleLogin}
              className="rounded-md border border-border px-4 py-2 text-sm font-medium hover:bg-muted"
            >
              Use configured environment
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between border-b pb-4">
        <div>
          <h3 className="font-medium">Pollek Cloud Connected</h3>
          <p className="text-sm text-muted-foreground">
            This Local Enforcement Kit is enrolled with Pollek Cloud.
          </p>
        </div>
        <div className="flex gap-2">
          <button
            onClick={handleDiscover}
            className="px-3 py-1 bg-secondary text-secondary-foreground rounded text-xs hover:opacity-80"
          >
            Refresh Contract
          </button>
          <button
            onClick={handleProbe}
            className="px-3 py-1 bg-secondary text-secondary-foreground rounded text-xs hover:opacity-80"
          >
            Probe Decision
          </button>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4 text-sm">
        <div className="space-y-1">
          <span className="text-muted-foreground block text-xs">Tenant ID</span>
          <span className="font-medium font-mono">
            {profile.tenant_id ?? "unknown"}
          </span>
        </div>
        <div className="space-y-1">
          <span className="text-muted-foreground block text-xs">Device ID</span>
          <span className="font-medium font-mono">
            {profile.device_id ?? "unknown"}
          </span>
        </div>
        <div className="space-y-1">
          <span className="text-muted-foreground block text-xs">
            Contract Version
          </span>
          <span className="font-medium">
            {profile.contract_version ?? "unknown"}
          </span>
        </div>
        <div className="space-y-1">
          <span className="text-muted-foreground block text-xs">
            Auth Method
          </span>
          <span className="font-medium">
            {profile.auth_method ?? "unknown"}
          </span>
        </div>
        <div className="space-y-1 col-span-2 border-t pt-4">
          <span className="text-muted-foreground block text-xs">
            PDP Endpoint
          </span>
          <span className="font-medium font-mono bg-muted px-2 py-1 rounded inline-block mt-1">
            {profile.pdp_endpoint ?? "not discovered"}
          </span>
        </div>
      </div>
    </div>
  );
}
