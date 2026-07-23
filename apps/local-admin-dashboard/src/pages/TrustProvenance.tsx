import { useCallback, useEffect, useState } from "react";
import {
  BadgeCheck,
  CircleSlash,
  FileCheck2,
  KeyRound,
  MinusCircle,
  RefreshCw,
  ShieldCheck,
  ShieldX,
} from "lucide-react";
import { PageHeader } from "../components/layout/PageHeader";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "../components/ui/Card";
import { Badge } from "../components/ui/Badge";
import {
  TrustApi,
  type TrustCheck,
  type TrustProvenanceView,
  type TrustVerdict,
} from "../services/api";
import { cn } from "../lib/utils";

const CHECK_LABELS: Record<string, string> = {
  signature: "Signature",
  signer_allowlist: "Signer allowlist",
  tenant_match: "Tenant match",
  generation_monotonicity: "Generation monotonicity",
  artifact_integrity: "Artifact integrity",
  provenance: "Provenance (SLSA)",
  sbom: "SBOM (CycloneDX)",
  test_attestation: "Test attestation",
};

export function TrustProvenance() {
  const [data, setData] = useState<TrustProvenanceView | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setData(await TrustApi.get());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const policy = data?.policy;
  const verdicts = data?.verdicts ?? [];

  return (
    <div className="space-y-6">
      <PageHeader
        title="Trust & Provenance"
        subtitle="The single Trust Policy Gate every bundle must pass before it can activate. Runtime trusts evidence, not location — signature, provenance, SBOM, test-attestation, signer allowlist, revocation, generation monotonicity, and tenant match, all in one choke point."
        icon={ShieldCheck}
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
            Could not load trust state: {error}
          </CardContent>
        </Card>
      )}

      {data && (
        <>
          {/* Active policy + key posture */}
          <div className="grid gap-4 lg:grid-cols-2">
            <Card>
              <CardHeader className="flex flex-row items-center gap-3">
                <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10 text-primary">
                  <FileCheck2 className="h-5 w-5" />
                </span>
                <div>
                  <CardTitle className="text-base">Active trust policy</CardTitle>
                  <p className="text-xs text-muted-foreground">
                    The <code>require_*</code> gate the runtime enforces
                  </p>
                </div>
              </CardHeader>
              <CardContent className="flex flex-wrap gap-1.5">
                <ReqBadge on={policy?.require_signature} label="signature" />
                <ReqBadge on={policy?.require_generation_monotonicity} label="monotonic revision" />
                <ReqBadge on={policy?.require_provenance} label="provenance" />
                <ReqBadge on={policy?.require_sbom} label="SBOM" />
                <ReqBadge on={policy?.require_test_attestation} label="test attestation" />
                {(policy?.min_slsa_level ?? 0) > 0 && (
                  <Badge variant="ok">SLSA ≥ L{policy?.min_slsa_level}</Badge>
                )}
                {(policy?.min_approvers ?? 0) > 0 && (
                  <Badge variant="ok">≥ {policy?.min_approvers} approvers</Badge>
                )}
                {policy?.signer_allowlist && policy.signer_allowlist.length > 0 && (
                  <Badge variant="ok">
                    allowlist: {policy.signer_allowlist.length} key(s)
                  </Badge>
                )}
              </CardContent>
            </Card>

            <Card>
              <CardHeader className="flex flex-row items-center justify-between gap-3">
                <div className="flex items-center gap-3">
                  <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10 text-primary">
                    <KeyRound className="h-5 w-5" />
                  </span>
                  <div>
                    <CardTitle className="text-base">Signing keys</CardTitle>
                    <p className="text-xs text-muted-foreground">
                      Trusted keys backing signature + revocation
                    </p>
                  </div>
                </div>
                {data.keys.provisioned ? (
                  <Badge variant="ok">
                    <ShieldCheck className="mr-1 inline h-3.5 w-3.5" />
                    {data.keys.usable_now} usable
                  </Badge>
                ) : (
                  <Badge variant="degraded">
                    <ShieldX className="mr-1 inline h-3.5 w-3.5" />
                    none provisioned
                  </Badge>
                )}
              </CardHeader>
              <CardContent className="text-sm text-muted-foreground">
                {data.keys.provisioned
                  ? "Cloud-distributed trusted keys are present; the gate verifies detached ed25519 signatures against them and rejects revoked keys."
                  : "No trusted keys yet. Until Cloud provisions the signing key set, every signed bundle fails closed at the signature check — by design."}
              </CardContent>
            </Card>
          </div>

          {/* Per-bundle verdicts */}
          <div>
            <h2 className="mb-3 text-sm font-semibold text-muted-foreground">
              Bundle verdicts ({verdicts.length})
            </h2>
            {verdicts.length === 0 ? (
              <Card>
                <CardContent className="py-8 text-center text-sm text-muted-foreground">
                  No bundle has been through the gate yet. When Cloud publishes a
                  signed bundle, its verdict — and every check it passed or failed —
                  appears here.
                </CardContent>
              </Card>
            ) : (
              <div className="space-y-4">
                {verdicts.map((v) => (
                  <VerdictCard key={`${v.bundle_id}:${v.bundle_revision}`} verdict={v} />
                ))}
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}

function VerdictCard({ verdict }: { verdict: TrustVerdict }) {
  const accepted = verdict.decision === "accept";
  return (
    <Card className={cn(!accepted && "border-destructive/40")}>
      <CardHeader className="flex flex-row items-start justify-between gap-3">
        <div className="flex items-center gap-3">
          <span
            className={cn(
              "flex h-9 w-9 items-center justify-center rounded-lg",
              accepted ? "bg-emerald-500/10 text-emerald-500" : "bg-destructive/10 text-destructive",
            )}
          >
            {accepted ? <BadgeCheck className="h-5 w-5" /> : <CircleSlash className="h-5 w-5" />}
          </span>
          <div>
            <CardTitle className="text-base">{verdict.bundle_id}</CardTitle>
            <p className="font-mono text-xs text-muted-foreground">
              {verdict.bundle_revision}
            </p>
          </div>
        </div>
        <Badge variant={accepted ? "ok" : "failed"}>
          {accepted ? "ACCEPTED" : "QUARANTINED"}
        </Badge>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex flex-wrap gap-x-6 gap-y-1 text-xs text-muted-foreground">
          <span>
            tenant <span className="font-mono text-foreground">{verdict.tenant}</span>
          </span>
          {verdict.signer_key_id && (
            <span>
              signer <span className="font-mono text-foreground">{verdict.signer_key_id}</span>
            </span>
          )}
          <span>
            evaluated{" "}
            <span className="text-foreground">
              {new Date(verdict.evaluated_at_unix * 1000).toLocaleString()}
            </span>
          </span>
        </div>

        <div className="grid gap-1.5 sm:grid-cols-2">
          {verdict.checks.map((c) => (
            <CheckRow key={c.name} check={c} />
          ))}
        </div>

        {!accepted && verdict.failure_classes.length > 0 && (
          <div className="flex flex-wrap gap-1.5 border-t border-border pt-3">
            <span className="text-xs font-medium text-destructive">Failure classes:</span>
            {verdict.failure_classes.map((f) => (
              <Badge key={f} variant="failed">
                {f}
              </Badge>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function CheckRow({ check }: { check: TrustCheck }) {
  const label = CHECK_LABELS[check.name] ?? check.name;
  const icon =
    check.status === "pass" ? (
      <ShieldCheck className="h-4 w-4 text-emerald-500" />
    ) : check.status === "fail" ? (
      <ShieldX className="h-4 w-4 text-destructive" />
    ) : (
      <MinusCircle className="h-4 w-4 text-muted-foreground" />
    );
  return (
    <div className="flex items-start gap-2 rounded-md border border-border/60 px-2.5 py-1.5">
      <span className="mt-0.5 shrink-0">{icon}</span>
      <div className="min-w-0">
        <div className="text-sm font-medium">{label}</div>
        <div className="truncate text-xs text-muted-foreground" title={check.detail}>
          {check.detail}
        </div>
      </div>
    </div>
  );
}

function ReqBadge({ on, label }: { on?: boolean; label: string }) {
  return (
    <Badge variant={on ? "ok" : "idle"}>
      {on ? (
        <ShieldCheck className="mr-1 inline h-3.5 w-3.5" />
      ) : (
        <MinusCircle className="mr-1 inline h-3.5 w-3.5" />
      )}
      {label}
    </Badge>
  );
}

export default TrustProvenance;
