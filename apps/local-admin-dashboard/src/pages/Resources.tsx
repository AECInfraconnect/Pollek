import { useState, useEffect } from "react";
import { Database, MoreVertical, Plus } from "lucide-react";
import { RegistryApi } from "../services/api";
import type { Resource } from "../services/api";
import { ResourceDetailDrawer } from "../components/ResourceDetailDrawer";

export function Resources() {
  const [resources, setResources] = useState<Resource[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedResource, setSelectedResource] = useState<Resource | null>(null);

  useEffect(() => {
    RegistryApi.listResources()
      .then(setResources)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">Protected Resources</h2>
          <p className="text-muted-foreground">
            Manage data boundaries and classifications for registered resources.
          </p>
        </div>
        <button className="flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors shadow-lg shadow-primary/20">
          <Plus className="h-4 w-4" />
          Add Resource
        </button>
      </div>

      <div className="glass rounded-xl overflow-hidden border">
        <table className="w-full text-sm text-left">
          <thead className="bg-muted/50 text-muted-foreground">
            <tr>
              <th className="px-6 py-4 font-medium">Resource Name</th>
              <th className="px-6 py-4 font-medium">Type</th>
              <th className="px-6 py-4 font-medium">URI</th>
              <th className="px-6 py-4 font-medium">Classification</th>
              <th className="px-6 py-4 font-medium text-right">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {loading ? (
              <tr>
                <td colSpan={5} className="px-6 py-8 text-center text-muted-foreground">
                  Loading resources...
                </td>
              </tr>
            ) : resources.length === 0 ? (
              <tr>
                <td colSpan={5} className="px-6 py-8 text-center text-muted-foreground">
                  No resources registered.
                </td>
              </tr>
            ) : resources.map((resource) => (
              <tr 
                key={resource.resource_id} 
                className="hover:bg-muted/30 transition-colors cursor-pointer"
                onClick={() => setSelectedResource(resource)}
              >
                <td className="px-6 py-4">
                  <div className="flex items-center gap-3">
                    <div className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center">
                      <Database className="h-4 w-4 text-primary" />
                    </div>
                    <span className="font-medium">{resource.name}</span>
                  </div>
                </td>
                <td className="px-6 py-4">
                  <span className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium bg-muted text-foreground uppercase">
                    {resource.resource_type}
                  </span>
                </td>
                <td className="px-6 py-4 text-muted-foreground font-mono text-xs truncate max-w-[200px]" title={resource.uri}>
                  {resource.uri}
                </td>
                <td className="px-6 py-4">
                  <span className={`inline-flex items-center gap-1.5 rounded-full px-2 py-1 text-xs font-medium ${
                    resource.classification === 'restricted'
                      ? 'bg-destructive/10 text-destructive' 
                      : resource.classification === 'confidential'
                      ? 'bg-amber-500/10 text-amber-500'
                      : 'bg-emerald-500/10 text-emerald-500'
                  }`}>
                    <span className={`h-1.5 w-1.5 rounded-full ${resource.classification === 'restricted' ? 'bg-destructive' : resource.classification === 'confidential' ? 'bg-amber-500' : 'bg-emerald-500'}`} />
                    {resource.classification}
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

      <ResourceDetailDrawer 
        resource={selectedResource} 
        onClose={() => setSelectedResource(null)} 
      />
    </div>
  );
}
