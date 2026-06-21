import { X } from "lucide-react";
import type { Tool } from "../services/api";

interface ToolDetailDrawerProps {
  tool: Tool | null;
  onClose: () => void;
}

export function ToolDetailDrawer({ tool, onClose }: ToolDetailDrawerProps) {
  if (!tool) return null;

  return (
    <>
      <div 
        className="fixed inset-0 bg-background/80 backdrop-blur-sm z-40 transition-opacity"
        onClick={onClose}
      />
      <div className="fixed inset-y-0 right-0 w-full max-w-md bg-background border-l shadow-2xl z-50 flex flex-col transform transition-transform duration-300 ease-in-out">
        <div className="flex items-center justify-between px-6 py-4 border-b">
          <h2 className="text-lg font-semibold tracking-tight">Tool Details</h2>
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
            <p className="text-base font-medium">{tool.name}</p>
          </div>
          
          <div>
            <h3 className="text-sm font-medium text-muted-foreground mb-1">Description</h3>
            <p className="text-sm">{tool.description || 'No description provided.'}</p>
          </div>
          
          <div className="grid grid-cols-2 gap-4">
            <div>
              <h3 className="text-sm font-medium text-muted-foreground mb-1">Data Access</h3>
              <span className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium bg-muted text-foreground uppercase">
                {tool.data_access_level}
              </span>
            </div>
            <div>
              <h3 className="text-sm font-medium text-muted-foreground mb-1">Side Effect</h3>
              <span className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium bg-muted text-foreground uppercase">
                {tool.side_effect_level}
              </span>
            </div>
            <div>
              <h3 className="text-sm font-medium text-muted-foreground mb-1">Risk Level</h3>
              <span className={`inline-flex items-center gap-1.5 rounded-full px-2 py-1 text-xs font-medium ${
                tool.risk_level === 'high' || tool.risk_level === 'critical'
                  ? 'bg-destructive/10 text-destructive' 
                  : tool.risk_level === 'medium'
                  ? 'bg-amber-500/10 text-amber-500'
                  : 'bg-emerald-500/10 text-emerald-500'
              }`}>
                <span className={`h-1.5 w-1.5 rounded-full ${tool.risk_level === 'high' || tool.risk_level === 'critical' ? 'bg-destructive' : tool.risk_level === 'medium' ? 'bg-amber-500' : 'bg-emerald-500'}`} />
                {tool.risk_level}
              </span>
            </div>
          </div>

          <div>
            <h3 className="text-sm font-medium text-muted-foreground mb-2">Input Schema</h3>
            <pre className="bg-muted p-4 rounded-lg text-xs overflow-x-auto">
              {JSON.stringify(tool.input_schema, null, 2)}
            </pre>
          </div>
          
          <div>
            <h3 className="text-sm font-medium text-muted-foreground mb-2">Raw Metadata</h3>
            <pre className="bg-muted p-4 rounded-lg text-xs overflow-x-auto">
              {JSON.stringify(tool.meta, null, 2)}
            </pre>
          </div>
        </div>
      </div>
    </>
  );
}
