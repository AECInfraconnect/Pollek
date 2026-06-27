import { useState, useEffect } from "react";
import { Plus, UserCircle, Activity, FileKey, Info } from "lucide-react";
import { useSearchParams } from "react-router-dom";
import { RegistryApi } from "../services/api";
import { MasterDetailLayout } from "../components/master-detail/MasterDetailLayout";
import { EntityCard } from "../components/master-detail/EntityCard";
import { DetailPane } from "../components/master-detail/DetailPane";
import { EmptyState } from "../components/master-detail/EmptyState";

import { useConfirm } from "../components/ui/ConfirmDialog";
import { toast } from "sonner";

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

export function Entities() {
  const [items, setItems] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [params, setParams] = useSearchParams();
  const selectedId = params.get("selected") ?? undefined;
  const { confirm } = useConfirm();

  const fetchEntities = () => {
    setLoading(true);
    Promise.all([
      RegistryApi.listEntities(),
      RegistryApi.listDiscoveryCandidates(),
    ])
      .then(([entities, candidates]) => {
        const observableSurfaces = candidates
          .filter(
            (candidate: any) =>
              candidate.labels?.["entity.observe_enforce"] ===
                "observable_surface" &&
              candidate.labels?.["entity.kind"] !== "ai_agent",
          )
          .map((candidate: any) => ({
            meta: {
              status: candidate.status,
              source: "discovery",
              created_at: candidate.first_seen,
            },
            entity_id: `discovery:${candidate.candidate_id}`,
            entity_type:
              candidate.labels?.["entity.kind"] ??
              candidate.inferred_agent_type,
            display_name: candidate.display_name,
            external_ids: [
              {
                provider: "agent_discovery",
                id: candidate.candidate_id,
              },
            ],
            roles: [],
            attributes: {
              discovery_candidate_id: candidate.candidate_id,
              confidence: candidate.confidence,
              risk_score: candidate.risk_score,
              capabilities: candidate.capability_tags ?? [],
              status: candidate.status,
            },
          }));
        setItems([...entities, ...observableSurfaces]);
      })
      .catch((err) => {
        console.error(err);
        toast.error("Failed to load entities");
      })
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    fetchEntities();
  }, []);

  const select = (id: string) =>
    setParams((p) => {
      p.set("selected", id);
      return p;
    });

  const handleDelete = async (id: string) => {
    const isConfirmed = await confirm({
      title: "Delete Entity",
      description:
        "Are you sure you want to delete this entity? This action cannot be undone.",
      confirmText: "Delete",
      danger: true,
    });

    if (isConfirmed) {
      const deletion = id.startsWith("discovery:")
        ? RegistryApi.deleteDiscoveryCandidate(id.slice("discovery:".length))
        : RegistryApi.deleteEntity(id);

      deletion
        .then(() => {
          if (selectedId === id) {
            setParams((p) => {
              p.delete("selected");
              return p;
            });
          }
          toast.success("Entity deleted successfully");
          fetchEntities();
        })
        .catch(() => toast.error("Failed to delete entity"));
    }
  };

  return (
    <div className="p-6 md:p-8 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold tracking-tight">Entities</h2>
          <p className="text-sm text-muted-foreground">
            People, service accounts, and workloads POLLEK governs.
          </p>
        </div>
        <button className="flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90">
          <Plus className="h-4 w-4" /> Register entity
        </button>
      </div>

      <MasterDetailLayout
        idSelector={(x: any) => x.entity_id || x.id}
        items={items}
        loading={loading}
        selectedId={selectedId}
        onSelect={select}
        toolbar={
          <div className="flex items-center gap-2 mb-4">
            <input
              type="text"
              placeholder="Search entities..."
              className="px-3 py-1.5 text-sm rounded-md border bg-background"
            />
          </div>
        }
        emptyState={
          <EmptyState
            icon={UserCircle}
            title="No entities yet"
            description="Run a scan or register an entity to start governing access."
            actionLabel="Register entity"
          />
        }
        renderCard={(e, selected) => {
          const isDiscoverySurface = e.entity_id?.startsWith("discovery:");
          const isGoverned = e.meta?.status === "active";
          return (
            <EntityCard
              title={e.display_name}
              subtitle={e.entity_type}
              summary={
                isDiscoverySurface
                  ? `Discovered ${e.entity_type} surface with ${((e.attributes?.confidence ?? 0) * 100).toFixed(0)}% confidence and ${e.attributes?.capabilities?.length ?? 0} capability tag(s).`
                  : `${e.entity_type} from ${e.external_ids?.[0]?.provider ?? "registry"} is ${isGoverned ? "governed by active policy" : "available for policy targeting"}.`
              }
              icon={UserCircle}
              status={
                isGoverned ? "ok" : isDiscoverySurface ? "degraded" : "idle"
              }
              statusLabel={
                isGoverned
                  ? "Governed"
                  : isDiscoverySurface
                    ? "Discovered"
                    : "Unmanaged"
              }
              meta={[
                {
                  label: "Source",
                  value: e.meta?.source ?? e.attributes?.status ?? "local",
                },
                {
                  label: "Provider",
                  value: e.external_ids?.[0]?.provider ?? "registry",
                },
                ...(isDiscoverySurface && e.attributes?.confidence
                  ? [
                      {
                        label: "Confidence",
                        value: `${(e.attributes.confidence * 100).toFixed(0)}%`,
                      },
                    ]
                  : []),
              ]}
              selected={selected}
            />
          );
        }}
        renderDetail={(e) => {
          const isDiscoverySurface = e.entity_id?.startsWith("discovery:");
          const isGoverned = e.meta?.status === "active";
          return (
            <DetailPane
              title={e.display_name}
              subtitle={e.entity_type}
              status={isGoverned ? "ok" : "idle"}
              statusLabel={isGoverned ? "Governed" : "Unmanaged"}
              actions={[
                {
                  label: "Protect",
                  primary: true,
                  onClick: () => {
                    /* open wizard */
                  },
                },
                {
                  label: "Delete",
                  danger: true,
                  onClick: () => handleDelete(e.entity_id),
                },
              ]}
              tabs={[
                {
                  id: "overview",
                  label: "Overview",
                  content: (
                    <div className="space-y-4">
                      <div className="grid grid-cols-2 gap-4 text-sm">
                        <SummaryMetric
                          label="Entity"
                          value={e.display_name}
                          helper={`${e.entity_type} - ${isGoverned ? "governed" : "unmanaged"}`}
                        />
                        <SummaryMetric
                          label="Provider"
                          value={e.external_ids?.[0]?.provider ?? "registry"}
                          helper={e.external_ids?.[0]?.id ?? "No external ID linked."}
                        />
                        <SummaryMetric
                          label="Stable ID"
                          value={e.entity_id}
                          helper="Used by policies and relationship checks."
                        />
                        <SummaryMetric
                          label="Created"
                          value={
                            e.meta?.created_at
                              ? new Date(e.meta.created_at).toLocaleString()
                              : "N/A"
                          }
                          helper={e.meta?.source ? `Source: ${e.meta.source}` : undefined}
                        />
                        {isDiscoverySurface && (
                          <>
                            <SummaryMetric
                              label="Confidence"
                              value={
                                e.attributes?.confidence
                                  ? `${(e.attributes.confidence * 100).toFixed(0)}%`
                                  : "Unknown"
                              }
                              helper="Discovery confidence before registration."
                            />
                            <SummaryMetric
                              label="Capabilities"
                              value={e.attributes?.capabilities?.length ?? 0}
                              helper={
                                e.attributes?.capabilities?.join(", ") ||
                                "No capabilities reported yet."
                              }
                            />
                          </>
                        )}
                      </div>
                      <div className="pt-4">
                        <h4 className="font-medium mb-2 flex items-center gap-2">
                          <Info className="h-4 w-4" /> Raw Data
                        </h4>
                        <pre className="text-[10px] font-mono bg-muted/50 p-4 rounded-lg overflow-x-auto">
                          {JSON.stringify(e, null, 2)}
                        </pre>
                      </div>
                    </div>
                  ),
                },
                {
                  id: "policies",
                  label: "Policies",
                  content: (
                    <div className="flex flex-col items-center justify-center p-8 text-center border border-dashed rounded-lg text-muted-foreground">
                      <FileKey className="h-8 w-8 mb-2 opacity-50" />
                      <p className="text-sm">
                        No policies active for this entity.
                      </p>
                    </div>
                  ),
                },
                {
                  id: "activity",
                  label: "Activity",
                  content: (
                    <div className="flex flex-col items-center justify-center p-8 text-center border border-dashed rounded-lg text-muted-foreground">
                      <Activity className="h-8 w-8 mb-2 opacity-50" />
                      <p className="text-sm">No recent activity found.</p>
                    </div>
                  ),
                },
              ]}
            />
          );
        }}
      />
    </div>
  );
}
