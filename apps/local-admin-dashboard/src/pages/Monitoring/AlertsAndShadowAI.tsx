import { useState } from "react";
import { ShieldAlert, AlertTriangle } from "lucide-react";
import { Alerts } from "../Alerts";
import { ShadowAI } from "../ShadowAI";

export function AlertsAndShadowAI() {
  const [activeTab, setActiveTab] = useState<"alerts" | "shadow">("alerts");

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">
            Alerts & Shadow AI
          </h2>
          <p className="text-muted-foreground">
            Monitor policy violations, security alerts, and unregistered AI
            activities.
          </p>
        </div>
      </div>

      <div className="flex space-x-1 border-b border-border/50">
        <button
          onClick={() => setActiveTab("alerts")}
          className={`flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            activeTab === "alerts"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground hover:border-border"
          }`}
        >
          <ShieldAlert className="h-4 w-4" />
          Active Alerts
        </button>
        <button
          onClick={() => setActiveTab("shadow")}
          className={`flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            activeTab === "shadow"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground hover:border-border"
          }`}
        >
          <AlertTriangle className="h-4 w-4" />
          Shadow AI Inbox
        </button>
      </div>

      <div className="pt-2">
        {activeTab === "alerts" ? (
          <div className="mt-[-24px]">
            <Alerts hideHeader={true} />
          </div>
        ) : (
          <div className="mt-[-24px]">
            <ShadowAI hideHeader={true} />
          </div>
        )}
      </div>
    </div>
  );
}
