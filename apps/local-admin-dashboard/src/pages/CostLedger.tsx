import { useState, useEffect } from "react";
import { DollarSign, RefreshCw } from "lucide-react";
import { ObservationApi } from "../services/api";

export function CostLedger() {
  const [loading, setLoading] = useState(false);
  const [totalCost, setTotalCost] = useState<number>(0);

  const fetchCost = async () => {
    setLoading(true);
    try {
      const data = await ObservationApi.getCostSummary();
      setTotalCost((data as any).total_cost || 0);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchCost();
  }, []);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">
            Token & Cost Ledger
          </h2>
          <p className="text-muted-foreground">
            Monitor estimated costs and token usage across all observed AI
            agents.
          </p>
        </div>
        <button
          onClick={fetchCost}
          disabled={loading}
          className="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:opacity-50 disabled:pointer-events-none ring-offset-background bg-primary text-primary-foreground hover:bg-primary/90 h-10 py-2 px-4"
        >
          {loading ? (
            <RefreshCw className="mr-2 h-4 w-4 animate-spin" />
          ) : (
            <RefreshCw className="mr-2 h-4 w-4" />
          )}
          Refresh
        </button>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <div className="glass rounded-xl p-6 relative overflow-hidden group">
          <div className="relative flex items-center justify-between">
            <span className="text-sm font-medium text-muted-foreground">
              Total Estimated Cost
            </span>
            <DollarSign className="h-4 w-4 text-muted-foreground" />
          </div>
          <div className="mt-4 flex items-baseline gap-2">
            <span className="text-3xl font-bold">${totalCost.toFixed(2)}</span>
            <span className="text-xs font-medium text-muted-foreground">
              USD
            </span>
          </div>
        </div>
      </div>

      <div className="glass rounded-xl p-6">
        <h3 className="font-semibold mb-4">Cost Breakdown by Agent</h3>
        <div className="flex h-[200px] items-center justify-center rounded-md border border-dashed border-muted">
          <p className="text-sm text-muted-foreground">
            No detailed breakdowns yet. Configure dek-agent-observer to capture
            provider usage.
          </p>
        </div>
      </div>
    </div>
  );
}
