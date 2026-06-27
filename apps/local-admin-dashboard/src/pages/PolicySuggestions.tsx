import { useState, useEffect } from "react";
import { Lightbulb, RefreshCw, AlertTriangle } from "lucide-react";
import { PolicyFirstApi } from "../services/api";

export function PolicySuggestions() {
  const [loading, setLoading] = useState(false);
  const [suggestions, setSuggestions] = useState<any[]>([]);

  const fetchSuggestions = async () => {
    try {
      setLoading(true);
      const data = await PolicyFirstApi.getPolicySuggestions();
      setSuggestions(data);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchSuggestions();
  }, []);

  const generateSuggestions = async () => {
    try {
      setLoading(true);
      await PolicyFirstApi.generatePolicySuggestions();
      await fetchSuggestions();
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold tracking-tight">
            Policy Suggestions
          </h2>
          <p className="text-muted-foreground">
            Policies automatically suggested based on Shadow AI and Auto
            Discovery findings.
          </p>
        </div>
        <button
          onClick={generateSuggestions}
          disabled={loading}
          className="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:opacity-50 disabled:pointer-events-none ring-offset-background bg-primary text-primary-foreground hover:bg-primary/90 h-10 py-2 px-4"
        >
          {loading ? (
            <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
          ) : (
            <Lightbulb className="mr-2 h-4 w-4" />
          )}
          Generate Suggestions
        </button>
      </div>

      <div className="glass rounded-lg p-4">
        <h3 className="font-semibold mb-4">Suggested Policies</h3>
        {suggestions.length === 0 ? (
          <div className="flex h-[200px] items-center justify-center rounded-md border border-dashed border-muted">
            <p className="text-sm text-muted-foreground">
              No suggestions generated yet. Click "Generate Suggestions" to run
              the engine.
            </p>
          </div>
        ) : (
          <div className="space-y-8">
            {(() => {
              const sortedSuggestions = [...suggestions].sort(
                (a, b) =>
                  new Date(b.created_at).getTime() -
                  new Date(a.created_at).getTime(),
              );
              const latestScanTime =
                sortedSuggestions.length > 0
                  ? new Date(sortedSuggestions[0].created_at).getTime()
                  : 0;
              const latestSuggestions = sortedSuggestions.filter(
                (s) =>
                  new Date(s.created_at).getTime() > latestScanTime - 60000,
              );
              const previousSuggestions = sortedSuggestions.filter(
                (s) =>
                  new Date(s.created_at).getTime() <= latestScanTime - 60000,
              );

              const renderSuggestionList = (list: any[]) => (
                <div className="space-y-4">
                  {list.map((s: any, idx: number) => (
                    <div
                      key={`${s.suggestion_id}-${idx}`}
                      className="border rounded-lg p-4 bg-muted/20 hover:bg-muted/40 transition-colors"
                    >
                      <div className="flex justify-between items-start mb-2">
                        <div className="flex items-center gap-2">
                          <Lightbulb className="h-5 w-5 text-amber-500" />
                          <h4 className="font-medium text-lg">
                            {s.title || s.display_name?.en || s.suggestion_id}
                          </h4>
                        </div>
                        <span
                          className={`px-2 py-1 text-xs rounded-full ${s.severity === "high" || s.feasibility === "needs_setup" ? "bg-red-500/10 text-red-500" : s.severity === "medium" ? "bg-amber-500/10 text-amber-500" : "bg-emerald-500/10 text-emerald-500"}`}
                        >
                          {(s.status || s.feasibility || "draft").replace(
                            /_/g,
                            " ",
                          )}
                        </span>
                      </div>
                      <div className="space-y-1 mb-4">
                        <p className="text-muted-foreground text-sm">
                          {s.summary || s.description?.en || ""}
                        </p>
                      </div>
                      <div className="flex flex-wrap items-center gap-4 mb-4">
                        <div className="flex items-center gap-2">
                          <span className="text-xs font-medium text-muted-foreground">
                            Recommended Policy:
                          </span>
                          <span className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium bg-muted text-foreground capitalize">
                            {s.recommended_policy_type ||
                              s.recommended_control_level ||
                              "Unknown"}
                          </span>
                        </div>
                        <div className="flex items-center gap-2">
                          <span className="text-xs font-medium text-muted-foreground">
                            Confidence:
                          </span>
                          <span className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium bg-muted text-foreground">
                            {((s.confidence || 0) * 100).toFixed(0)}%
                          </span>
                        </div>
                        <div className="flex items-center gap-2">
                          <span className="text-xs font-medium text-muted-foreground">
                            Targets:
                          </span>
                          <span className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium bg-muted text-foreground">
                            {s.target_agent_id
                              ? s.target_agent_id
                              : s.target_agent_ids
                                ? s.target_agent_ids.join(", ")
                                : "None"}
                          </span>
                        </div>
                      </div>

                      {s.setup_required && s.setup_required.length > 0 && (
                        <div className="mt-4 p-3 bg-red-500/10 border border-red-500/20 rounded-md space-y-2">
                          <h5 className="text-xs font-semibold text-red-500 flex items-center gap-2">
                            <AlertTriangle className="w-4 h-4" />
                            Setup Required
                          </h5>
                          <ul className="text-xs text-red-400 list-disc list-inside">
                            {s.setup_required.map((req: any, i: number) => (
                              <li key={i}>{req.label?.en || req}</li>
                            ))}
                          </ul>
                        </div>
                      )}

                      <div className="mt-4 flex justify-end">
                        <a
                          href={`/wizard?policy=${s.suggestion_type || s.policy_template_id}&targets=${s.target_agent_id || (s.target_agent_ids ? s.target_agent_ids.join(",") : "")}`}
                          className="px-4 py-2 text-sm bg-primary text-primary-foreground rounded hover:bg-primary/90 font-medium transition-colors"
                        >
                          Deploy Policy
                        </a>
                      </div>
                    </div>
                  ))}
                </div>
              );

              return (
                <>
                  {latestSuggestions.length > 0 && (
                    <div>
                      <h4 className="font-medium text-sm text-primary mb-3 flex items-center gap-2">
                        <div className="w-2 h-2 rounded-full bg-primary animate-pulse" />
                        Latest Suggestions
                      </h4>
                      {renderSuggestionList(latestSuggestions)}
                    </div>
                  )}

                  {previousSuggestions.length > 0 && (
                    <div className="pt-4 border-t">
                      <h4 className="font-medium text-sm text-muted-foreground mb-3">
                        Previous Suggestions
                      </h4>
                      {renderSuggestionList(previousSuggestions)}
                    </div>
                  )}
                </>
              );
            })()}
          </div>
        )}
      </div>
    </div>
  );
}
