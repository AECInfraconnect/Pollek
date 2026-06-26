import { useState, useEffect } from "react";
import { UserCircle, Network, Activity, Info } from "lucide-react";
import { useSearchParams } from "react-router-dom";
import { RegistryApi, TelemetryApi } from "../../services/api";
import type {
  Entity,
  ObservedIdentity,
  Relationship,
} from "../../services/types";
import { MasterDetailLayout } from "../../components/master-detail/MasterDetailLayout";
import { EntityCard } from "../../components/master-detail/EntityCard";
import { DetailPane } from "../../components/master-detail/DetailPane";
import { EmptyState } from "../../components/master-detail/EmptyState";

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

export function IdentityNetwork() {
  const [entities, setEntities] = useState<
    (Entity & { observed_details?: ObservedIdentity; is_observed?: boolean })[]
  >([]);
  const [relationships, setRelationships] = useState<Relationship[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [scopeFilter, setScopeFilter] = useState<"all" | "local" | "cloud">(
    "all",
  );
  const [agentFilter, setAgentFilter] = useState("");
  const [params, setParams] = useSearchParams();
  const selectedId = params.get("selected") ?? undefined;

  const loadData = () => {
    setLoading(true);
    Promise.all([
      RegistryApi.listEntities(),
      RegistryApi.listRelationships(),
      TelemetryApi.listIdentityInventory({
        agentId: agentFilter || undefined,
        scope: scopeFilter === "all" ? undefined : scopeFilter,
      }).catch(() => ({ items: [] as ObservedIdentity[] })),
    ])
      .then(([ents, rels, observed]) => {
        const map = new Map<
          string,
          Entity & { observed_details?: ObservedIdentity; is_observed?: boolean }
        >();
        for (const entity of ents) {
          map.set(entity.entity_id, { ...entity, is_observed: false });
        }
        for (const identity of observed.items ?? []) {
          const existing = map.get(identity.identity_id);
          if (existing) {
            existing.observed_details = identity;
            existing.is_observed = true;
          } else {
            map.set(identity.identity_id, {
              meta: {
                schema_version: "entity.v1",
                tenant_id: "local",
                workspace_id: "local",
                environment_id: "local",
                created_at: identity.last_seen,
                updated_at: identity.last_seen,
                created_by: "telemetry",
                updated_by: "telemetry",
                source: "discovery",
                status: "discovered",
                tags: ["observed"],
              },
              entity_id: identity.identity_id,
              entity_type:
                identity.identity_kind === "device"
                  ? "device"
                  : identity.identity_kind === "workload"
                    ? "workload"
                    : identity.identity_kind === "user"
                      ? "human_user"
                      : "service_account",
              display_name: identity.identity_label,
              external_ids: [
                {
                  provider: identity.provider ?? "telemetry",
                  id: identity.identity_id,
                },
              ],
              roles: [],
              attributes: {
                scope: identity.scope,
                kind: identity.identity_kind,
                provider: identity.provider,
              },
              observed_details: identity,
              is_observed: true,
            });
          }
        }
        setEntities(
          Array.from(map.values()).filter((entity) => {
            const haystack =
              `${entity.display_name} ${entity.entity_type} ${entity.entity_id}`.toLowerCase();
            return haystack.includes(search.trim().toLowerCase());
          }),
        );
        setRelationships(rels);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    loadData();

    const source = new EventSource(TelemetryApi.streamUrl("identities"));
    source.onmessage = () => {
      // Refresh identity graph on telemetry update
      loadData();
    };

    return () => source.close();
  }, [search, scopeFilter, agentFilter]);

  const select = (id: string) =>
    setParams((p) => {
      p.set("selected", id);
      return p;
    });

  return (
    <div className="p-6 md:p-8 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-semibold tracking-tight">
            Identity & Network
          </h2>
          <p className="text-sm text-muted-foreground">
            Manage your local identity graph: people, systems, and their
            relationships.
          </p>
        </div>
      </div>

      <MasterDetailLayout
        idSelector={(x: any) => x.identity_id || x.id}
        items={entities}
        loading={loading}
        selectedId={selectedId}
        onSelect={select}
        toolbar={
          <div className="flex items-center gap-2 mb-4">
            <input
              type="text"
              placeholder="Search identities..."
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              className="px-3 py-1.5 text-sm rounded-md border bg-background"
            />
            <select
              value={scopeFilter}
              onChange={(event) =>
                setScopeFilter(event.target.value as "all" | "local" | "cloud")
              }
              className="px-3 py-1.5 text-sm rounded-md border bg-background"
            >
              <option value="all">All scopes</option>
              <option value="local">Local</option>
              <option value="cloud">Cloud</option>
            </select>
            <input
              type="text"
              placeholder="Agent ID"
              value={agentFilter}
              onChange={(event) => setAgentFilter(event.target.value)}
              className="px-3 py-1.5 text-sm rounded-md border bg-background"
            />
          </div>
        }
        emptyState={
          <EmptyState
            icon={Network}
            title="No identities found"
            description="The identity graph is currently empty."
          />
        }
        renderCard={(e, selected) => {
          const isGoverned = e.meta?.status === "active";
          return (
            <EntityCard
              title={e.display_name}
              subtitle={e.entity_type}
              icon={UserCircle}
              status={isGoverned ? "ok" : e.is_observed ? "degraded" : "idle"}
              statusLabel={
                isGoverned ? "Governed" : e.is_observed ? "Observed" : "Unmanaged"
              }
              meta={[
                {
                  label: "Provider",
                  value:
                    e.observed_details?.provider ??
                    e.external_ids?.[0]?.provider ??
                    "local",
                },
                ...(e.observed_details
                  ? [
                      {
                        label: "Agents",
                        value: e.observed_details.agents.length,
                      },
                    ]
                  : []),
              ]}
              selected={selected}
            />
          );
        }}
        renderDetail={(e) => {
          const isGoverned = e.meta?.status === "active";
          const related = relationships.filter(
            (r) =>
              r.subject.object_id === e.entity_id ||
              r.object.object_id === e.entity_id,
          );

          return (
            <DetailPane
              title={e.display_name}
              subtitle={e.entity_type}
              status={isGoverned ? "ok" : e.is_observed ? "degraded" : "idle"}
              statusLabel={
                isGoverned ? "Governed" : e.is_observed ? "Observed" : "Unmanaged"
              }
              tabs={[
                {
                  id: "overview",
                  label: "Overview",
                  content: (
                    <div className="space-y-6">
                      <div className="grid grid-cols-2 gap-4 text-sm">
                        <SummaryMetric
                          label="Identity"
                          value={e.display_name}
                          helper={`${e.entity_type} - ${e.is_observed ? "observed" : "registered"}`}
                        />
                        <SummaryMetric
                          label="Provider"
                          value={
                            e.observed_details?.provider ??
                            e.external_ids?.[0]?.provider ??
                            "local"
                          }
                          helper={e.roles?.length ? `Roles: ${e.roles.join(", ")}` : "No roles assigned."}
                        />
                        <SummaryMetric
                          label="Stable ID"
                          value={e.entity_id}
                          helper="Used to join registry, telemetry, and policy targets."
                        />
                        {e.observed_details?.spiffe_id && (
                          <SummaryMetric
                            label="SPIFFE ID"
                            value={e.observed_details.spiffe_id}
                            helper="Workload trace identity for Pollek Cloud correlation."
                          />
                        )}
                        {e.observed_details && (
                          <>
                            <SummaryMetric
                              label="Last seen"
                              value={new Date(
                                e.observed_details.last_seen,
                              ).toLocaleString()}
                              helper={`${e.observed_details.access_count} identity event(s).`}
                            />
                            <SummaryMetric
                              label="Agents using it"
                              value={e.observed_details.agents.length}
                              helper={e.observed_details.agents.join(", ") || "No agent linked yet."}
                            />
                            <SummaryMetric
                              label="Actions"
                              value={e.observed_details.actions.join(", ") || "access"}
                              helper="Authentication, token, delegation, or access actions observed."
                            />
                            <SummaryMetric
                              label="Governance"
                              value={
                                e.observed_details.governed
                                  ? "Policy attached"
                                  : "Needs policy"
                              }
                              helper="Registered agents should bind to this identity before Cloud control."
                            />
                          </>
                        )}
                      </div>

                      <div>
                        <h4 className="font-medium mb-2 flex items-center gap-2 text-sm">
                          <Info className="h-4 w-4" /> Raw Data
                        </h4>
                        <pre className="text-[10px] font-mono bg-muted/50 p-4 rounded-lg overflow-x-auto border">
                          {JSON.stringify(e, null, 2)}
                        </pre>
                      </div>
                    </div>
                  ),
                },
                {
                  id: "relationships",
                  label: "Relationships",
                  content: (
                    <div className="space-y-4">
                      {related.length === 0 ? (
                        <div className="flex flex-col items-center justify-center p-8 text-center border border-dashed rounded-lg text-muted-foreground">
                          <Network className="h-8 w-8 mb-2 opacity-50" />
                          <p className="text-sm">
                            No relationships defined for this identity.
                          </p>
                        </div>
                      ) : (
                        <div className="rounded-md border bg-card/30">
                          <table className="w-full text-sm text-left">
                            <thead className="bg-muted/50">
                              <tr>
                                <th className="px-4 py-2 font-medium">
                                  Relation
                                </th>
                                <th className="px-4 py-2 font-medium">
                                  Target
                                </th>
                              </tr>
                            </thead>
                            <tbody className="divide-y divide-border">
                              {related.map((r) => {
                                const isSubject =
                                  r.subject.object_id === e.entity_id;
                                const target = isSubject ? r.object : r.subject;
                                return (
                                  <tr key={r.relationship_id}>
                                    <td className="px-4 py-3 font-medium text-primary">
                                      {isSubject
                                        ? `${r.relation} ➔`
                                        : `⬅ ${r.relation}`}
                                    </td>
                                    <td className="px-4 py-3 font-mono text-xs">
                                      {target.object_type}:{target.object_id}
                                    </td>
                                  </tr>
                                );
                              })}
                            </tbody>
                          </table>
                        </div>
                      )}
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
