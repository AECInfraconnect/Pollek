import type { EntityGraphResponse, GraphNode } from "./types";
import { RelationshipGraph } from "../../components/relationship/RelationshipGraph";

export function EntityGraphCanvas({
  graph,
  selectedNodeId,
  onNodeClick,
}: {
  graph: EntityGraphResponse;
  selectedNodeId?: string;
  onNodeClick: (node: GraphNode) => void;
}) {
  return (
    <RelationshipGraph
      nodes={graph.nodes}
      edges={graph.edges}
      selectedNodeId={selectedNodeId}
      onNodeClick={onNodeClick}
    />
  );
}
