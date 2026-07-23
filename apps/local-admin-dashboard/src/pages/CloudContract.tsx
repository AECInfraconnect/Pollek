import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import {
  BadgeCheck,
  Boxes,
  Cloud,
  Cpu,
  RefreshCw,
  ShieldCheck,
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
  ContractApi,
  type BundleCompatibility,
  type CompatibilityStatus,
  type CompatibilityVerdict,
  type ContractAdapterInfo,
  type ContractAdaptationResult,
  type DekContract,
} from "../services/api";
import type { UiStatus } from "../lib/status";
import { cn } from "../lib/utils";

function statusUi(status: CompatibilityStatus): UiStatus {
  if (status === "compatible") return "ok";
  if (status === "needs_upgrade") return "degraded";
  return "failed";
}

function statusLabel(status: CompatibilityStatus): string {
  if (status === "compatible") return "Compatible";
  if (status === "needs_upgrade") return "Needs upgrade";
  return "Unsupported";
}

const CSV = (s: string): string[] =>
  s
    .split(",")
    .map((x) => x.trim())
    .filter(Boolean);

export function CloudContract() {
  const [contract, setContract] = useState<DekContract | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Evaluator form state.
  const [minVersion, setMinVersion] = useState("1.0.0-beta.6");
  const [reqPeps, setReqPeps] = useState("mcp_proxy");
  const [reqLinux, setReqLinux] = useState("");
  const [verdict, setVerdict] = useState<CompatibilityVerdict | null>(null);
  const [evaluating, setEvaluating] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await ContractApi.get();
      setContract(res.contract);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const evaluate = useCallback(async () => {
    setEvaluating(true);
    try {
      const compat: BundleCompatibility = {
        min_dek_version: minVersion.trim() || "0.0.0",
        required_crates: [],
        required_pep_types: CSV(reqPeps),
        required_os_modules: {
          linux: CSV(reqLinux),
          windows: [],
          macos: [],
        },
      };
      const res = await ContractApi.evaluate(compat);
      setVerdict(res.verdict);
      toast.success(`Verdict: ${statusLabel(res.verdict.status)}`);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setEvaluating(false);
    }
  }, [minVersion, reqPeps, reqLinux]);

  return (
    <div className="space-y-6">
      <PageHeader
        title="Cloud Contract"
        subtitle="What this DEK can run, and whether a Pollek Cloud bundle is safe to activate here. This is how a fleet of DEKs on different versions each gets the right bundle."
        icon={Cloud}
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
            Could not load contract: {error}
          </CardContent>
        </Card>
      )}

      {contract && (
        <Card>
          <CardHeader className="flex flex-row items-center gap-3">
            <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10 text-primary">
              <BadgeCheck className="h-5 w-5" />
            </span>
            <div>
              <CardTitle className="text-base">This DEK's contract</CardTitle>
              <p className="text-xs text-muted-foreground">
                Self-reported from the running binary — no hardcoded version.
              </p>
            </div>
          </CardHeader>
          <CardContent className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            <Field icon={Cpu} label="DEK version" value={contract.dek_version} />
            <Field
              icon={ShieldCheck}
              label="Contract generation"
              value={contract.contract_version}
            />
            <Field
              icon={Boxes}
              label="Platform"
              value={contract.platform}
            />
            <Field
              icon={Boxes}
              label="Bundle API"
              value={contract.supported_bundle_api_versions.join(", ")}
            />
            <div className="sm:col-span-2">
              <p className="mb-1 text-xs text-muted-foreground">
                Available PEP types
              </p>
              <div className="flex flex-wrap gap-1.5">
                {contract.available_pep_types.map((p) => (
                  <Badge key={p} variant="info">
                    {p}
                  </Badge>
                ))}
              </div>
            </div>
            <div className="sm:col-span-2">
              <p className="mb-1 text-xs text-muted-foreground">
                OS enforcement modules ({contract.platform})
              </p>
              <div className="flex flex-wrap gap-1.5">
                {(contract.os_modules[
                  contract.platform as keyof typeof contract.os_modules
                ] ?? []).length === 0 ? (
                  <span className="text-xs text-muted-foreground">
                    none available on this host
                  </span>
                ) : (
                  (
                    contract.os_modules[
                      contract.platform as keyof typeof contract.os_modules
                    ] ?? []
                  ).map((m) => (
                    <Badge key={m} variant="ok">
                      {m}
                    </Badge>
                  ))
                )}
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="text-base">
            Evaluate a bundle's compatibility
          </CardTitle>
          <p className="text-xs text-muted-foreground">
            Enter a bundle's requirements to see whether this DEK could activate
            it — the same check Cloud runs to pick the right bundle per DEK.
          </p>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-3 sm:grid-cols-3">
            <label className="text-sm">
              <span className="mb-1 block text-xs text-muted-foreground">
                Minimum DEK version
              </span>
              <input
                value={minVersion}
                onChange={(e) => setMinVersion(e.target.value)}
                className="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm"
                placeholder="1.0.0-beta.6"
              />
            </label>
            <label className="text-sm">
              <span className="mb-1 block text-xs text-muted-foreground">
                Required PEP types (comma-sep)
              </span>
              <input
                value={reqPeps}
                onChange={(e) => setReqPeps(e.target.value)}
                className="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm"
                placeholder="mcp_proxy, linux_ebpf"
              />
            </label>
            <label className="text-sm">
              <span className="mb-1 block text-xs text-muted-foreground">
                Required Linux OS modules (comma-sep)
              </span>
              <input
                value={reqLinux}
                onChange={(e) => setReqLinux(e.target.value)}
                className="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm"
                placeholder="ebpfd.v1"
              />
            </label>
          </div>
          <button
            type="button"
            onClick={() => void evaluate()}
            disabled={evaluating}
            className={cn(
              "inline-flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground",
              "hover:bg-primary/90 disabled:opacity-60",
            )}
          >
            {evaluating ? "Evaluating…" : "Evaluate"}
          </button>

          {verdict && (
            <div className="rounded-lg border border-border p-4">
              <div className="mb-2 flex items-center gap-3">
                <Badge variant={statusUi(verdict.status)}>
                  {statusLabel(verdict.status)}
                </Badge>
                <span className="text-xs text-muted-foreground">
                  DEK {verdict.dek_version} vs min {verdict.min_dek_version}
                </span>
              </div>
              <ul className="list-inside list-disc space-y-1 text-sm">
                {verdict.reasons.map((r, i) => (
                  <li key={i}>{r}</li>
                ))}
              </ul>
            </div>
          )}
        </CardContent>
      </Card>

      <ContractAdapterCard />
    </div>
  );
}

function ContractAdapterCard() {
  const [info, setInfo] = useState<ContractAdapterInfo | null>(null);
  const [draft, setDraft] = useState(
    JSON.stringify(
      {
        metadata: { bundle_id: "cloud-bundle-legacy" },
        compatibility: { required_pep_types: ["mcp_proxy"] },
      },
      null,
      2,
    ),
  );
  const [result, setResult] = useState<ContractAdaptationResult | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    ContractApi.adapterInfo()
      .then(setInfo)
      .catch(() => setInfo({ loaded: false }));
  }, []);

  const adapt = useCallback(async () => {
    let parsed: unknown;
    try {
      parsed = JSON.parse(draft);
    } catch {
      toast.error("Bundle is not valid JSON");
      return;
    }
    setBusy(true);
    try {
      const res = await ContractApi.adapt(parsed);
      setResult(res);
      toast.success(
        res.adapted ? `Adapted — ${res.changes.length} change(s)` : "No changes needed",
      );
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }, [draft]);

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10 text-primary">
            <Boxes className="h-5 w-5" />
          </span>
          <div>
            <CardTitle className="text-base">Adapt a bundle (WASM)</CardTitle>
            <p className="text-xs text-muted-foreground">
              Migrate a bundle authored for another contract generation into the
              shape this DEK expects — the version-skew bridge, run in a
              sandboxed WASM component.
            </p>
          </div>
        </div>
        {info?.loaded && (
          <Badge variant="ok">
            wasm {info.wasm_sha256?.slice(0, 8)} · {info.wasm_bytes} B
          </Badge>
        )}
      </CardHeader>
      <CardContent className="space-y-3">
        <textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          spellCheck={false}
          className="h-40 w-full rounded-lg border border-border bg-background p-3 font-mono text-xs"
        />
        <button
          type="button"
          onClick={() => void adapt()}
          disabled={busy || !info?.loaded}
          className={cn(
            "inline-flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground",
            "hover:bg-primary/90 disabled:opacity-60",
          )}
        >
          {busy ? "Adapting…" : "Adapt via WASM"}
        </button>

        {result && (
          <div className="space-y-3 rounded-lg border border-border p-4">
            <div className="flex flex-wrap items-center gap-2 text-sm">
              <span className="font-medium">Result</span>
              {result.verdict_before && (
                <Badge variant={statusUi(result.verdict_before.status)}>
                  before: {statusLabel(result.verdict_before.status)}
                </Badge>
              )}
              <span className="text-muted-foreground">→</span>
              {result.verdict_after ? (
                <Badge variant={statusUi(result.verdict_after.status)}>
                  after: {statusLabel(result.verdict_after.status)}
                </Badge>
              ) : (
                <span className="text-xs text-muted-foreground">
                  after: no compatibility block
                </span>
              )}
              <span className="ml-auto text-xs text-muted-foreground">
                {result.changes.length} change(s) · → contract {result.to_contract}
              </span>
            </div>
            {result.changes.length > 0 && (
              <ul className="list-inside list-disc space-y-0.5 text-xs">
                {result.changes.map((c, i) => (
                  <li key={i}>{c}</li>
                ))}
              </ul>
            )}
            <details>
              <summary className="cursor-pointer text-xs text-muted-foreground">
                Migrated bundle
              </summary>
              <pre className="mt-2 max-h-64 overflow-auto rounded bg-muted p-2 text-xs">
                {JSON.stringify(result.migrated_bundle, null, 2)}
              </pre>
            </details>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function Field({
  icon: Icon,
  label,
  value,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  value: string;
}) {
  return (
    <div className="flex items-start gap-2">
      <span className="mt-0.5 flex h-8 w-8 items-center justify-center rounded-lg bg-muted text-muted-foreground">
        <Icon className="h-4 w-4" />
      </span>
      <div>
        <p className="text-xs text-muted-foreground">{label}</p>
        <p className="font-mono text-sm">{value}</p>
      </div>
    </div>
  );
}

export default CloudContract;
