import { useState } from "react";
import { RefreshCw, Search } from "lucide-react";

export function AutoDiscovery() {
  const [loading, setLoading] = useState(false);

  const triggerScan = async () => {
    setLoading(true);
    try {
      await fetch("http://127.0.0.1:3000/v1/tenants/local/agent-discovery/scan", {
        method: "POST"
      });
      alert("Scan completed!");
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
          <h2 className="text-2xl font-bold tracking-tight">Auto Discovery</h2>
          <p className="text-muted-foreground">
            Agents found running on this device that might need to be registered.
          </p>
        </div>
        <button 
          onClick={triggerScan} 
          disabled={loading}
          className="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:opacity-50 disabled:pointer-events-none ring-offset-background bg-primary text-primary-foreground hover:bg-primary/90 h-10 py-2 px-4"
        >
          {loading ? <RefreshCw className="mr-2 h-4 w-4 animate-spin" /> : <Search className="mr-2 h-4 w-4" />}
          Scan Now
        </button>
      </div>

      <div className="glass rounded-xl p-6">
        <h3 className="font-semibold mb-4">Discovered Agents</h3>
        <div className="flex h-[200px] items-center justify-center rounded-md border border-dashed border-muted">
          <p className="text-sm text-muted-foreground">
            No discovered agents yet. Click "Scan Now" to begin.
          </p>
        </div>
      </div>
    </div>
  );
}
