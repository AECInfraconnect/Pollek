import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import {
  Activity,
  BookOpen,
  ChevronDown,
  ChevronUp,
  Cpu,
  Eye,
  FileWarning,
  Network,
  RefreshCw,
  ShieldAlert,
  ShieldCheck,
  Wrench,
} from "lucide-react";
import { MasterDetailLayout } from "../components/master-detail/MasterDetailLayout";
import { useConfirm } from "../components/ui/ConfirmDialog";
import { PageHeader } from "../components/layout/PageHeader";
import {
  DetectionApi,
  type DetectionCoverageResponse,
  type DetectionRuleSummary,
  type ObserveSensor,
} from "../services/api";
import { statusToken, type UiStatus } from "../lib/status";
import { cn } from "../lib/utils";

function labelize(value: string) {
  return value
    .replace(/[_-]+/g, " ")
    .split(" ")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function severityStatus(severity: string): UiStatus {
  if (severity === "critical" || severity === "high") return "failed";
  if (severity === "medium") return "degraded";
  if (severity === "low") return "info";
  return "idle";
}

function sensorStatus(sensor: ObserveSensor): UiStatus {
  if (sensor.status === "ready") return "ok";
  if (sensor.status.includes("not_available")) return "idle";
  if (sensor.can_observe) return "degraded";
  return "info";
}

function hasCurrentObserve(sensor: ObserveSensor) {
  return (
    sensor.achieved_level === "observe_only" ||
    sensor.achieved_level === "enforce"
  );
}

function hasCurrentEnforce(sensor: ObserveSensor) {
  return sensor.achieved_level === "enforce";
}

function frameworkCount(rule: DetectionRuleSummary) {
  return Object.values(rule.maps ?? {}).reduce(
    (total, values) => total + (Array.isArray(values) ? values.length : 0),
    0,
  );
}

function mappedControls(rule: DetectionRuleSummary) {
  return Object.entries(rule.maps ?? {}).flatMap(([framework, controls]) =>
    (controls ?? []).map((control) => ({
      framework,
      control,
    })),
  );
}

function relevantSensors(rule: DetectionRuleSummary, sensors: ObserveSensor[]) {
  const text =
    `${rule.name} ${rule.user_message} ${rule.setup_requirements.join(
      " ",
    )}`.toLowerCase();
  return sensors.filter((sensor) => {
    if (text.includes("file") && sensor.domains.includes("files")) return true;
    if (text.includes("web") && sensor.domains.includes("web")) return true;
    if (text.includes("network") && sensor.domains.includes("network")) {
      return true;
    }
    if (text.includes("command") && sensor.domains.includes("commands")) {
      return true;
    }
    if (text.includes("tool") && sensor.domains.includes("tools")) return true;
    if (text.includes("llm") && sensor.domains.includes("llm_api")) return true;
    if (text.includes("prompt") && sensor.domains.includes("prompts")) {
      return true;
    }
    return sensor.id === "mcp_proxy" || sensor.id === "http_gateway";
  });
}

function SummaryTile({
  icon: Icon,
  label,
  value,
}: {
  icon: typeof Activity;
  label: string;
  value: string | number;
}) {
  return (
    <div className="rounded-lg border bg-card/60 p-4">
      <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
        <Icon className="h-3.5 w-3.5" />
        {label}
      </div>
      <div className="mt-2 text-2xl font-semibold">{value}</div>
    </div>
  );
}

function RuleMasterCard({
  rule,
  selected,
}: {
  rule: DetectionRuleSummary;
  selected: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  const token = statusToken(severityStatus(rule.severity));
  const longSummary =
    rule.user_message.length > 120 || rule.setup_requirements.length > 1;

  return (
    <section
      className={cn(
        "group h-full cursor-pointer rounded-lg border bg-card/70 p-4 transition-all hover:border-primary/40 hover:bg-primary/5",
        selected && "border-primary/60 bg-primary/10",
      )}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h3 className="truncate text-sm font-semibold">{rule.name}</h3>
          <p className="mt-1 text-xs text-muted-foreground">
            {rule.id} - {labelize(rule.detect_type)}
          </p>
        </div>
        <span
          className={cn(
            "shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium",
            token.bg,
            token.text,
          )}
        >
          {labelize(rule.severity)}
        </span>
      </div>

      <p
        className={cn(
          "mt-3 text-xs leading-5 text-muted-foreground",
          !expanded && "line-clamp-2",
        )}
      >
        {rule.user_message}
      </p>

      <div className="mt-3 grid grid-cols-3 gap-2">
        <div className="rounded-md border bg-background/50 p-2">
          <div className="text-[10px] uppercase text-muted-foreground">
            Maps
          </div>
          <div className="mt-1 text-sm font-semibold">
            {frameworkCount(rule)}
          </div>
        </div>
        <div className="rounded-md border bg-background/50 p-2">
          <div className="text-[10px] uppercase text-muted-foreground">
            Default
          </div>
          <div className="mt-1 text-sm font-semibold">
            {labelize(rule.default_response)}
          </div>
        </div>
        <div className="rounded-md border bg-background/50 p-2">
          <div className="text-[10px] uppercase text-muted-foreground">
            Stop
          </div>
          <div className="mt-1 text-sm font-semibold">
            {rule.can_stop_next_time ? "Possible" : "Observe"}
          </div>
        </div>
      </div>

      {expanded && (
        <div className="mt-3 rounded-md border bg-background/50 p-3 text-xs text-muted-foreground">
          <div className="font-medium text-foreground">Setup needs</div>
          <ul className="mt-2 space-y-1">
            {rule.setup_requirements.map((item) => (
              <li key={item}>{item}</li>
            ))}
            {rule.setup_requirements.length === 0 && (
              <li>No extra setup requirement is declared for this rule.</li>
            )}
          </ul>
        </div>
      )}

      {longSummary && (
        <button
          type="button"
          aria-expanded={expanded}
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            setExpanded((current) => !current);
          }}
          className="mt-3 inline-flex h-7 items-center gap-1 rounded-md border bg-background px-2 text-[11px] font-medium text-muted-foreground hover:bg-muted hover:text-foreground"
        >
          {expanded ? (
            <>
              Show less <ChevronUp className="h-3 w-3" />
            </>
          ) : (
            <>
              Show more <ChevronDown className="h-3 w-3" />
            </>
          )}
        </button>
      )}
    </section>
  );
}

function SensorCard({
  sensor,
  onPreflight,
  onSetup,
}: {
  sensor: ObserveSensor;
  onPreflight: (sensor: ObserveSensor) => void;
  onSetup: (sensor: ObserveSensor) => void;
}) {
  const token = statusToken(sensorStatus(sensor));
  return (
    <div className="rounded-lg border bg-background/60 p-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h4 className="truncate text-sm font-semibold">{sensor.title}</h4>
          <p className="mt-1 text-xs leading-5 text-muted-foreground">
            {sensor.reason}
          </p>
        </div>
        <span
          className={cn(
            "shrink-0 rounded-full px-2 py-0.5 text-[11px]",
            token.bg,
            token.text,
          )}
        >
          {labelize(sensor.status)}
        </span>
      </div>
      <div className="mt-3 flex flex-wrap gap-1.5">
        {sensor.domains.map((domain) => (
          <span
            key={domain}
            className="rounded border bg-card/60 px-2 py-0.5 text-[11px] text-muted-foreground"
          >
            {labelize(domain)}
          </span>
        ))}
      </div>
      <div className="mt-3 grid grid-cols-2 gap-2 text-xs text-muted-foreground">
        <div>
          <span className="block text-foreground">
            {hasCurrentObserve(sensor) ? "Active" : "Not yet"}
          </span>
          Current observe
        </div>
        <div>
          <span className="block text-foreground">
            {hasCurrentEnforce(sensor) ? "Active" : "Not yet"}
          </span>
          Current enforce
        </div>
      </div>
      <div className="mt-3 grid grid-cols-2 gap-2 text-xs text-muted-foreground">
        <div>
          <span className="block text-foreground">
            {labelize(sensor.achieved_level ?? "none")}
          </span>
          Achieved
        </div>
        <div>
          <span className="block text-foreground">
            {labelize(sensor.achievable_level ?? "unknown")}
          </span>
          Achievable
        </div>
      </div>
      <p className="mt-3 rounded-md border bg-card/60 p-2 text-xs leading-5 text-muted-foreground">
        {sensor.deterministic_decision ??
          "Pollek combines this source with the broader evidence matrix and falls back when a sensor is unavailable."}
      </p>
      {(sensor.missing_requirements?.length ?? 0) > 0 && (
        <div className="mt-3 rounded-md border bg-card/60 p-2">
          <div className="text-xs font-medium">Missing gate</div>
          <ul className="mt-1 space-y-1 text-xs text-muted-foreground">
            {sensor.missing_requirements?.slice(0, 2).map((item, index) => (
              <li key={`${sensor.id}-missing-${index}`}>
                {String(item.description ?? item.code ?? "OS requirement")}
              </li>
            ))}
          </ul>
        </div>
      )}
      <div className="mt-3 flex flex-wrap gap-2">
        <button
          type="button"
          onClick={() => onPreflight(sensor)}
          className="inline-flex h-8 items-center gap-1.5 rounded-md border px-2.5 text-xs hover:bg-muted"
        >
          <RefreshCw className="h-3.5 w-3.5" />
          Preflight
        </button>
        <button
          type="button"
          onClick={() => onSetup(sensor)}
          className="inline-flex h-8 items-center gap-1.5 rounded-md bg-primary px-2.5 text-xs font-medium text-primary-foreground hover:bg-primary/90"
        >
          <Wrench className="h-3.5 w-3.5" />
          Start setup
        </button>
      </div>
    </div>
  );
}

function RuleDetail({
  rule,
  coverage,
  onReload,
}: {
  rule: DetectionRuleSummary;
  coverage: DetectionCoverageResponse;
  onReload: () => void;
}) {
  const { confirm } = useConfirm();
  const sensors = relevantSensors(rule, coverage.sensors);
  const token = statusToken(severityStatus(rule.severity));
  const controls = mappedControls(rule);

  const preflight = async (sensor: ObserveSensor) => {
    try {
      await DetectionApi.preflightSensor(sensor.id);
      toast.success(`${sensor.title} preflight complete`);
      onReload();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Preflight failed");
    }
  };

  const setup = async (sensor: ObserveSensor) => {
    const ok = await confirm({
      title: `Allow ${sensor.title}?`,
      description: `${sensor.setup_action} Pollek will store local setup metadata only; raw prompts, responses, email bodies, and file contents are not stored by this setup flow.`,
      confirmText: "Allow setup",
      cancelText: "Not now",
    });
    if (!ok) return;

    try {
      await DetectionApi.consentSensor(sensor.id, true);
      await DetectionApi.requestSensorInstall(
        sensor.id,
        sensor.can_enforce ? "enforce" : "observe",
      );
      toast.success(`${sensor.title} setup request recorded`);
      onReload();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Setup failed");
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Detection Rule
          </div>
          <h2 className="mt-1 text-2xl font-bold tracking-tight">
            {rule.name}
          </h2>
          <p className="mt-1 text-sm text-muted-foreground">{rule.id}</p>
        </div>
        <span
          className={cn(
            "rounded-full px-3 py-1 text-xs font-medium",
            token.bg,
            token.text,
          )}
        >
          {labelize(rule.severity)}
        </span>
      </div>

      <div className="grid gap-4 xl:grid-cols-[0.9fr_1.2fr_0.9fr]">
        <section className="rounded-lg border bg-card/60 p-4">
          <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            <FileWarning className="h-4 w-4 text-primary" />
            Rule Summary
          </div>
          <dl className="mt-4 space-y-3 text-sm">
            <div className="flex justify-between gap-3">
              <dt className="text-muted-foreground">Detect type</dt>
              <dd className="font-medium">{labelize(rule.detect_type)}</dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt className="text-muted-foreground">Confidence</dt>
              <dd className="font-medium">{labelize(rule.confidence)}</dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt className="text-muted-foreground">Maturity</dt>
              <dd className="font-medium">{labelize(rule.maturity)}</dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt className="text-muted-foreground">Default action</dt>
              <dd className="font-medium">{labelize(rule.default_response)}</dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt className="text-muted-foreground">If capable</dt>
              <dd className="font-medium">
                {rule.enforce_if_capable
                  ? labelize(rule.enforce_if_capable)
                  : "Observe only"}
              </dd>
            </div>
          </dl>
          <p className="mt-4 rounded-lg border bg-background/60 p-3 text-xs leading-5 text-muted-foreground">
            {rule.privacy_note}
          </p>
        </section>

        <section className="rounded-lg border bg-card/60 p-4">
          <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            <Activity className="h-4 w-4 text-primary" />
            Detection Workspace
          </div>
          <div className="mt-4 rounded-lg border bg-background/60 p-4">
            <h3 className="text-sm font-semibold">What Pollek looks for</h3>
            <p className="mt-2 text-sm leading-6 text-muted-foreground">
              {rule.user_message}
            </p>
          </div>
          <div className="mt-3 rounded-lg border bg-background/60 p-4">
            <h3 className="text-sm font-semibold">Setup requirements</h3>
            <ul className="mt-2 space-y-2 text-sm leading-6 text-muted-foreground">
              {rule.setup_requirements.map((item) => (
                <li key={item}>{item}</li>
              ))}
              {rule.setup_requirements.length === 0 && (
                <li>This rule can run from existing normalized metadata.</li>
              )}
            </ul>
          </div>
          <div className="mt-3 grid gap-2 sm:grid-cols-2">
            {controls.map((item) => (
              <div
                key={`${item.framework}-${item.control}`}
                className="rounded-md border bg-background/60 p-3 text-xs"
              >
                <div className="font-medium">{item.control}</div>
                <div className="mt-1 text-muted-foreground">
                  {labelize(item.framework)}
                </div>
              </div>
            ))}
          </div>
        </section>

        <section className="rounded-lg border bg-card/60 p-4">
          <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            <Cpu className="h-4 w-4 text-primary" />
            Sensors And Setup
          </div>
          <div className="mt-4 space-y-3">
            {sensors.map((sensor) => (
              <SensorCard
                key={sensor.id}
                sensor={sensor}
                onPreflight={preflight}
                onSetup={setup}
              />
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}

export function DetectionCoveragePage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedId = searchParams.get("id") ?? undefined;
  const [coverage, setCoverage] = useState<DetectionCoverageResponse | null>(
    null,
  );
  const [loading, setLoading] = useState(true);

  const load = async () => {
    setLoading(true);
    try {
      setCoverage(await DetectionApi.coverage());
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : "Detection coverage failed to load",
      );
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  const counts = useMemo(() => {
    const sensors = coverage?.sensors ?? [];
    return {
      rules: coverage?.rule_count ?? 0,
      ready: sensors.filter((sensor) => sensor.status === "ready").length,
      observe: sensors.filter(hasCurrentObserve).length,
      enforce: sensors.filter(hasCurrentEnforce).length,
    };
  }, [coverage]);

  const handleSelect = (id: string) => {
    if (id) setSearchParams({ id });
    else setSearchParams({});
  };

  return (
    <div className="space-y-5">
      {!selectedId && (
        <>
          <PageHeader
            title="Observe Coverage"
            subtitle="What Pollek can watch, what it can stop, and what still needs your approval — across detection rules and device sensors."
            icon={ShieldAlert}
            actions={
              <button
                type="button"
                onClick={() => void load()}
                disabled={loading}
                className="inline-flex h-9 items-center gap-2 rounded-md border bg-background px-3 text-sm hover:bg-muted disabled:opacity-60"
              >
                <RefreshCw
                  className={cn("h-4 w-4", loading && "animate-spin")}
                />
                Refresh
              </button>
            }
          />

          <section className="grid gap-3 md:grid-cols-4">
            <SummaryTile
              icon={ShieldCheck}
              label="Rules"
              value={counts.rules}
            />
            <SummaryTile
              icon={Cpu}
              label="Ready sensors"
              value={counts.ready}
            />
            <SummaryTile
              icon={Eye}
              label="Observe active"
              value={counts.observe}
            />
            <SummaryTile
              icon={Network}
              label="Enforce active"
              value={counts.enforce}
            />
          </section>

          {coverage && (
            <section className="rounded-lg border bg-card/60 p-4">
              <div className="flex flex-col gap-3 xl:flex-row xl:items-start xl:justify-between">
                <div>
                  <h3 className="text-sm font-semibold">
                    Detection pack integrity
                  </h3>
                  <p className="mt-1 text-xs leading-5 text-muted-foreground">
                    {coverage.pack_id} {coverage.pack_version} - manifest{" "}
                    {coverage.manifest_integrity}. Rules are mapped to OWASP,
                    NIST, ATT&CK, and ATLAS where applicable.
                  </p>
                </div>
                <span className="rounded-full border bg-background px-2.5 py-1 text-xs text-muted-foreground">
                  {coverage.generated_at}
                </span>
              </div>
              <div className="mt-3 grid gap-2 lg:grid-cols-2">
                <div className="rounded-md border bg-background/60 p-3">
                  <div className="text-xs font-medium">Privacy guardrails</div>
                  <ul className="mt-2 space-y-1 text-xs text-muted-foreground">
                    {coverage.privacy_guards.slice(0, 3).map((item) => (
                      <li key={item}>{item}</li>
                    ))}
                  </ul>
                </div>
                <div className="rounded-md border bg-background/60 p-3">
                  <div className="text-xs font-medium">Real-world limits</div>
                  <ul className="mt-2 space-y-1 text-xs text-muted-foreground">
                    {coverage.limitations.slice(0, 3).map((item) => (
                      <li key={item}>{item}</li>
                    ))}
                  </ul>
                </div>
              </div>
            </section>
          )}

          {coverage && (
            <section className="rounded-lg border bg-card/60 p-4">
              <div className="mb-3 flex items-center gap-2 text-sm font-semibold">
                <BookOpen className="h-4 w-4 text-primary" />
                Research basis
              </div>
              <div className="grid gap-2 md:grid-cols-2">
                {coverage.research_basis.map((item) => (
                  <a
                    key={item.framework}
                    href={item.source}
                    target="_blank"
                    rel="noreferrer"
                    className="rounded-md border bg-background/60 p-3 text-sm hover:border-primary/40 hover:bg-primary/5"
                  >
                    <div className="font-medium">{item.framework}</div>
                    <p className="mt-1 text-xs leading-5 text-muted-foreground">
                      {item.implementation_use}
                    </p>
                  </a>
                ))}
              </div>
            </section>
          )}
        </>
      )}

      <MasterDetailLayout
        items={coverage?.rules ?? []}
        selectedId={selectedId}
        onSelect={handleSelect}
        idSelector={(rule) => rule.id}
        loading={loading && !coverage}
        masterLayout="grid"
        masterListClassName="grid gap-4 lg:grid-cols-2 2xl:grid-cols-3"
        detailBackLabel="Back to all detection rules"
        emptyState={
          <div className="rounded-lg border border-dashed p-8 text-center">
            <ShieldAlert className="mx-auto h-8 w-8 text-muted-foreground/60" />
            <p className="mt-3 text-sm font-medium">
              No detection rules loaded
            </p>
            <p className="mx-auto mt-2 max-w-md text-sm leading-6 text-muted-foreground">
              Check the local detection pack under contracts/detections or
              restart the Local Control Plane after updating the repository.
            </p>
          </div>
        }
        renderCard={(rule, selected) => (
          <RuleMasterCard rule={rule} selected={selected} />
        )}
        renderDetail={(rule) =>
          coverage ? (
            <RuleDetail rule={rule} coverage={coverage} onReload={load} />
          ) : null
        }
      />
    </div>
  );
}
