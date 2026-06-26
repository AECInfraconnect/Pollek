import { useMemo, useState } from "react";
import { Network, RefreshCw, Search } from "lucide-react";
import { useEntityGraph } from "./useEntityGraph";
import type { GraphNode } from "./types";
import { EntityGraphCanvas } from "./EntityGraphCanvas";
import { EntityQuickViewDrawer } from "./EntityQuickViewDrawer";
import { RelationshipSummaryCards } from "../entity-360/RelationshipSummaryCards";

const typeOptions = [
  { label: "All", value: "" },
  { label: "Agents", value: "agent" },
  { label: "Tools", value: "tool" },
  { label: "Resources", value: "resource" },
  { label: "Policies", value: "policy" },
  { label: "Identities", value: "identity" },
  { label: "Providers", value: "provider" },
];

export function EntityGraphPage() {
  const [query, setQuery] = useState("");
  const [types, setTypes] = useState("");
  const [selected, setSelected] = useState<GraphNode | null>(null);
  const params = useMemo(
    () => ({ q: query || undefined, types: types || undefined, limit: 250 }),
    [query, types],
  );
  const { data, loading, error } = useEntityGraph(params);

  return (
    <div className="space-y-5">
      <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
        <div>
          <h2 className="flex items-center gap-2 text-2xl font-bold tracking-tight">
            <Network className="h-6 w-6 text-primary" />
            Relationship Map
          </h2>
          <p className="text-sm text-muted-foreground">
            Explore agents, tools, resources, identities, policies, decisions, and usage links from one local read model.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <div className="relative">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search graph..."
              className="h-9 rounded-md border bg-background pl-8 pr-3 text-sm"
            />
          </div>
          <select
            value={types}
            onChange={(event) => setTypes(event.target.value)}
            className="h-9 rounded-md border bg-background px-3 text-sm"
          >
            {typeOptions.map((option) => (
              <option key={option.label} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </div>
      </div>

      {data && <RelationshipSummaryCards items={data.summaries} />}

      <div className="relative overflow-hidden rounded-lg border bg-card/40 p-3">
        {loading && (
          <div className="absolute inset-0 z-20 flex items-center justify-center bg-background/60 backdrop-blur-sm">
            <RefreshCw className="h-5 w-5 animate-spin text-primary" />
          </div>
        )}
        {error && (
          <div className="rounded-lg border border-red-500/20 bg-red-500/10 p-4 text-sm text-red-600">
            {error.message}
          </div>
        )}
        {data && (
          <EntityGraphCanvas
            graph={data}
            selectedNodeId={selected?.id}
            onNodeClick={setSelected}
          />
        )}
        <EntityQuickViewDrawer node={selected} onClose={() => setSelected(null)} />
      </div>
    </div>
  );
}
