import { Code, AlertTriangle } from "lucide-react";
import type { PolicyPresetPreviewResponse } from "../../types/policy-presets";

export function PolicyPreview({ preview }: { preview: PolicyPresetPreviewResponse }) {
  if (!preview || !preview.artifacts || preview.artifacts.length === 0) {
    return (
      <div className="text-sm text-muted-foreground p-4 bg-muted/30 rounded border">
        No artifacts generated for this preset with the current configuration.
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {preview.artifacts.map((artifact, idx) => (
        <div key={idx} className="bg-muted/30 border rounded-lg overflow-hidden">
          <div className="bg-muted px-4 py-2 border-b flex justify-between items-center text-sm font-medium">
            <span className="flex items-center gap-2">
              <Code className="h-4 w-4" /> {artifact.language.toUpperCase()} Artifact
            </span>
          </div>
          <div className="p-4 overflow-auto max-h-64 text-xs font-mono whitespace-pre">
            {artifact.content}
          </div>
          {artifact.warnings && artifact.warnings.length > 0 && (
            <div className="p-3 bg-yellow-500/10 border-t border-yellow-500/20 text-yellow-600 text-xs">
              <div className="font-semibold flex items-center gap-1 mb-1">
                <AlertTriangle className="h-3 w-3" /> Warnings:
              </div>
              <ul className="list-disc pl-4 space-y-1">
                {artifact.warnings.map((w, i) => {
                  const isUncoveredRisk = w.toLowerCase().includes("uncovered risk");
                  return (
                    <li key={i} className={isUncoveredRisk ? "text-red-500 font-medium" : ""}>
                      {w}
                    </li>
                  );
                })}
              </ul>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
