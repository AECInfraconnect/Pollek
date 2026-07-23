import { Navigate } from "react-router-dom";
import type { ReactNode } from "react";
import { Overview } from "@/pages/Overview";
import { SimpleOverviewPage } from "@/pages/SimpleOverviewPage";
import { Resources } from "@/pages/Resources";
import { Simulator } from "@/pages/Simulator";
import { Bundles } from "@/pages/Bundles";
import { Settings } from "@/pages/Settings";
import { PdpRoutingPage } from "@/pages/PdpRoutingPage";
import { AutoDiscovery } from "@/pages/AutoDiscovery";
import { PolicySuggestions } from "@/pages/PolicySuggestions";
import { CostLedger } from "@/pages/CostLedger";
import { PolicyPresets } from "@/pages/PolicyPresets";
import { Protect } from "@/pages/Protect";
import { useMode } from "@/context/ModeContext";
import { Integrations } from "@/pages/Ecosystem/Integrations";
import { PluginMarketplace } from "@/pages/Ecosystem/PluginMarketplace";
import { IdentityNetwork } from "@/pages/Data/IdentityNetwork";
import { AlertsAndShadowAI } from "@/pages/Monitoring/AlertsAndShadowAI";
import { Entities } from "@/pages/Entities";
import { ActivityTimelineV2 } from "@/features/activity/ActivityTimelineV2";
import { EntityGraphPage } from "@/features/entity-graph/EntityGraphPage";
import { Deployments } from "@/pages/Deployments";
import { LocalEvidence } from "@/pages/LocalEvidence";
import { ControlMethods } from "@/pages/ControlMethods";
import { Capabilities } from "@/pages/Capabilities";
import { Health } from "@/pages/Health";
import { AiActivityPage } from "@/pages/AiActivityPage";
import { AllowedBlockedPage } from "@/pages/AllowedBlockedPage";
import { DataAndAppsPage } from "@/pages/DataAndAppsPage";
import { HistoryReportsPage } from "@/pages/HistoryReportsPage";
import { MyAiAppsPage } from "@/pages/MyAiAppsPage";
import { SetupCapabilitiesPage } from "@/pages/SetupCapabilitiesPage";
import { DetectionCoveragePage } from "@/pages/DetectionCoveragePage";
import { SignalCorrelation } from "@/pages/SignalCorrelation";
import { CloudContract } from "@/pages/CloudContract";
import { DefinitionsHotReload } from "@/pages/DefinitionsHotReload";
import { WorkloadIdentity } from "@/pages/WorkloadIdentity";
import AgentsV2 from "@/pages/AgentsV2";
import ToolsResourcesV2 from "@/pages/ToolsResourcesV2";
import PoliciesV2 from "@/pages/PoliciesV2";

export interface DashboardRoute {
  key: string;
  path?: string;
  index?: boolean;
  element: ReactNode;
}

function redirect(to: string) {
  return <Navigate to={to} replace />;
}

export function HomeRoute() {
  const { mode } = useMode();
  return mode === "desktop_simple" ? <SimpleOverviewPage /> : <Overview />;
}

export const dashboardRoutes: DashboardRoute[] = [
  { key: "home", index: true, element: <HomeRoute /> },

  { key: "scan", path: "scan", element: <AutoDiscovery /> },
  { key: "my-ai-apps", path: "my-ai-apps", element: <MyAiAppsPage /> },
  { key: "data-apps", path: "data-apps", element: <DataAndAppsPage /> },
  {
    key: "allowed-blocked",
    path: "allowed-blocked",
    element: <AllowedBlockedPage />,
  },
  { key: "setup", path: "setup", element: <SetupCapabilitiesPage /> },
  { key: "history", path: "history", element: <HistoryReportsPage /> },
  { key: "offline-scan", path: "offline-scan", element: <LocalEvidence /> },
  {
    key: "recommended-policies",
    path: "recommended-policies",
    element: <PolicySuggestions />,
  },
  { key: "policy-feasibility", path: "policy-feasibility", element: <Protect /> },
  { key: "deployments", path: "deployments", element: <Deployments /> },
  { key: "control-methods", path: "control-methods", element: <ControlMethods /> },
  { key: "capabilities", path: "capabilities", element: <Capabilities /> },
  { key: "pep-layers", path: "pep-layers", element: redirect("/capabilities") },
  { key: "pdp-engines", path: "pdp-engines", element: redirect("/capabilities") },
  {
    key: "timeline",
    path: "timeline",
    element: redirect("/activity-timeline"),
  },
  { key: "local-evidence", path: "local-evidence", element: <LocalEvidence /> },
  { key: "health", path: "health", element: <Health /> },
  { key: "entity-graph", path: "entity-graph", element: <EntityGraphPage /> },

  { key: "agents", path: "agents", element: <AgentsV2 /> },
  { key: "integrations", path: "integrations", element: <Integrations /> },
  {
    key: "plugin-marketplace",
    path: "plugin-marketplace",
    element: <PluginMarketplace />,
  },
  { key: "tools", path: "tools", element: <ToolsResourcesV2 /> },

  { key: "resources", path: "resources", element: <Resources /> },
  { key: "identities", path: "identities", element: <IdentityNetwork /> },

  { key: "protect", path: "protect", element: <Protect /> },
  { key: "policy-presets", path: "policy-presets", element: <PolicyPresets /> },
  {
    key: "policy-suggestions",
    path: "policy-suggestions",
    element: <PolicySuggestions />,
  },
  { key: "policies", path: "policies", element: <PoliciesV2 /> },
  { key: "simulator", path: "simulator", element: <Simulator /> },

  { key: "activity", path: "activity", element: <AiActivityPage /> },
  {
    key: "observe-coverage",
    path: "observe-coverage",
    element: <DetectionCoveragePage />,
  },
  {
    key: "signal-correlation",
    path: "signal-correlation",
    element: <SignalCorrelation />,
  },
  {
    key: "cloud-contract",
    path: "cloud-contract",
    element: <CloudContract />,
  },
  {
    key: "definitions-hot-reload",
    path: "definitions-hot-reload",
    element: <DefinitionsHotReload />,
  },
  {
    key: "workload-identity",
    path: "workload-identity",
    element: <WorkloadIdentity />,
  },
  {
    key: "activity-timeline",
    path: "activity-timeline",
    element: <ActivityTimelineV2 />,
  },
  { key: "alerts", path: "alerts", element: <AlertsAndShadowAI /> },
  { key: "audit", path: "audit", element: redirect("/activity-timeline") },
  {
    key: "decision-logs",
    path: "decision-logs",
    element: redirect("/activity-timeline"),
  },
  { key: "cost-ledger", path: "cost-ledger", element: <CostLedger /> },
  { key: "cost", path: "cost", element: redirect("/cost-ledger") },
  {
    key: "activity-log",
    path: "activity-log",
    element: redirect("/activity"),
  },

  { key: "bundles", path: "bundles", element: <Bundles /> },
  { key: "discovery", path: "discovery", element: <AutoDiscovery /> },
  { key: "settings", path: "settings", element: <Settings /> },
  { key: "settings-pdp", path: "settings/pdp", element: <PdpRoutingPage /> },

  { key: "blackbox-ai", path: "blackbox-ai", element: redirect("/agents") },
  { key: "servers", path: "servers", element: redirect("/integrations") },
  { key: "entities", path: "entities", element: <Entities /> },
  {
    key: "relationships",
    path: "relationships",
    element: redirect("/entity-graph"),
  },
  { key: "shadow-ai", path: "shadow-ai", element: redirect("/alerts") },
];
