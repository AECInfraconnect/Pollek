import { useEffect, useState } from "react";
import { AlertCircle, ShieldAlert } from "lucide-react";

import { TelemetryApi } from "../services/api";

export function Alerts({ hideHeader = false }: { hideHeader?: boolean }) {
  const [denyCount, setDenyCount] = useState(0);

  useEffect(() => {
    // Poll telemetry logs periodically to check for Deny decisions in the last minute
    const checkAlerts = async () => {
      try {
        const data = await TelemetryApi.listDecisionLogs();

        const oneMinuteAgo = new Date(Date.now() - 60000);
        let recentDenies = 0;

        for (const item of data) {
          const timestamp = new Date(item.timestamp);
          const decisionStr = item.payload?.decision?.toString().toLowerCase();
          if (
            timestamp > oneMinuteAgo &&
            (decisionStr === "deny" || decisionStr === "false")
          ) {
            recentDenies++;
          }
        }
        setDenyCount(recentDenies);
      } catch (err) {
        console.error("Failed to check alerts:", err);
      }
    };

    checkAlerts();
    const interval = setInterval(checkAlerts, 5000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="space-y-6">
      {!hideHeader && (
        <div>
          <h1 className="text-lg font-semibold tracking-tight">Alerts</h1>
          <p className="text-muted-foreground mt-2">
            Local edge alerts and recent policy violations.
          </p>
        </div>
      )}

      <div className="grid gap-6">
        {denyCount > 5 ? (
          <div className="flex items-start gap-3 rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-3">
            <ShieldAlert className="h-4 w-4 text-destructive shrink-0 mt-0.5" />
            <div>
              <h3 className="font-semibold text-destructive text-sm">
                High Deny Rate Detected
              </h3>
              <p className="mt-0.5 text-xs text-muted-foreground">
                {denyCount} Access Denied decisions in the last minute.
              </p>
            </div>
          </div>
        ) : (
          <div className="flex items-center gap-3 rounded-lg border border-dashed px-4 py-4 text-center text-muted-foreground bg-card/30">
            <div className="flex flex-col items-center justify-center w-full">
              <AlertCircle className="h-5 w-5 mb-2 opacity-50" />
              <p className="text-sm">No active alerts. System operating normally.</p>
              <p className="text-[10px] opacity-75 mt-0.5">
                Alert triggers when Deny decisions exceed 5/min.
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
