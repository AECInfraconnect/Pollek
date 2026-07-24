import { Navigate } from "react-router-dom";
import { lazy, type ComponentType, type ReactNode } from "react";
import { useMode } from "@/context/ModeContext";

/**
 * Route-based code splitting: each page is loaded on demand as its own chunk
 * instead of being bundled into the initial download. A `<Suspense>` boundary
 * in `DashboardLayout` renders a fallback while a page chunk loads.
 *
 * `lazyNamed` adapts our named page exports to the default-export shape that
 * `React.lazy` requires.
 */
function lazyNamed<M extends Record<string, unknown>, K extends keyof M>(
  loader: () => Promise<M>,
  name: K,
) {
  return lazy(async () => {
    const module = await loader();
    return { default: module[name] as ComponentType };
  });
}

const Overview = lazyNamed(() => import("@/pages/Overview"), "Overview");
const SimpleOverviewPage = lazyNamed(
  () => import("@/pages/SimpleOverviewPage"),
  "SimpleOverviewPage",
);
const Resources = lazyNamed(() => import("@/pages/Resources"), "Resources");
const Simulator = lazyNamed(() => import("@/pages/Simulator"), "Simulator");
const Bundles = lazyNamed(() => import("@/pages/Bundles"), "Bundles");
const Settings = lazyNamed(() => import("@/pages/Settings"), "Settings");
const PdpRoutingPage = lazyNamed(
  () => import("@/pages/PdpRoutingPage"),
  "PdpRoutingPage",
);
const AutoDiscovery = lazyNamed(
  () => import("@/pages/AutoDiscovery"),
  "AutoDiscovery",
);
const PolicySuggestions = lazyNamed(
  () => import("@/pages/PolicySuggestions"),
  "PolicySuggestions",
);
const CostLedger = lazyNamed(() => import("@/pages/CostLedger"), "CostLedger");
const PolicyPresets = lazyNamed(
  () => import("@/pages/PolicyPresets"),
  "PolicyPresets",
);
const Protect = lazyNamed(() => import("@/pages/Protect"), "Protect");
const Integrations = lazyNamed(
  () => import("@/pages/Ecosystem/Integrations"),
  "Integrations",
);
const PluginMarketplace = lazyNamed(
  () => import("@/pages/Ecosystem/PluginMarketplace"),
  "PluginMarketplace",
);
const IdentityNetwork = lazyNamed(
  () => import("@/pages/Data/IdentityNetwork"),
  "IdentityNetwork",
);
const AlertsAndShadowAI = lazyNamed(
  () => import("@/pages/Monitoring/AlertsAndShadowAI"),
  "AlertsAndShadowAI",
);
const Entities = lazyNamed(() => import("@/pages/Entities"), "Entities");
const ActivityTimelineV2 = lazyNamed(
  () => import("@/features/activity/ActivityTimelineV2"),
  "ActivityTimelineV2",
);
const EntityGraphPage = lazyNamed(
  () => import("@/features/entity-graph/EntityGraphPage"),
  "EntityGraphPage",
);
const Deployments = lazyNamed(
  () => import("@/pages/Deployments"),
  "Deployments",
);
const LocalEvidence = lazyNamed(
  () => import("@/pages/LocalEvidence"),
  "LocalEvidence",
);
const ControlMethods = lazyNamed(
  () => import("@/pages/ControlMethods"),
  "ControlMethods",
);
const Capabilities = lazyNamed(
  () => import("@/pages/Capabilities"),
  "Capabilities",
);
const Health = lazyNamed(() => import("@/pages/Health"), "Health");
const AiActivityPage = lazyNamed(
  () => import("@/pages/AiActivityPage"),
  "AiActivityPage",
);
const AllowedBlockedPage = lazyNamed(
  () => import("@/pages/AllowedBlockedPage"),
  "AllowedBlockedPage",
);
const DataAndAppsPage = lazyNamed(
  () => import("@/pages/DataAndAppsPage"),
  "DataAndAppsPage",
);
const HistoryReportsPage = lazyNamed(
  () => import("@/pages/HistoryReportsPage"),
  "HistoryReportsPage",
);
const MyAiAppsPage = lazyNamed(
  () => import("@/pages/MyAiAppsPage"),
  "MyAiAppsPage",
);
const SetupCapabilitiesPage = lazyNamed(
  () => import("@/pages/SetupCapabilitiesPage"),
  "SetupCapabilitiesPage",
);
const DetectionCoveragePage = lazyNamed(
  () => import("@/pages/DetectionCoveragePage"),
  "DetectionCoveragePage",
);
const SignalCorrelation = lazyNamed(
  () => import("@/pages/SignalCorrelation"),
  "SignalCorrelation",
);
const CloudContract = lazyNamed(
  () => import("@/pages/CloudContract"),
  "CloudContract",
);
const DefinitionsHotReload = lazyNamed(
  () => import("@/pages/DefinitionsHotReload"),
  "DefinitionsHotReload",
);
const WorkloadIdentity = lazyNamed(
  () => import("@/pages/WorkloadIdentity"),
  "WorkloadIdentity",
);
const TrustProvenance = lazyNamed(
  () => import("@/pages/TrustProvenance"),
  "TrustProvenance",
);
const AgentsV2 = lazy(() => import("@/pages/AgentsV2"));
const ToolsResourcesV2 = lazy(() => import("@/pages/ToolsResourcesV2"));
const PoliciesV2 = lazy(() => import("@/pages/PoliciesV2"));

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
    key: "trust-provenance",
    path: "trust-provenance",
    element: <TrustProvenance />,
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
