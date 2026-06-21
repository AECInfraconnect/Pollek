import { AlertTriangle, CheckCircle2, XCircle } from "lucide-react";
import type { PolicyPresetSimulationResponse } from "../../types/policy-presets";

export function SimulationSummary({ simResult }: { simResult: PolicyPresetSimulationResponse }) {
  if (!simResult || !simResult.result) return null;

  const { result } = simResult;
  
  return (
    <div
      className={`p-4 rounded border ${
        result.allowed
          ? "bg-green-500/10 border-green-500/20"
          : "bg-red-500/10 border-red-500/20"
      }`}
    >
      <div className="flex items-center gap-2 mb-2 font-semibold">
        {result.decision === "error" ? (
          <AlertTriangle className="h-4 w-4 text-orange-400" />
        ) : result.allowed ? (
          <CheckCircle2 className="h-4 w-4 text-green-500" />
        ) : (
          <XCircle className="h-4 w-4 text-red-500" />
        )}
        <span
          className={
            result.decision === "error"
              ? "text-orange-400"
              : result.allowed
                ? "text-green-500"
                : "text-red-500"
          }
        >
          {result.decision.toUpperCase()}
        </span>
      </div>
      <div className="text-sm">
        {result.reason && <p className="mb-2">{result.reason}</p>}
      </div>
      {result.obligations && result.obligations.length > 0 && (
        <div className="mt-2 text-xs text-muted-foreground border-t border-border/50 pt-2">
          <strong>Obligations:</strong>
          <ul className="list-disc pl-4 mt-1">
            {result.obligations.map((o: any, idx: number) => (
              <li key={idx}>{JSON.stringify(o)}</li>
            ))}
          </ul>
        </div>
      )}
      {result.deployment_test && result.deployment_test.startsWith("Failed:") && (
        <div className="mt-3 p-3 bg-red-500/10 border-t border-red-500/20 text-red-600 text-xs rounded">
          <div className="font-bold flex items-center gap-1 mb-1">
            <AlertTriangle className="h-4 w-4" /> Uncovered Risk Detected
          </div>
          <p>{result.deployment_test}</p>
          <div className="mt-2 space-x-2">
            <button className="px-2 py-1 bg-red-500 text-white rounded hover:bg-red-600">Install Recommended PEP</button>
            <button className="px-2 py-1 bg-muted text-muted-foreground rounded hover:bg-muted/80">Fallback to Observe Only</button>
          </div>
        </div>
      )}
    </div>
  );
}
