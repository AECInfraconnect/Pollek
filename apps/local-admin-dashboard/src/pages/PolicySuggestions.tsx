import { useState } from "react";
import { Lightbulb, RefreshCw } from "lucide-react";

export function PolicySuggestions() {
  const [loading, setLoading] = useState(false);

  const generateSuggestions = async () => {
    setLoading(true);
    try {
      await fetch("http://127.0.0.1:3000/v1/tenants/local/policy-suggestions/generate", {
        method: "POST"
      });
      alert("Suggestions generated! Refreshing list...");
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
          <h2 className="text-2xl font-bold tracking-tight">Policy Suggestions</h2>
          <p className="text-muted-foreground">
            Policies automatically suggested based on Shadow AI and Auto Discovery findings.
          </p>
        </div>
        <button 
          onClick={generateSuggestions} 
          disabled={loading}
          className="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:opacity-50 disabled:pointer-events-none ring-offset-background bg-primary text-primary-foreground hover:bg-primary/90 h-10 py-2 px-4"
        >
          {loading ? <RefreshCw className="mr-2 h-4 w-4 animate-spin" /> : <Lightbulb className="mr-2 h-4 w-4" />}
          Generate Suggestions
        </button>
      </div>

      <div className="glass rounded-xl p-6">
        <h3 className="font-semibold mb-4">Suggested Policies</h3>
        <div className="flex h-[200px] items-center justify-center rounded-md border border-dashed border-muted">
          <p className="text-sm text-muted-foreground">
            No suggestions generated yet. Click "Generate Suggestions" to run the engine.
          </p>
        </div>
      </div>
    </div>
  );
}
