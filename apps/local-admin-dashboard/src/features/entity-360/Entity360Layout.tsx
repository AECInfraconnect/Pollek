import { useState, type ReactNode } from "react";
import { EntityRelationshipPanel } from "../../components/relationship/EntityRelationshipPanel";
import type { Entity360Response } from "../entity-graph/types";
import { CostTokenPanel } from "./CostTokenPanel";
import { EntityHeader } from "./EntityHeader";
import { EntityKpiStrip } from "./EntityKpiStrip";
import { EvidenceTimeline } from "./EvidenceTimeline";
import { PolicyImpactPanel } from "./PolicyImpactPanel";
import { RelatedEntityCards } from "./RelatedEntityCards";

type TabId =
  | "overview"
  | "relationships"
  | "policies"
  | "activity"
  | "cost"
  | "debug";

export function Entity360Layout({
  data,
  actions,
  overview,
}: {
  data: Entity360Response;
  actions?: ReactNode;
  overview?: ReactNode;
}) {
  const [activeTab, setActiveTab] = useState<TabId>("overview");
  const isPolicy = data.entity.type === "policy";
  const tabs: Array<{ id: TabId; label: string }> = [
    { id: "overview", label: "Overview" },
    { id: "relationships", label: "Relationships" },
    { id: "policies", label: isPolicy ? "Impact" : "Policies" },
    { id: "activity", label: "Activity" },
    { id: "cost", label: "Cost & Tokens" },
    { id: "debug", label: "Raw JSON" },
  ];

  return (
    <div className="space-y-4">
      <EntityHeader entity={data.entity} actions={actions} />
      <EntityKpiStrip data={data} />

      <div className="grid gap-4 xl:grid-cols-[1.05fr_0.95fr]">
        <EntityRelationshipPanel
          graph={data.graph}
          selectedNodeId={data.entity.id}
          compact
        />
        <section className="space-y-3 rounded-lg border bg-card/50 p-4">
          <h3 className="font-semibold">Friendly Summary</h3>
          {overview ?? (
            <div className="space-y-3 text-sm text-muted-foreground">
              <p>
                This view connects the selected entity to policies, tools, resources,
                identities, decisions, and cost evidence available on this local device.
              </p>
              <RelatedEntityCards nodes={data.graph.nodes} centerId={data.entity.id} />
            </div>
          )}
        </section>
      </div>

      <div className="rounded-lg border bg-card/50">
        <div className="overflow-x-auto border-b px-4">
          <nav className="flex gap-5" aria-label="Entity 360 tabs">
            {tabs.map((tab) => (
              <button
                key={tab.id}
                type="button"
                onClick={() => setActiveTab(tab.id)}
                className={`whitespace-nowrap border-b-2 px-1 py-3 text-sm font-medium transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary ${
                  activeTab === tab.id
                    ? "border-primary text-primary"
                    : "border-transparent text-muted-foreground hover:text-foreground"
                }`}
              >
                {tab.label}
              </button>
            ))}
          </nav>
        </div>
        <div className="p-4">
          {activeTab === "overview" && (
            <RelatedEntityCards nodes={data.graph.nodes} centerId={data.entity.id} />
          )}
          {activeTab === "relationships" && (
            <EntityRelationshipPanel
              graph={data.graph}
              selectedNodeId={data.entity.id}
            />
          )}
          {activeTab === "policies" &&
            (isPolicy ? (
              <PolicyImpactPanel data={data} />
            ) : (
              <RelatedEntityCards
                nodes={data.graph.nodes.filter(
                  (node) => node.type === "policy" || node.id === data.entity.id,
                )}
                centerId={data.entity.id}
              />
            ))}
          {activeTab === "activity" && <EvidenceTimeline items={data.activity} />}
          {activeTab === "cost" && <CostTokenPanel items={data.activity} />}
          {activeTab === "debug" && (
            <pre className="max-h-[520px] overflow-auto rounded-lg bg-muted/40 p-4 text-xs">
              {JSON.stringify(data, null, 2)}
            </pre>
          )}
        </div>
      </div>
    </div>
  );
}
