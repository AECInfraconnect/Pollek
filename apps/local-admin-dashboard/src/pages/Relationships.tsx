import { useState, useEffect } from "react";
import { Network, MoreVertical, Plus } from "lucide-react";
import { RegistryApi } from "../services/api";
import type { Relationship } from "../services/types";

export function Relationships({
  hideHeader = false,
}: {
  hideHeader?: boolean;
}) {
  const [relationships, setRelationships] = useState<Relationship[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    RegistryApi.listRelationships()
      .then(setRelationships)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  return (
    <div className="space-y-6">
      {!hideHeader && (
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-lg font-semibold tracking-tight">Relationships</h2>
            <p className="text-muted-foreground">
              Manage relationships and graph edges between entities, resources,
              and agents.
            </p>
          </div>
          <button className="flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors shadow-lg shadow-primary/20">
            <Plus className="h-4 w-4" />
            Create Edge
          </button>
        </div>
      )}

      <div className="glass rounded-xl overflow-hidden border">
        <table className="w-full text-sm text-left">
          <thead className="bg-muted/50 text-muted-foreground">
            <tr>
              <th className="px-6 py-4 font-medium">Subject</th>
              <th className="px-6 py-4 font-medium">Relation</th>
              <th className="px-6 py-4 font-medium">Object</th>
              <th className="px-6 py-4 font-medium">Status</th>
              <th className="px-6 py-4 font-medium text-right">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {loading ? (
              <tr>
                <td
                  colSpan={5}
                  className="px-6 py-8 text-center text-muted-foreground"
                >
                  Loading relationships...
                </td>
              </tr>
            ) : relationships.length === 0 ? (
              <tr>
                <td
                  colSpan={5}
                  className="px-6 py-8 text-center text-muted-foreground"
                >
                  No relationships registered.
                </td>
              </tr>
            ) : (
              relationships.map((rel) => (
                <tr
                  key={rel.relationship_id}
                  className="hover:bg-muted/30 transition-colors"
                >
                  <td className="px-6 py-4">
                    <div className="flex items-center gap-3">
                      <div className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center">
                        <Network className="h-4 w-4 text-primary" />
                      </div>
                      <span className="font-mono text-xs">
                        {rel.subject.object_type}:{rel.subject.object_id}
                      </span>
                    </div>
                  </td>
                  <td className="px-6 py-4 text-primary font-semibold">
                    {rel.relation}
                  </td>
                  <td className="px-6 py-4 font-mono text-xs text-muted-foreground">
                    {rel.object.object_type}:{rel.object.object_id}
                  </td>
                  <td className="px-6 py-4">
                    <span
                      className={`inline-flex items-center gap-1.5 rounded-full px-2 py-1 text-xs font-medium ${
                        rel.meta.status === "active"
                          ? "bg-emerald-500/10 text-emerald-500"
                          : "bg-muted text-muted-foreground"
                      }`}
                    >
                      <span
                        className={`h-1.5 w-1.5 rounded-full ${rel.meta.status === "active" ? "bg-emerald-500" : "bg-muted-foreground"}`}
                      />
                      {rel.meta.status}
                    </span>
                  </td>
                  <td className="px-6 py-4 text-right">
                    <button className="text-muted-foreground hover:text-foreground transition-colors p-1">
                      <MoreVertical className="h-4 w-4" />
                    </button>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
