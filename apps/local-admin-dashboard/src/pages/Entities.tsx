import { useState, useEffect } from "react";
import { UserCircle, MoreVertical, Plus } from "lucide-react";
import { RegistryApi } from "../services/api";
import type { Entity } from "../services/types";

export function Entities({ hideHeader = false }: { hideHeader?: boolean }) {
  const [entities, setEntities] = useState<Entity[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    RegistryApi.listEntities()
      .then(setEntities)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  return (
    <div className="space-y-6">
      {!hideHeader && (
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-2xl font-bold tracking-tight">Entities</h2>
            <p className="text-muted-foreground">
              Manage human users, service accounts, and workloads.
            </p>
          </div>
          <button className="flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors shadow-lg shadow-primary/20">
            <Plus className="h-4 w-4" />
            Register Entity
          </button>
        </div>
      )}

      <div className="glass rounded-xl overflow-hidden border">
        <table className="w-full text-sm text-left">
          <thead className="bg-muted/50 text-muted-foreground">
            <tr>
              <th className="px-6 py-4 font-medium">Display Name</th>
              <th className="px-6 py-4 font-medium">Entity ID</th>
              <th className="px-6 py-4 font-medium">Type</th>
              <th className="px-6 py-4 font-medium">Roles</th>
              <th className="px-6 py-4 font-medium">Status</th>
              <th className="px-6 py-4 font-medium text-right">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {loading ? (
              <tr>
                <td colSpan={6} className="px-6 py-8 text-center text-muted-foreground">
                  Loading entities...
                </td>
              </tr>
            ) : entities.length === 0 ? (
              <tr>
                <td colSpan={6} className="px-6 py-8 text-center text-muted-foreground">
                  No entities registered.
                </td>
              </tr>
            ) : entities.map((entity) => (
              <tr key={entity.entity_id} className="hover:bg-muted/30 transition-colors">
                <td className="px-6 py-4">
                  <div className="flex items-center gap-3">
                    <div className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center">
                      <UserCircle className="h-4 w-4 text-primary" />
                    </div>
                    <span className="font-medium">{entity.display_name}</span>
                  </div>
                </td>
                <td className="px-6 py-4 text-muted-foreground font-mono text-xs">{entity.entity_id}</td>
                <td className="px-6 py-4 text-muted-foreground">{entity.entity_type}</td>
                <td className="px-6 py-4 text-muted-foreground">{entity.roles?.join(", ") || "None"}</td>
                <td className="px-6 py-4">
                  <span className={`inline-flex items-center gap-1.5 rounded-full px-2 py-1 text-xs font-medium ${
                    entity.meta.status === 'active' 
                      ? 'bg-emerald-500/10 text-emerald-500' 
                      : 'bg-muted text-muted-foreground'
                  }`}>
                    <span className={`h-1.5 w-1.5 rounded-full ${entity.meta.status === 'active' ? 'bg-emerald-500' : 'bg-muted-foreground'}`} />
                    {entity.meta.status}
                  </span>
                </td>
                <td className="px-6 py-4 text-right">
                  <button className="text-muted-foreground hover:text-foreground transition-colors p-1">
                    <MoreVertical className="h-4 w-4" />
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
