import { useCallback, useEffect, useState } from "react";
import {
  Fingerprint,
  KeyRound,
  RefreshCw,
  ShieldCheck,
  ShieldX,
  UserRound,
} from "lucide-react";
import { PageHeader } from "../components/layout/PageHeader";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "../components/ui/Card";
import { Badge } from "../components/ui/Badge";
import { IdentityApi, type WorkloadIdentity } from "../services/api";
import { cn } from "../lib/utils";

function expiryLabel(seconds?: number, expired?: boolean): string {
  if (expired) return "expired";
  if (seconds == null) return "—";
  const days = Math.floor(seconds / 86400);
  if (days >= 1) return `${days}d left`;
  const hours = Math.floor(seconds / 3600);
  return `${hours}h left`;
}

export function WorkloadIdentity() {
  const [data, setData] = useState<WorkloadIdentity | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setData(await IdentityApi.get());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const wl = data?.workload_identity;
  const tr = data?.transport;
  const ui = data?.user_identity;

  return (
    <div className="space-y-6">
      <PageHeader
        title="Workload Identity"
        subtitle="How this DEK proves itself to Pollek Cloud: a device SVID over mutual TLS (which DEK) plus an OAuth identity (which user). Two planes, both cryptographic."
        icon={Fingerprint}
        actions={
          <button
            type="button"
            onClick={() => void load()}
            disabled={loading}
            className={cn(
              "inline-flex items-center gap-2 rounded-lg border border-border px-3 py-2 text-sm font-medium",
              "hover:bg-muted disabled:opacity-60",
            )}
          >
            <RefreshCw className={cn("h-4 w-4", loading && "animate-spin")} />
            Refresh
          </button>
        }
      />

      {error && (
        <Card>
          <CardContent className="py-6 text-sm text-destructive">
            Could not load identity: {error}
          </CardContent>
        </Card>
      )}

      {data && (
        <div className="grid gap-4 lg:grid-cols-2">
          {/* Device / workload plane */}
          <Card>
            <CardHeader className="flex flex-row items-center justify-between gap-3">
              <div className="flex items-center gap-3">
                <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10 text-primary">
                  <Fingerprint className="h-5 w-5" />
                </span>
                <div>
                  <CardTitle className="text-base">Device identity (SVID)</CardTitle>
                  <p className="text-xs text-muted-foreground">
                    X.509-SVID backing mutual TLS
                  </p>
                </div>
              </div>
              {tr?.mtls_ready ? (
                <Badge variant="ok">
                  <ShieldCheck className="mr-1 inline h-3.5 w-3.5" />
                  mTLS ready
                </Badge>
              ) : (
                <Badge variant="degraded">
                  <ShieldX className="mr-1 inline h-3.5 w-3.5" />
                  not provisioned
                </Badge>
              )}
            </CardHeader>
            <CardContent className="space-y-3">
              {wl?.provisioned && !wl.error ? (
                <>
                  <Row label="SPIFFE ID">
                    <span className="font-mono text-xs">{wl.spiffe_id ?? "—"}</span>
                  </Row>
                  <Row label="Expiry">
                    <Badge variant={wl.expired ? "failed" : "ok"}>
                      {expiryLabel(wl.seconds_until_expiry, wl.expired)}
                    </Badge>
                    {wl.not_after_unix != null && (
                      <span className="ml-2 text-xs text-muted-foreground">
                        {new Date(wl.not_after_unix * 1000).toLocaleString()}
                      </span>
                    )}
                  </Row>
                  <Row label="Subject">
                    <span className="font-mono text-xs">{wl.subject}</span>
                  </Row>
                  <Row label="Serial">
                    <span className="font-mono text-xs">{wl.serial}</span>
                  </Row>
                </>
              ) : (
                <p className="text-sm text-muted-foreground">
                  {wl?.error
                    ? `SVID present but unreadable: ${wl.error}`
                    : "No SVID provisioned yet. The device enrolls with a join token and receives an SVID from Pollek Cloud."}
                </p>
              )}
              <div className="flex flex-wrap gap-1.5 pt-1">
                <MatBadge ok={tr?.svid_present} label="SVID cert" icon={Fingerprint} />
                <MatBadge ok={tr?.private_key_present} label="Private key" icon={KeyRound} />
                <MatBadge ok={tr?.trust_bundle_present} label="Trust bundle" icon={ShieldCheck} />
              </div>
            </CardContent>
          </Card>

          {/* User / OAuth plane */}
          <Card>
            <CardHeader className="flex flex-row items-center gap-3">
              <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10 text-primary">
                <UserRound className="h-5 w-5" />
              </span>
              <div>
                <CardTitle className="text-base">User identity (OAuth)</CardTitle>
                <p className="text-xs text-muted-foreground">
                  OIDC bearer attributing telemetry to a user/tenant
                </p>
              </div>
            </CardHeader>
            <CardContent className="space-y-3">
              <Row label="Configured">
                <Badge variant={ui?.oauth_configured ? "ok" : "degraded"}>
                  {ui?.oauth_configured ? "yes" : "no"}
                </Badge>
              </Row>
              <Row label="OIDC issuer">
                <span className="break-all font-mono text-xs">
                  {ui?.oidc_issuer ?? "—"}
                </span>
              </Row>
              <Row label="Client id">
                <span className="font-mono text-xs">{ui?.oidc_client_id ?? "—"}</span>
              </Row>
              <Row label="Subject">
                <span className="font-mono text-xs">{ui?.auth_subject ?? "—"}</span>
              </Row>
              <Row label="Tenant">
                <span className="font-mono text-xs">{data.tenant_id}</span>
              </Row>
            </CardContent>
          </Card>
        </div>
      )}
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-wrap items-center gap-2 text-sm">
      <span className="w-24 shrink-0 text-xs text-muted-foreground">{label}</span>
      <span className="flex items-center">{children}</span>
    </div>
  );
}

function MatBadge({
  ok,
  label,
  icon: Icon,
}: {
  ok?: boolean;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
}) {
  return (
    <Badge variant={ok ? "ok" : "idle"}>
      <Icon className="mr-1 inline h-3.5 w-3.5" />
      {label}
    </Badge>
  );
}

export default WorkloadIdentity;
