import { useState } from "react";
import { Server, Wrench } from "lucide-react";
import { Servers } from "../Servers";
import { Tools } from "../Tools";

export function Integrations() {
  const [activeTab, setActiveTab] = useState<"servers" | "tools">("servers");

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">Integrations</h2>
          <p className="text-muted-foreground">
            Manage Model Context Protocol (MCP) servers and their provided
            tools.
          </p>
        </div>
      </div>

      <div className="flex space-x-1 border-b border-border/50">
        <button
          onClick={() => setActiveTab("servers")}
          className={`flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            activeTab === "servers"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground hover:border-border"
          }`}
        >
          <Server className="h-4 w-4" />
          MCP Servers
        </button>
        <button
          onClick={() => setActiveTab("tools")}
          className={`flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            activeTab === "tools"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground hover:border-border"
          }`}
        >
          <Wrench className="h-4 w-4" />
          Tools
        </button>
      </div>

      <div className="pt-2">
        {activeTab === "servers" ? (
          <div className="mt-[-24px]">
            <Servers hideHeader={true} />
          </div>
        ) : (
          <div className="mt-[-24px]">
            <Tools hideHeader={true} />
          </div>
        )}
      </div>
    </div>
  );
}
