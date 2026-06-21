import { X } from "lucide-react";
import type { Resource } from "../services/api";

interface ResourceDetailDrawerProps {
  resource: Resource | null;
  onClose: () => void;
}

export function ResourceDetailDrawer({ resource, onClose }: ResourceDetailDrawerProps) {
  if (!resource) return null;

  return (
    <>
      <div 
        className="fixed inset-0 bg-background/80 backdrop-blur-sm z-40 transition-opacity"
        onClick={onClose}
      />
      <div className="fixed inset-y-0 right-0 w-full max-w-md bg-background border-l shadow-2xl z-50 flex flex-col transform transition-transform duration-300 ease-in-out">
        <div className="flex items-center justify-between px-6 py-4 border-b">
          <h2 className="text-lg font-semibold tracking-tight">Resource Details</h2>
          <button 
            onClick={onClose}
            className="rounded-full p-2 hover:bg-muted transition-colors"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
        
        <div className="flex-1 overflow-y-auto p-6 space-y-6">
          <div>
            <h3 className="text-sm font-medium text-muted-foreground mb-1">Name</h3>
            <p className="text-base font-medium">{resource.name}</p>
          </div>
          
          <div>
            <h3 className="text-sm font-medium text-muted-foreground mb-1">Data Boundary</h3>
            <p className="text-sm">{resource.data_boundary || 'No data boundary provided.'}</p>
          </div>
          
          <div>
            <h3 className="text-sm font-medium text-muted-foreground mb-1">Type</h3>
            <span className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium bg-muted text-foreground uppercase">
              {resource.resource_type}
            </span>
          </div>

          <div>
            <h3 className="text-sm font-medium text-muted-foreground mb-1">Location</h3>
            <p className="text-sm font-mono bg-muted p-2 rounded break-all">{resource.uri}</p>
          </div>

          <div>
            <h3 className="text-sm font-medium text-muted-foreground mb-2">Classification</h3>
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
          </div>
          
          <div>
            <h3 className="text-sm font-medium text-muted-foreground mb-2">Raw Metadata</h3>
            <pre className="bg-muted p-4 rounded-lg text-xs overflow-x-auto">
              {JSON.stringify(resource.meta, null, 2)}
            </pre>
          </div>
        </div>
      </div>
    </>
  );
}
