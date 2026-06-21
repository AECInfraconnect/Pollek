import { useState, useEffect } from "react";
import { RefreshCw, Search, ShieldAlert } from "lucide-react";
import { RegistryApi } from "../services/api";

export function AutoDiscovery() {
  const [candidates, setCandidates] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);
  const [loadingCandidates, setLoadingCandidates] = useState(true);

  const fetchCandidates = () => {
    setLoadingCandidates(true);
    RegistryApi.listDiscoveryCandidates()
      .then(setCandidates)
      .catch(console.error)
      .finally(() => setLoadingCandidates(false));
  };

  useEffect(() => {
    fetchCandidates();
  }, []);

  const triggerScan = async () => {
    setLoading(true);
    try {
      await RegistryApi.triggerDiscoveryScan();
      alert("Scan completed!");
      fetchCandidates();
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
          className="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:opacity-50 disabled:pointer-events-none ring-offset-background bg-primary text-primary-foreground hover:bg-primary/90 h-10 py-2 px-4 shadow-lg shadow-primary/20"
        >
          {loading ? <RefreshCw className="mr-2 h-4 w-4 animate-spin" /> : <Search className="mr-2 h-4 w-4" />}
          Scan Now
        </button>
      </div>

      <div className="glass rounded-xl p-6">
        <h3 className="font-semibold mb-4">Discovered Agents</h3>
        {loadingCandidates ? (
          <div className="flex h-[200px] items-center justify-center rounded-md border border-dashed border-muted">
            <p className="text-sm text-muted-foreground">Loading candidates...</p>
          </div>
        ) : candidates.length === 0 ? (
          <div className="flex h-[200px] items-center justify-center rounded-md border border-dashed border-muted">
            <p className="text-sm text-muted-foreground">
              No discovered agents yet. Click "Scan Now" to begin.
            </p>
          </div>
        ) : (
          <div className="space-y-4">
            {candidates.map((c, idx) => (
              <div key={idx} className="border rounded-lg p-4 hover:bg-muted/30 transition-colors">
                <div className="flex justify-between items-start mb-2">
                  <div className="flex items-center gap-2">
                    <ShieldAlert className="h-5 w-5 text-primary" />
                    <h4 className="font-medium text-lg">Candidate: {c.candidate_id}</h4>
                  </div>
                </div>
                <p className="text-muted-foreground text-sm">
                  First seen: {new Date(c.first_seen).toLocaleString()} <br/>
                  Last seen: {new Date(c.last_seen).toLocaleString()} <br/>
                  {c.heuristics_matched && `Heuristics matched: ${c.heuristics_matched.join(', ')}`}
                </p>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
