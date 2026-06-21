import { useState } from "react";
import { Users, Cpu } from "lucide-react";
import { Agents } from "../Agents";
import { BlackboxAI } from "../BlackboxAI";

export function AgentsAndModels() {
  const [activeTab, setActiveTab] = useState<"agents" | "models">("agents");

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">Agents & Models</h2>
          <p className="text-muted-foreground">
            Manage your AI Ecosystem: Authorized agents and external model
            providers.
          </p>
        </div>
      </div>

      <div className="flex space-x-1 border-b border-border/50">
        <button
          onClick={() => setActiveTab("agents")}
          className={`flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            activeTab === "agents"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground hover:border-border"
          }`}
        >
          <Users className="h-4 w-4" />
          AI Agents
        </button>
        <button
          onClick={() => setActiveTab("models")}
          className={`flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            activeTab === "models"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground hover:border-border"
          }`}
        >
          <Cpu className="h-4 w-4" />
          LLM Providers (Blackbox AI)
        </button>
      </div>

      <div className="pt-2">
        {activeTab === "agents" ? (
          <div className="mt-[-24px]">
            <Agents hideHeader={true} />
          </div>
        ) : (
          <div className="mt-[-24px]">
            <BlackboxAI hideHeader={true} />
          </div>
        )}
      </div>
    </div>
  );
}
