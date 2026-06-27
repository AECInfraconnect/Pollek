import { useConfirm } from "../components/ui/ConfirmDialog";
import { toast } from "sonner";
import { useState, useEffect } from "react";
import {
  Activity,
  Database,
  FileKey,
  Info,
  Plus,
  RefreshCw,
  ShieldCheck,
  Wrench,
} from "lucide-react";
import { useNavigate, useSearchParams } from "react-router-dom";
import {
  CapabilityApi,
  LocalObserveApi,
  RegistryApi,
  TelemetryApi,
  type LocalCapabilitySnapshotV2,
  type LocalObserveRefreshResponse,
} from "../services/api";
import type { Resource, ObservedResource } from "../services/api";

export interface UnifiedResource {
  id: string;
  name: string;
  resource_type: string;
  uri: string;
  classification?: string;
  is_registered: boolean;
  is_observed: boolean;
  observed_details?: ObservedResource;
  registered_details?: Resource;
}
type ResourceTraceDetails = Record<string, string | number | boolean | string[] | undefined>;
import { MasterDetailLayout } from "../components/master-detail/MasterDetailLayout";
import { EntityCard } from "../components/master-detail/EntityCard";
import { DetailPane } from "../components/master-detail/DetailPane";
import { EmptyState } from "../components/master-detail/EmptyState";
import type { UiStatus } from "../lib/status";
import { Entity360Layout } from "../features/entity-360/Entity360Layout";
import { useEntity360 } from "../features/entity-graph/useEntity360";

function SummaryMetric({
  label,
  value,
  helper,
}: {
  label: string;
  value: React.ReactNode;
  helper?: string;
}) {
  return (
    <div className="p-4 bg-muted/30 rounded-xl border">
      <span className="text-muted-foreground block mb-1 text-xs">{label}</span>
      <span className="text-sm font-medium break-words">{value}</span>
      {helper && <p className="mt-1 text-xs text-muted-foreground">{helper}</p>}
    </div>
  );
}

function resourceStatus(resource: UnifiedResource): {
  status: UiStatus;
  label: string;
} {
  if (!resource.is_registered) {
    return { status: "idle", label: "Observed" };
  }
  if (resource.classification === "restricted") {
    return { status: "failed", label: "Restricted" };
  }
  if (resource.classification === "confidential") {
    return { status: "degraded", label: "Confidential" };
  }
  return { status: "ok", label: "Protected" };
}

function observedTraceDetails(observed?: ObservedResource): ResourceTraceDetails {
  return ((observed as any)?.details ?? {}) as ResourceTraceDetails;
}

function resourceDisplayNameFromObserved(observed: ObservedResource) {
  const details = observedTraceDetails(observed);
  return (
    details.file_name ||
    details.folder_name ||
    details.db_table ||
    details.db_collection ||
    details.resource_name ||
    observed.target_redacted.split(/[\\/]/).pop() ||
    observed.target_redacted
  ).toString();
}

export function Resources() {
  const { confirm } = useConfirm();
  const navigate = useNavigate();

  const [resources, setResources] = useState<UnifiedResource[]>([]);
  const [loading, setLoading] = useState(true);
  const [observing, setObserving] = useState(false);
  const [observeResult, setObserveResult] =
    useState<LocalObserveRefreshResponse | null>(null);
  const [capabilitySnapshot, setCapabilitySnapshot] =
    useState<LocalCapabilitySnapshotV2 | null>(null);
  const [search, setSearch] = useState("");
  const [scopeFilter, setScopeFilter] = useState<"all" | "local" | "cloud">(
    "all",
  );
  const [kindFilter, setKindFilter] = useState("all");
  const [agentFilter, setAgentFilter] = useState("");
  const [params, setParams] = useSearchParams();
  const selectedId = params.get("selected") ?? undefined;

  const fetchResources = async () => {
    setLoading(true);
    try {
      const [regRes, obsRes] = await Promise.all([
        RegistryApi.listResources(),
        TelemetryApi.listResourceInventory({
          agentId: agentFilter || undefined,
          scope: scopeFilter === "all" ? undefined : scopeFilter,
        }).catch(() => ({ items: [] as ObservedResource[] })),
      ]);

      const unifiedMap = new Map<string, UnifiedResource>();

      for (const r of regRes) {
        unifiedMap.set(r.uri, {
          id: (r as any).resource_id || (r as any).id || r.uri,
          name: r.name,
          resource_type: r.resource_type,
          uri: r.uri,
          classification: r.classification,
          is_registered: true,
          is_observed: false,
          registered_details: r,
        });
      }

      for (const o of obsRes.items || []) {
        const uri = o.target_redacted;
        if (unifiedMap.has(uri)) {
          const existing = unifiedMap.get(uri)!;
          existing.is_observed = true;
          existing.observed_details = o;
        } else {
          unifiedMap.set(uri, {
            id: o.resource_id || uri,
            name: resourceDisplayNameFromObserved(o),
            resource_type: o.kind,
            uri: uri,
            classification: o.classification,
            is_registered: false,
            is_observed: true,
            observed_details: o,
          });
        }
      }

      setResources(
        Array.from(unifiedMap.values()).filter((r) => {
          const details = observedTraceDetails(r.observed_details);
          const haystack =
            `${r.name} ${r.resource_type} ${r.uri} ${r.classification ?? ""} ${Object.values(details).join(" ")}`.toLowerCase();
          const matchesSearch = haystack.includes(search.trim().toLowerCase());
          const matchesKind =
            kindFilter === "all" || r.resource_type === kindFilter;
          return matchesSearch && matchesKind;
        }),
      );
    } catch (err) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchResources();

    const source = new EventSource(TelemetryApi.streamUrl("resources"));
    source.onmessage = (e) => {
      try {
        const data = JSON.parse(e.data);
        if (data.event_type === "resource_access") {
          fetchResources();
        }
      } catch (err) {}
    };

    return () => source.close();
  }, [search, scopeFilter, kindFilter, agentFilter]);

  useEffect(() => {
    let mounted = true;
    CapabilityApi.getSnapshotV2()
      .then((snapshot) => {
        if (mounted) setCapabilitySnapshot(snapshot);
      })
      .catch(() => {});
    return () => {
      mounted = false;
    };
  }, []);

  const select = (id: string) =>
    setParams((p) => {
      p.set("selected", id);
      return p;
    });

  const deleteResource = async (id: string) => {
    if (
      !(await confirm({
        title: "Confirm",
        description:
          "Are you sure you want to delete this resource? Make sure no active policies depend on it.",
        danger: true,
      }))
    )
      return;
    try {
      await RegistryApi.deleteResource(id);
      if (selectedId === id) {
        setParams((p) => {
          p.delete("selected");
          return p;
        });
      }
      fetchResources();
    } catch (err) {
      console.error("Failed to delete resource:", err);
      toast.error("Failed to delete resource");
    }
  };

  const runLocalObserve = async () => {
    setObserving(true);
    try {
      const result = await LocalObserveApi.refresh({ include_estimates: true });
      setObserveResult(result);
      await fetchResources();
      toast.success(
        `Observed ${result.resource_events} resource event(s), ${result.exact_usage_events} exact usage event(s).`,
      );
    } catch (error) {
      console.error(error);
      toast.error(
        error instanceof Error ? error.message : "Local observe refresh failed",
      );
    } finally {
      setObserving(false);
    }
  };

  const protectResource = (resource: UnifiedResource) => {
    const query = new URLSearchParams({
      resource: resource.id,
      resource_uri: resource.uri,
    });
    navigate(`/protect?${query.toString()}`);
  };

  const canEnforce =
    capabilitySnapshot?.control_methods.some(
      (method) =>
        method.status === "available" &&
        (method.max_level === "enforce" || method.max_level === "strict_deny"),
    ) ?? false;
  const setupActions = capabilitySnapshot?.setup_actions ?? [];

  return (
    <div className="p-6 md:p-8 space-y-6">
      <div className="mb-2 rounded-lg border bg-card/60 p-4 shadow-sm">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
          <div className="flex gap-3">
            <div className="flex-shrink-0 rounded-lg bg-primary/10 p-2">
              {canEnforce ? (
                <ShieldCheck className="h-5 w-5 text-emerald-500" />
              ) : (
                <Info className="h-5 w-5 text-primary" aria-hidden="true" />
              )}
            </div>
            <div>
              <h3 className="text-sm font-medium">
                {canEnforce
                  ? "Local observe and enforcement are available on this device."
                  : "Local observe is active; some enforcement paths need setup."}
              </h3>
              <p className="mt-1 text-sm text-muted-foreground">
                Observe Now reads exact usage first from wrapper, proxy,
                browser, and known agent-log telemetry. Estimates are only used
                as labeled fallback when exact usage is unavailable.
              </p>
              <div className="mt-3 flex flex-wrap gap-2 text-xs">
                <span className="rounded-md border bg-background px-2 py-1">
                  {observeResult
                    ? `${observeResult.resource_events} resource events`
                    : "Ready to observe"}
                </span>
                <span className="rounded-md border bg-background px-2 py-1">
                  {observeResult
                    ? `${observeResult.exact_usage_events} exact usage`
                    : "Exact-first"}
                </span>
                {!canEnforce && (
                  <span className="rounded-md border bg-background px-2 py-1">
                    {setupActions.length || "Capability"} setup pending
                  </span>
                )}
              </div>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={runLocalObserve}
              disabled={observing}
              className="inline-flex h-10 items-center rounded-lg bg-primary px-3 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
            >
              <RefreshCw
                className={`mr-2 h-4 w-4 ${observing ? "animate-spin" : ""}`}
              />
              Observe Now
            </button>
            <button
              type="button"
              onClick={() => navigate("/capabilities")}
              className="inline-flex h-10 items-center rounded-lg border bg-background px-3 text-sm font-medium hover:bg-muted"
            >
              <Wrench className="mr-2 h-4 w-4" />
              Setup
            </button>
          </div>
        </div>
      </div>

      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold tracking-tight">
            Data Resources
          </h2>
          <p className="text-sm text-muted-foreground">
            Manage data boundaries and classifications for registered resources.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <button
            type="button"
            onClick={runLocalObserve}
            disabled={observing}
            className="flex items-center gap-2 rounded-lg border bg-background px-4 py-2 text-sm font-medium hover:bg-muted disabled:pointer-events-none disabled:opacity-50"
          >
            <RefreshCw className={`h-4 w-4 ${observing ? "animate-spin" : ""}`} />
            Observe
          </button>
          <button className="flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 shadow-sm">
            <Plus className="h-4 w-4" />
            Add Resource
          </button>
        </div>
      </div>

      <MasterDetailLayout
        idSelector={(x: UnifiedResource) => x.id}
        items={resources}
        loading={loading}
        selectedId={selectedId}
        onSelect={select}
        toolbar={
          <div className="flex items-center gap-2 mb-4">
            <input
              type="text"
              placeholder="Search resources..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="px-3 py-1.5 text-sm rounded-md border bg-background"
            />
            <select
              value={scopeFilter}
              onChange={(e) =>
                setScopeFilter(e.target.value as "all" | "local" | "cloud")
              }
              className="px-3 py-1.5 text-sm rounded-md border bg-background"
            >
              <option value="all">All scopes</option>
              <option value="local">Local</option>
              <option value="cloud">Cloud</option>
            </select>
            <select
              value={kindFilter}
              onChange={(e) => setKindFilter(e.target.value)}
              className="px-3 py-1.5 text-sm rounded-md border bg-background"
            >
              <option value="all">All kinds</option>
              <option value="file">File</option>
              <option value="folder">Folder</option>
              <option value="database_local">Local DB</option>
              <option value="api">API</option>
              <option value="cloud_drive">Cloud drive</option>
              <option value="email">Email</option>
              <option value="saas">SaaS</option>
            </select>
            <input
              type="text"
              placeholder="Agent ID"
              value={agentFilter}
              onChange={(e) => setAgentFilter(e.target.value)}
              className="px-3 py-1.5 text-sm rounded-md border bg-background"
            />
          </div>
        }
        emptyState={
          <EmptyState
            icon={Database}
            title="No resources observed"
            description="Run scan and protect, then agent file, cloud, API, or database access will appear here."
            actionLabel="Add Resource"
          />
        }
        renderCard={(r: UnifiedResource, selected) => {
          const { status, label } = resourceStatus(r);
          const observed = r.observed_details;

          return (
            <EntityCard
              title={r.name}
              subtitle={r.resource_type}
              summary={
                observed
                  ? `${observed.access_count} observed access event(s) from ${observed.agents.length || 0} agent(s). ${observed.governed ? "Policy governed." : "No active policy yet."}`
                  : `${r.classification || "Unclassified"} resource registered for policy targeting.`
              }
              icon={Database}
              status={status}
              statusLabel={label}
              meta={[
                { label: "URI", value: r.uri },
                {
                  label: "Scope",
                  value:
                    observed?.scope ??
                    (r.uri.startsWith("http") ? "cloud" : "local"),
                },
              ]}
              actions={[
                {
                  label: r.is_registered ? "Policy" : "Protect",
                  primary: !r.is_registered,
                  onClick: () => protectResource(r),
                },
              ]}
              selected={selected}
            />
          );
        }}
        renderDetail={(r: UnifiedResource) => (
          <Resource360Detail
            resource={r}
            onDelete={() => deleteResource(r.id)}
            onProtect={() => protectResource(r)}
          />
        )}
      />
    </div>
  );
}

function ResourceFriendlyOverview({ resource }: { resource: UnifiedResource }) {
  const details = observedTraceDetails(resource.observed_details);
  const primaryObject =
    details.file_name ||
    details.folder_name ||
    details.db_table ||
    details.db_collection ||
    details.resource_name;
  return (
    <div className="space-y-5">
      <div className="grid grid-cols-1 gap-4 text-sm md:grid-cols-2">
        <SummaryMetric
          label="What POLLEK saw"
          value={resource.uri}
          helper={`${resource.resource_type} - ${
            resource.is_registered ? "registered" : "observed only"
          }`}
        />
        <SummaryMetric
          label="Sensitivity"
          value={resource.classification || "Unclassified"}
          helper="Used for policy suggestions and default guardrails."
        />
        {primaryObject && (
          <SummaryMetric
            label="Exact object"
            value={primaryObject}
            helper={
              details.trace_granularity
                ? `${details.trace_granularity}`.replace(/_/g, " ")
                : "Object name captured from local telemetry."
            }
          />
        )}
        {(details.folder_path || details.host || details.db_namespace) && (
          <SummaryMetric
            label="Container"
            value={details.folder_path || details.db_namespace || details.host}
            helper="Folder, database namespace, or host associated with this resource."
          />
        )}
        {(details.db_system || details.db_operation) && (
          <SummaryMetric
            label="Database trace"
            value={[details.db_system, details.db_operation]
              .filter(Boolean)
              .join(" / ")}
            helper={
              details.query_fingerprint
                ? `Query fingerprint ${details.query_fingerprint}`
                : "Table-level detail when available from DB logs or hooks."
            }
          />
        )}
        {(details.trace_source || details.capture_quality) && (
          <SummaryMetric
            label="Provenance"
            value={details.capture_quality || "observed"}
            helper={
              details.trace_source
                ? `${details.trace_source}`.replace(/_/g, " ")
                : "Telemetry source recorded with this event."
            }
          />
        )}
        {resource.is_observed && resource.observed_details && (
          <>
            <SummaryMetric
              label="Last access"
              value={new Date(
                resource.observed_details.last_access,
              ).toLocaleString()}
              helper={`${resource.observed_details.access_count} total observed access event(s).`}
            />
            <SummaryMetric
              label="Agents touching it"
              value={resource.observed_details.agents.length}
              helper={
                resource.observed_details.agents.join(", ") ||
                "No agent linked yet."
              }
            />
            <SummaryMetric
              label="Access modes"
              value={resource.observed_details.modes.join(", ") || "Unknown"}
              helper="Read/write/connect actions grouped from telemetry."
            />
            <SummaryMetric
              label="Governance"
              value={
                resource.observed_details.governed
                  ? "Policy attached"
                  : "Needs policy"
              }
              helper={
                resource.is_registered
                  ? "Registered resource can be targeted directly."
                  : "Protect will create a policy target for this observed resource."
              }
            />
          </>
        )}
      </div>
    </div>
  );
}

function ResourcePolicyPrompt({
  resource,
  onProtect,
}: {
  resource: UnifiedResource;
  onProtect: () => void;
}) {
  return (
    <div className="flex flex-col items-center justify-center rounded-lg border border-dashed p-8 text-center text-muted-foreground">
      <FileKey className="mb-4 h-8 w-8 opacity-50" />
      <p className="mb-4 text-sm">
        Protect {resource.name} by assigning an access policy to specific agents.
      </p>
      <button
        type="button"
        className="rounded-md bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90"
        onClick={onProtect}
      >
        Create Policy
      </button>
    </div>
  );
}

function Resource360Detail({
  resource,
  onDelete,
  onProtect,
}: {
  resource: UnifiedResource;
  onDelete: () => void;
  onProtect: () => void;
}) {
  const { data } = useEntity360("resource", resource.id);
  const { status, label } = resourceStatus(resource);
  const actions = (
    <>
      <button
        type="button"
        onClick={onProtect}
        className="inline-flex h-9 items-center rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground hover:bg-primary/90"
      >
        {resource.is_registered ? "Apply Policy" : "Protect"}
      </button>
      {resource.is_registered && (
        <button
          type="button"
          onClick={onDelete}
          className="inline-flex h-9 items-center rounded-md border border-red-500/30 bg-red-500/10 px-4 text-sm font-medium text-red-600 hover:bg-red-500/15"
        >
          Delete
        </button>
      )}
    </>
  );

  if (data) {
    return (
      <Entity360Layout
        data={data}
        actions={actions}
        overview={<ResourceFriendlyOverview resource={resource} />}
      />
    );
  }

  return (
    <DetailPane
      title={resource.name}
      subtitle={resource.resource_type}
      status={status}
      statusLabel={label}
      actions={
        [
          {
            label: resource.is_registered ? "Apply Policy" : "Protect",
            primary: true,
            onClick: onProtect,
          },
          resource.is_registered
            ? {
                label: "Delete",
                danger: true,
                onClick: onDelete,
              }
            : undefined,
        ].filter(Boolean) as any
      }
      tabs={[
        {
          id: "overview",
          label: "Overview",
          content: <ResourceFriendlyOverview resource={resource} />,
        },
        {
          id: "access",
          label: "Access Policies",
          content: <ResourcePolicyPrompt resource={resource} onProtect={onProtect} />,
        },
        {
          id: "activity",
          label: "Activity",
          content: <ResourceActivityTimeline resource={resource} />,
        },
      ]}
    />
  );
}

function ResourceActivityTimeline({ resource }: { resource: UnifiedResource }) {
  const [events, setEvents] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let mounted = true;
    setLoading(true);
    TelemetryApi.getObservations({ target: resource.uri }).then((res) => {
      if (mounted) {
        setEvents(res.items || []);
        setLoading(false);
      }
    });
    return () => {
      mounted = false;
    };
  }, [resource.uri]);

  if (loading)
    return (
      <div className="p-8 text-center text-sm text-muted-foreground">
        Loading activity...
      </div>
    );
  if (events.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center p-8 text-center border border-dashed rounded-lg text-muted-foreground">
        <Activity className="h-8 w-8 mb-2 opacity-50" />
        <p className="text-sm">No activity recorded yet.</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {events.map((ev, i) => {
        const payload = ev.payload ?? ev.details ?? ev;
        const details = (payload.details ?? {}) as ResourceTraceDetails;
        const object =
          details.file_name ||
          details.folder_name ||
          details.db_table ||
          details.db_collection ||
          payload.target_redacted ||
          resource.name;
        return (
        <div key={i} className="flex gap-4 p-4 border rounded-lg bg-card">
          <div className="mt-1">
            <Activity className="h-4 w-4 text-primary" />
          </div>
          <div>
            <p className="text-sm font-medium">
              {object} by {payload.agent_id || "Unknown agent"}
            </p>
            <p className="text-xs text-muted-foreground mt-1">
              Mode: {payload.mode || "read"} -{" "}
              {new Date(payload.observed_at || ev.timestamp).toLocaleString()}
            </p>
            {(details.capture_quality || details.trace_granularity) && (
              <p className="mt-1 text-xs text-muted-foreground">
                {details.capture_quality || "observed"} /{" "}
                {details.trace_granularity || "resource"}
              </p>
            )}
          </div>
        </div>
        );
      })}
    </div>
  );
}
