import { AlertTriangle } from "lucide-react";
import type {
  EntityGraphResponse,
  GraphNode,
} from "../../features/entity-graph/types";
import { RelationshipGraph } from "./RelationshipGraph";
import { RelationshipEdgeLegend } from "./RelationshipEdgeLegend";
import { RelationshipSummaryCards } from "../../features/entity-360/RelationshipSummaryCards";

export function EntityRelationshipPanel({
  graph,
  selectedNodeId,
  compact,
  onNodeClick,
}: {
  graph: EntityGraphResponse;
  selectedNodeId?: string;
  compact?: boolean;
  onNodeClick?: (node: GraphNode) => void;
}) {
  return (
    <section className="space-y-4" aria-label="Entity relationships">
      <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
        <div>
          <h3 className="text-base font-semibold">Relationship Map</h3>
          <p className="text-sm text-muted-foreground">
            Direct entity links from registry, policies, observations, and usage telemetry.
          </p>
        </div>
        <RelationshipEdgeLegend />
      </div>
      <RelationshipSummaryCards items={graph.summaries} />
      <RelationshipGraph
        nodes={graph.nodes}
        edges={graph.edges}
        selectedNodeId={selectedNodeId ?? graph.center?.id}
        compact={compact}
        onNodeClick={onNodeClick}
      />
      {graph.warnings.length > 0 && (
        <div className="rounded-lg border border-amber-500/20 bg-amber-500/10 p-4">
          <div className="flex items-center gap-2 text-sm font-medium text-amber-700">
            <AlertTriangle className="h-4 w-4" />
            Policy coverage gaps
          </div>
          <div className="mt-2 space-y-1 text-sm text-amber-700/90">
            {graph.warnings.slice(0, 4).map((warning) => (
              <p key={`${warning.code}-${warning.entity_id}`}>{warning.message}</p>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}
