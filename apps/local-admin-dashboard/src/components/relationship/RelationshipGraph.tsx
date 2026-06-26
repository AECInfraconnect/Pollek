import { useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { Network } from "lucide-react";
import { cn } from "@/lib/utils";
import type { GraphEdge, GraphNode } from "../../features/entity-graph/types";
import { entityRoute } from "../../features/entity-graph/graphUtils";
import { RelationshipNodeCard } from "./RelationshipNodeCard";

interface PositionedNode {
  node: GraphNode;
  x: number;
  y: number;
}

function layoutNodes(nodes: GraphNode[], selectedId?: string, compact = false) {
  const width = compact ? 720 : 1080;
  const height = compact ? 360 : 620;
  const center = nodes.find((node) => node.id === selectedId) ?? nodes[0];
  const others = nodes.filter((node) => node.id !== center?.id);
  const radiusX = compact ? 245 : 380;
  const radiusY = compact ? 105 : 205;
  const positioned: PositionedNode[] = [];

  if (center) {
    positioned.push({ node: center, x: width / 2, y: height / 2 });
  }

  others.forEach((node, index) => {
    const angle = (Math.PI * 2 * index) / Math.max(others.length, 1) - Math.PI / 2;
    positioned.push({
      node,
      x: width / 2 + Math.cos(angle) * radiusX,
      y: height / 2 + Math.sin(angle) * radiusY,
    });
  });

  return { width, height, positioned };
}

export function RelationshipGraph({
  nodes,
  edges,
  selectedNodeId,
  compact = false,
  onNodeClick,
}: {
  nodes: GraphNode[];
  edges: GraphEdge[];
  selectedNodeId?: string;
  compact?: boolean;
  onNodeClick?: (node: GraphNode) => void;
}) {
  const navigate = useNavigate();
  const { width, height, positioned } = useMemo(
    () => layoutNodes(nodes.slice(0, compact ? 10 : 30), selectedNodeId, compact),
    [nodes, selectedNodeId, compact],
  );
  const positions = useMemo(
    () => new Map(positioned.map((item) => [item.node.id, item])),
    [positioned],
  );
  const visibleNodeIds = new Set(positioned.map((item) => item.node.id));
  const visibleEdges = edges.filter(
    (edge) => visibleNodeIds.has(edge.source) && visibleNodeIds.has(edge.target),
  );

  if (!nodes.length) {
    return (
      <div className="flex min-h-64 flex-col items-center justify-center rounded-lg border border-dashed bg-muted/20 p-8 text-center text-muted-foreground">
        <Network className="mb-3 h-8 w-8" />
        <div className="text-sm font-medium text-foreground">No relationships yet</div>
        <p className="mt-1 max-w-md text-sm">
          Registered entities and observed activity will appear here as connected nodes.
        </p>
      </div>
    );
  }

  return (
    <div
      role="img"
      aria-label="Entity relationship graph"
      className={cn(
        "relative overflow-hidden rounded-lg border bg-background/80",
        compact ? "h-[360px]" : "h-[620px]",
      )}
    >
      <div
        className="absolute left-1/2 top-1/2 origin-center"
        style={{
          width,
          height,
          transform: `translate(-50%, -50%) scale(${compact ? 0.82 : 0.95})`,
        }}
      >
        <svg
          className="absolute inset-0 h-full w-full"
          viewBox={`0 0 ${width} ${height}`}
          aria-hidden="true"
        >
          <defs>
            <marker
              id="relationship-arrow"
              viewBox="0 0 10 10"
              refX="9"
              refY="5"
              markerWidth="6"
              markerHeight="6"
              orient="auto-start-reverse"
            >
              <path d="M 0 0 L 10 5 L 0 10 z" className="fill-muted-foreground/50" />
            </marker>
          </defs>
          {visibleEdges.map((edge) => {
            const source = positions.get(edge.source);
            const target = positions.get(edge.target);
            if (!source || !target) return null;
            const stroke = edge.enforced
              ? "rgb(16 185 129 / 0.65)"
              : edge.observed
                ? "rgb(59 130 246 / 0.58)"
                : "rgb(148 163 184 / 0.45)";
            const midX = (source.x + target.x) / 2;
            const midY = (source.y + target.y) / 2;
            return (
              <g key={edge.id}>
                <line
                  x1={source.x}
                  y1={source.y}
                  x2={target.x}
                  y2={target.y}
                  stroke={stroke}
                  strokeWidth={edge.enforced ? 2.4 : 1.6}
                  strokeDasharray={edge.observed ? undefined : "5 5"}
                  markerEnd="url(#relationship-arrow)"
                />
                {!compact && (
                  <text
                    x={midX}
                    y={midY - 6}
                    textAnchor="middle"
                    className="fill-muted-foreground text-[11px]"
                  >
                    {edge.label}
                  </text>
                )}
              </g>
            );
          })}
        </svg>

        {positioned.map(({ node, x, y }) => (
          <button
            key={node.id}
            type="button"
            onClick={() => {
              if (onNodeClick) onNodeClick(node);
              else navigate(entityRoute(node));
            }}
            className="absolute w-48 -translate-x-1/2 -translate-y-1/2 rounded-lg text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
            style={{ left: x, top: y }}
          >
            <RelationshipNodeCard
              node={node}
              selected={node.id === selectedNodeId}
              compact={compact}
            />
          </button>
        ))}
      </div>
    </div>
  );
}
