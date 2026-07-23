import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import {
  Fingerprint,
  Globe,
  History,
  MonitorSmartphone,
  Package,
  RefreshCw,
  RotateCcw,
  Upload,
} from "lucide-react";
import { PageHeader } from "../components/layout/PageHeader";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "../components/ui/Card";
import { Badge } from "../components/ui/Badge";
import { DefinitionApi, type DefinitionState } from "../services/api";
import { cn } from "../lib/utils";

function sampleDelta(nextVersion: number, baseVersion: number): string {
  return JSON.stringify(
    {
      schema_version: "pollek.def.v4",
      definition_version: nextVersion,
      released_at: new Date().toISOString(),
      min_engine_version: "1.0.0",
      kind: "delta",
      base_version: baseVersion,
      signatures: [
        {
          id: "example.new.agent",
          display_name: "Example New Agent",
          agent_type: "cli_agent",
        },
      ],
      removed_ids: [],
      catalog_hash: "",
      collapse_rules: [],
    },
    null,
    2,
  );
}

export function DefinitionsHotReload() {
  const [state, setState] = useState<DefinitionState | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await DefinitionApi.getState();
      setState(res);
      setDraft((prev) =>
        prev
          ? prev
          : sampleDelta(
              res.current.definition_version + 1,
              res.current.definition_version,
            ),
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const activate = useCallback(async () => {
    let parsed: unknown;
    try {
      parsed = JSON.parse(draft);
    } catch {
      toast.error("Draft is not valid JSON");
      return;
    }
    setBusy(true);
    try {
      const res = await DefinitionApi.activate(parsed);
      if (res.status === "activated") {
        toast.success(
          `Activated — signatures now ${res.current.counts.signatures}`,
        );
      } else {
        toast.error(`Rejected: ${res.reason ?? "unknown"}`);
      }
      await load();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }, [draft, load]);

  const rollback = useCallback(async () => {
    setBusy(true);
    try {
      const res = await DefinitionApi.rollback();
      if (res.status === "rolled_back") {
        toast.success(
          `Rolled back — signatures now ${res.current.counts.signatures}`,
        );
      } else {
        toast.error(`Rollback rejected: ${res.reason ?? "unknown"}`);
      }
      await load();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }, [load]);

  const counts = state?.current.counts;
  const last = state?.last_activation;

  const nextVersionHint = useMemo(
    () =>
      state ? state.current.definition_version + 1 : undefined,
    [state],
  );

  return (
    <div className="space-y-6">
      <PageHeader
        title="Definitions Hot-Reload"
        subtitle="Agent signatures and definitions activate live — no restart. Every apply is snapshotted first, so you can roll back in one click."
        icon={Package}
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
            Could not load definition state: {error}
          </CardContent>
        </Card>
      )}

      {state && (
        <Card>
          <CardHeader className="flex flex-row items-center justify-between gap-3">
            <div className="flex items-center gap-3">
              <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10 text-primary">
                <Fingerprint className="h-5 w-5" />
              </span>
              <div>
                <CardTitle className="text-base">Active definition</CardTitle>
                <p className="text-xs text-muted-foreground">
                  schema {state.current.schema_version}
                </p>
              </div>
            </div>
            <Badge variant="info">v{state.current.definition_version}</Badge>
          </CardHeader>
          <CardContent className="grid gap-4 sm:grid-cols-4">
            <Stat icon={Fingerprint} label="Agent signatures" value={counts?.signatures ?? 0} />
            <Stat icon={Globe} label="Web-AI signatures" value={counts?.web_ai_signatures ?? 0} />
            <Stat
              icon={MonitorSmartphone}
              label="Browser processes"
              value={counts?.browser_processes ?? 0}
            />
            <Stat
              icon={Package}
              label="Installed apps"
              value={counts?.installed_app_signatures ?? 0}
            />
          </CardContent>
        </Card>
      )}

      {last && (
        <Card>
          <CardContent className="flex flex-wrap items-center gap-3 py-4 text-sm">
            <History className="h-4 w-4 text-muted-foreground" />
            <span className="font-medium capitalize">{last.operation}</span>
            <span className="text-muted-foreground">
              v{last.from_version} → v{last.to_version}
            </span>
            {last.activated_at && (
              <span className="text-xs text-muted-foreground">
                {new Date(last.activated_at).toLocaleString()}
              </span>
            )}
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Apply a definition update</CardTitle>
          <p className="text-xs text-muted-foreground">
            Paste a full or delta definition. It must match the schema and use a
            newer version{nextVersionHint ? ` (next: ${nextVersionHint})` : ""}.
          </p>
        </CardHeader>
        <CardContent className="space-y-3">
          <textarea
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            spellCheck={false}
            className="h-64 w-full rounded-lg border border-border bg-background p-3 font-mono text-xs"
          />
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => void activate()}
              disabled={busy}
              className={cn(
                "inline-flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground",
                "hover:bg-primary/90 disabled:opacity-60",
              )}
            >
              <Upload className="h-4 w-4" />
              {busy ? "Working…" : "Activate"}
            </button>
            <button
              type="button"
              onClick={() => void rollback()}
              disabled={busy || !state?.rollback_available}
              title={
                state?.rollback_available
                  ? "Restore the previous definition"
                  : "No snapshot to roll back to yet"
              }
              className={cn(
                "inline-flex items-center gap-2 rounded-lg border border-border px-4 py-2 text-sm font-medium",
                "hover:bg-muted disabled:opacity-50",
              )}
            >
              <RotateCcw className="h-4 w-4" />
              Roll back
            </button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function Stat({
  icon: Icon,
  label,
  value,
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  value: number;
}) {
  return (
    <div className="flex items-start gap-2">
      <span className="mt-0.5 flex h-8 w-8 items-center justify-center rounded-lg bg-muted text-muted-foreground">
        <Icon className="h-4 w-4" />
      </span>
      <div>
        <p className="text-2xl font-semibold tabular-nums">{value}</p>
        <p className="text-xs text-muted-foreground">{label}</p>
      </div>
    </div>
  );
}

export default DefinitionsHotReload;
