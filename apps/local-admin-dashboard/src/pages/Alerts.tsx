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
          <h1 className="text-3xl font-bold tracking-tight">Alerts</h1>
          <p className="text-muted-foreground mt-2">
            Local edge alerts and recent policy violations.
          </p>
        </div>
      )}

      <div className="grid gap-6">
        {denyCount > 5 ? (
          <div className="flex items-start gap-4 rounded-xl border border-destructive/50 bg-destructive/10 p-6 shadow-sm">
            <ShieldAlert className="h-6 w-6 text-destructive shrink-0 mt-1" />
            <div>
              <h3 className="font-semibold text-destructive text-lg">
                High Deny Rate Detected
              </h3>
              <p className="mt-1 text-sm text-muted-foreground">
                There have been {denyCount} Access Denied decisions in the last
                minute. This could indicate an ongoing attack or misconfigured
                policy.
              </p>
            </div>
          </div>
        ) : (
          <div className="flex items-center gap-4 rounded-xl border border-dashed p-8 text-center text-muted-foreground bg-card/30">
            <div className="flex flex-col items-center justify-center w-full">
              <AlertCircle className="h-8 w-8 mb-3 opacity-50" />
              <p>No active alerts. The system is operating normally.</p>
              <p className="text-xs opacity-75 mt-1">
                Note: Local edge will trigger an alert if "Deny" decisions
                exceed 5 per minute.
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
