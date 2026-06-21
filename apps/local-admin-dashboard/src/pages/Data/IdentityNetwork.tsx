import { useState } from "react";
import { UserCircle, Network } from "lucide-react";
import { Entities } from "../Entities";
import { Relationships } from "../Relationships";

export function IdentityNetwork() {
  const [activeTab, setActiveTab] = useState<"entities" | "relationships">("entities");

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">Identity & Network</h2>
          <p className="text-muted-foreground">
            Manage your local identity graph: people, systems, and their relationships.
          </p>
        </div>
      </div>

      <div className="flex space-x-1 border-b border-border/50">
        <button
          onClick={() => setActiveTab("entities")}
          className={`flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            activeTab === "entities"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground hover:border-border"
          }`}
        >
          <UserCircle className="h-4 w-4" />
          Entities
        </button>
        <button
          onClick={() => setActiveTab("relationships")}
          className={`flex items-center gap-2 px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            activeTab === "relationships"
              ? "border-primary text-primary"
              : "border-transparent text-muted-foreground hover:text-foreground hover:border-border"
          }`}
        >
          <Network className="h-4 w-4" />
          Relationships
        </button>
      </div>

      <div className="pt-2">
        {activeTab === "entities" ? (
          <div className="mt-[-24px]">
            <Entities hideHeader={true} />
          </div>
        ) : (
          <div className="mt-[-24px]">
            <Relationships hideHeader={true} />
          </div>
        )}
      </div>
    </div>
  );
}
