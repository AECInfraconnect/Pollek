import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import {
  Boxes,
  Fingerprint,
  Link2,
  RefreshCw,
  ScanLine,
  Workflow,
} from "lucide-react";
import { PageHeader } from "../components/layout/PageHeader";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "../components/ui/Card";
import { Badge } from "../components/ui/Badge";
import { TechnicalDetails } from "../components/ui/TechnicalDetails";
import {
  CorrelationApi,
  type CorrelationAttribution,
  type CorrelationBasis,
  type SignalCorrelationResponse,
} from "../services/api";
import { cn } from "../lib/utils";
import type { UiStatus } from "../lib/status";

const BASIS_LABEL: Record<CorrelationBasis, string> = {
  pid_and_exe: "PID + executable",
  exe_hash: "Executable hash",
  cgroup: "Control group",
  pid: "PID only",
  process_name_unique: "Process name",
};

/** Higher-confidence bases read as stronger evidence. */
function basisStatus(confidence: number): UiStatus {
  if (confidence >= 90) return "ok";
  if (confidence >= 70) return "info";
  return "degraded";
}

function shortHash(hash: string | null): string {
  if (!hash) return "—";
  return hash.length > 12 ? `${hash.slice(0, 12)}…` : hash;
}

interface AgentGroup {
  agentId: string;
  attributions: CorrelationAttribution[];
  bestConfidence: number;
  exeHash: string | null;
  processNames: string[];
}

export function SignalCorrelation() {
  const [data, setData] = useState<SignalCorrelationResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async (announce = false) => {
    setLoading(true);
    setError(null);
    try {
      const res = await CorrelationApi.get();
      setData(res);
      if (announce) {
        toast.success(
          `Scanned ${res.live_scan.processes_scanned} processes · ${res.live_scan.attributed} attributed`,
        );
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const groups = useMemo<AgentGroup[]>(() => {
    if (!data) return [];
    const bindingByAgent = new Map(
      data.bindings.map((b) => [b.agent_id, b]),
    );
    const byAgent = new Map<string, CorrelationAttribution[]>();
    for (const a of data.live_scan.attributions) {
      const list = byAgent.get(a.agent_id) ?? [];
      list.push(a);
      byAgent.set(a.agent_id, list);
    }
    return Array.from(byAgent.entries())
      .map(([agentId, attributions]) => {
        const binding = bindingByAgent.get(agentId);
        return {
          agentId,
          attributions: attributions.sort((x, y) => y.confidence - x.confidence),
          bestConfidence: attributions.reduce(
            (m, a) => Math.max(m, a.confidence),
            0,
          ),
          exeHash: binding?.exe_path_hash ?? null,
          processNames: binding?.process_names ?? [],
        };
      })
      .sort((x, y) => y.attributions.length - x.attributions.length);
  }, [data]);

  const scan = data?.live_scan;

  return (
    <div className="space-y-6">
      <PageHeader
        title="Signal Correlation"
        subtitle="Which running processes on this device belong to which discovered AI agent — the join that lets Pollek observe and enforce per agent, not just per device."
        icon={Workflow}
        actions={
          <button
            type="button"
            onClick={() => void load(true)}
            disabled={loading}
            className={cn(
              "inline-flex items-center gap-2 rounded-lg border border-border px-3 py-2 text-sm font-medium",
              "hover:bg-muted disabled:opacity-60",
            )}
          >
            <RefreshCw className={cn("h-4 w-4", loading && "animate-spin")} />
            Re-scan
          </button>
        }
      />

      {error && (
        <Card>
          <CardContent className="py-6 text-sm text-destructive">
            Could not load correlation: {error}
          </CardContent>
        </Card>
      )}

      <div className="grid gap-4 sm:grid-cols-3">
        <StatCard
          icon={Boxes}
          label="Agents indexed"
          value={data?.agents_indexed ?? "—"}
          hint="Discovered agents with process identity"
        />
        <StatCard
          icon={ScanLine}
          label="Processes scanned"
          value={scan?.processes_scanned ?? "—"}
          hint="Live snapshot of running processes"
        />
        <StatCard
          icon={Link2}
          label="Attributed"
          value={scan?.attributed ?? "—"}
          hint="Processes joined to a known agent"
        />
      </div>

      {!loading && data && groups.length === 0 && (
        <Card>
          <CardContent className="py-10 text-center text-sm text-muted-foreground">
            No running process matched a discovered agent yet. Run a discovery
            scan first, then re-scan here.
          </CardContent>
        </Card>
      )}

      <div className="space-y-4">
        {groups.map((group) => (
          <Card key={group.agentId}>
            <CardHeader className="flex flex-row items-center justify-between gap-3">
              <div className="flex items-center gap-3">
                <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10 text-primary">
                  <Fingerprint className="h-5 w-5" />
                </span>
                <div>
                  <CardTitle className="text-base font-mono">
                    {group.agentId}
                  </CardTitle>
                  <p className="text-xs text-muted-foreground">
                    {group.attributions.length} live process
                    {group.attributions.length === 1 ? "" : "es"} attributed
                  </p>
                </div>
              </div>
              <Badge variant={basisStatus(group.bestConfidence)}>
                {group.bestConfidence}% confidence
              </Badge>
            </CardHeader>
            <CardContent className="space-y-3">
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="text-left text-xs uppercase tracking-wide text-muted-foreground">
                      <th className="pb-2 pr-4 font-medium">PID</th>
                      <th className="pb-2 pr-4 font-medium">Process</th>
                      <th className="pb-2 pr-4 font-medium">Path</th>
                      <th className="pb-2 pr-4 font-medium">Matched by</th>
                    </tr>
                  </thead>
                  <tbody>
                    {group.attributions.map((a) => (
                      <tr
                        key={a.pid}
                        className="border-t border-border/60 align-top"
                      >
                        <td className="py-2 pr-4 font-mono text-xs">{a.pid}</td>
                        <td className="py-2 pr-4">{a.process_name}</td>
                        <td className="py-2 pr-4 font-mono text-xs text-muted-foreground">
                          {a.exe_path_redacted ?? "—"}
                        </td>
                        <td className="py-2 pr-4">
                          <Badge variant={basisStatus(a.confidence)}>
                            {BASIS_LABEL[a.basis]}
                          </Badge>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>

              <TechnicalDetails
                label="Process identity keys"
                hint="How this agent is fingerprinted"
              >
                <dl className="grid gap-2 text-sm sm:grid-cols-2">
                  <div>
                    <dt className="text-xs text-muted-foreground">
                      Executable hash (sha256)
                    </dt>
                    <dd className="font-mono text-xs" title={group.exeHash ?? ""}>
                      {shortHash(group.exeHash)}
                    </dd>
                  </div>
                  <div>
                    <dt className="text-xs text-muted-foreground">
                      Process names
                    </dt>
                    <dd className="text-xs">
                      {group.processNames.length > 0
                        ? group.processNames.join(", ")
                        : "—"}
                    </dd>
                  </div>
                </dl>
              </TechnicalDetails>
            </CardContent>
          </Card>
        ))}
      </div>

      {data && (
        <p className="text-xs text-muted-foreground">
          Generated {new Date(data.generated_at).toLocaleString()} · schema{" "}
          {data.schema_version}
        </p>
      )}
    </div>
  );
}

function StatCard({
  icon: Icon,
  label,
  value,
  hint,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  value: number | string;
  hint: string;
}) {
  return (
    <Card>
      <CardContent className="flex items-start gap-3 py-4">
        <span className="mt-0.5 flex h-9 w-9 items-center justify-center rounded-lg bg-muted text-muted-foreground">
          <Icon className="h-5 w-5" />
        </span>
        <div>
          <p className="text-2xl font-semibold tabular-nums">{value}</p>
          <p className="text-sm font-medium">{label}</p>
          <p className="text-xs text-muted-foreground">{hint}</p>
        </div>
      </CardContent>
    </Card>
  );
}

export default SignalCorrelation;
