import { useState, useEffect } from "react";
import { Lightbulb, RefreshCw, AlertTriangle, ShieldCheck } from "lucide-react";
import { PolicySuggestionApi } from "../services/api";

export function PolicySuggestions() {
  const [loading, setLoading] = useState(false);
  const [suggestions, setSuggestions] = useState<any[]>([]);

  const fetchSuggestions = async () => {
    try {
      const data = await PolicySuggestionApi.list();
      setSuggestions(data);
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    fetchSuggestions();
  }, []);

  const generateSuggestions = async () => {
    setLoading(true);
    try {
      await PolicySuggestionApi.generate();
      await fetchSuggestions();
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">
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

      <div className="glass rounded-xl p-6">
        <h3 className="font-semibold mb-4">Suggested Policies</h3>
        {suggestions.length === 0 ? (
          <div className="flex h-[200px] items-center justify-center rounded-md border border-dashed border-muted">
            <p className="text-sm text-muted-foreground">
              No suggestions generated yet. Click "Generate Suggestions" to run
              the engine.
            </p>
          </div>
        ) : (
          <div className="space-y-4">
            {suggestions.map((s, idx) => (
              <div
                key={idx}
                className="border rounded-lg p-4 bg-muted/20 hover:bg-muted/40 transition-colors"
              >
                <div className="flex justify-between items-start mb-2">
                  <div className="flex items-center gap-2">
                    {s.severity === "high" || s.severity === "critical" ? (
                      <AlertTriangle className="h-5 w-5 text-destructive" />
                    ) : (
                      <ShieldCheck className="h-5 w-5 text-emerald-500" />
                    )}
                    <h4 className="font-medium text-lg">{s.title}</h4>
                  </div>
                  <span
                    className={`px-2 py-1 text-xs rounded-full ${s.severity === "high" ? "bg-destructive/10 text-destructive" : "bg-amber-500/10 text-amber-500"}`}
                  >
                    {s.severity} severity
                  </span>
                </div>
                <p className="text-muted-foreground text-sm mb-4">
                  {s.summary}
                </p>
                <div className="flex items-center gap-2 mb-4">
                  <span className="text-xs font-medium text-muted-foreground">
                    Recommended PEP:
                  </span>
                  <span className="inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium bg-muted text-foreground">
                    {s.recommended_pep_type || "Unknown"}
                  </span>
                </div>

                {s.artifacts && s.artifacts.length > 0 && (
                  <div className="mt-4 space-y-4">
                    <h5 className="text-sm font-medium">Policy Artifacts:</h5>
                    <div className="grid grid-cols-1 gap-4">
                      {s.artifacts.map((art: any, i: number) => (
                        <div
                          key={i}
                          className="bg-background rounded-md border overflow-hidden"
                        >
                          <div className="bg-muted px-4 py-2 border-b flex justify-between items-center">
                            <span className="text-xs font-medium uppercase">
                              {art.language}
                            </span>
                            <span className="text-xs text-muted-foreground font-mono">
                              {art.name}
                            </span>
                          </div>
                          <pre className="p-4 text-xs font-mono overflow-x-auto whitespace-pre">
                            {art.content}
                          </pre>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
