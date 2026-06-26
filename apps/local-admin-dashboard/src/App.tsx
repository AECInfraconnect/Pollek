import {
  BrowserRouter as Router,
  Routes,
  Route,
  Navigate,
  Outlet,
} from "react-router-dom";
import { DashboardLayout } from "./components/layout/DashboardLayout";
import { Overview } from "./pages/Overview";
import { Resources } from "./pages/Resources";
import { Policies } from "./pages/Policies";
import { Simulator } from "./pages/Simulator";
import { Bundles } from "./pages/Bundles";
import { Settings } from "./pages/Settings";
import { PdpRoutingPage } from "./pages/PdpRoutingPage";
import { AutoDiscovery } from "./pages/AutoDiscovery";
import { PolicySuggestions } from "./pages/PolicySuggestions";
import { CostLedger } from "./pages/CostLedger";
import { PolicyPresets } from "./pages/PolicyPresets";
import { Wizard } from "./pages/Wizard";
import { ModeProvider, useMode } from "./context/ModeContext";
import { Protect } from "./pages/Protect";

// Merged composite pages
import { AgentsAndModels } from "./pages/Ecosystem/AgentsAndModels";
import { Integrations } from "./pages/Ecosystem/Integrations";
import { PluginMarketplace } from "./pages/Ecosystem/PluginMarketplace";
import { IdentityNetwork } from "./pages/Data/IdentityNetwork";
import { AlertsAndShadowAI } from "./pages/Monitoring/AlertsAndShadowAI";
import { Entities } from "./pages/Entities";
import { Tools } from "./pages/Tools";
import { ActivityTimelineV2 } from "./features/activity/ActivityTimelineV2";
import { EntityGraphPage } from "./features/entity-graph/EntityGraphPage";

import { Deployments } from "./pages/Deployments";
import { LocalEvidence } from "./pages/LocalEvidence";
import { ControlMethods } from "./pages/ControlMethods";
import { Capabilities } from "./pages/Capabilities";
import { Health } from "./pages/Health";

import { Toaster } from "sonner";

// Placeholders for new structure
const Placeholder = ({ name }: { name: string }) => (
  <div className="p-8">
    <h1>{name}</h1>
  </div>
);

const ModeGuard = () => {
  const { mode } = useMode();
  // Check if current route is allowed in current mode.
  // We can do a simplistic check: if simple mode, deny known advanced paths.
  // The requirements say: filter by useMode: simple hides PDP/Routing/Bundles/Presets/Identities/Tools.
  if (mode === "desktop_simple") {
    const path = window.location.pathname;
    if (
      path.includes("pdp-engines") ||
      path.includes("pep-layers") ||
      path.includes("bundles") ||
      path.includes("policy-presets") ||
      path.includes("identities") ||
      path.includes("tools")
    ) {
      return <Navigate to="/" replace />;
    }
  }
  return <Outlet />;
};

import { ConfirmProvider } from "./components/ui/ConfirmDialog";

function App() {
  return (
    <ModeProvider>
      <ConfirmProvider>
        <Toaster position="top-right" theme="system" />
        <Router>
          <Routes>
            <Route path="/" element={<DashboardLayout />}>
              <Route element={<ModeGuard />}>
                <Route index element={<Overview />} />

                {/* New Navigation Routes */}
                <Route path="scan" element={<AutoDiscovery />} />
                <Route path="offline-scan" element={<LocalEvidence />} />
                <Route
                  path="recommended-policies"
                  element={<PolicySuggestions />}
                />
                <Route path="policy-feasibility" element={<Protect />} />
                <Route path="deployments" element={<Deployments />} />
                <Route path="control-methods" element={<ControlMethods />} />
                <Route path="capabilities" element={<Capabilities />} />
                <Route
                  path="pep-layers"
                  element={<Placeholder name="PEP Layers" />}
                />
                <Route
                  path="pdp-engines"
                  element={<Placeholder name="PDP Engines" />}
                />
                <Route
                  path="timeline"
                  element={<Navigate to="/activity-timeline" replace />}
                />
                <Route path="local-evidence" element={<LocalEvidence />} />
                <Route path="health" element={<Health />} />
                <Route path="entity-graph" element={<EntityGraphPage />} />

                {/* AI Ecosystem */}
                <Route path="agents" element={<AgentsAndModels />} />
                <Route path="integrations" element={<Integrations />} />
                <Route
                  path="plugin-marketplace"
                  element={<PluginMarketplace />}
                />
                <Route path="tools" element={<Tools />} />

                {/* Data & Context */}
                <Route path="resources" element={<Resources />} />
                <Route path="identities" element={<IdentityNetwork />} />

                {/* Security & Guardrails */}
                <Route path="protect" element={<Protect />} />
                <Route path="policy-presets" element={<PolicyPresets />} />
                <Route
                  path="policy-suggestions"
                  element={<PolicySuggestions />}
                />
                <Route path="policies" element={<Policies />} />
                <Route path="simulator" element={<Simulator />} />

                {/* Monitoring & Activity */}
                <Route
                  path="activity"
                  element={<Navigate to="/activity-timeline" replace />}
                />
                <Route
                  path="activity-timeline"
                  element={<ActivityTimelineV2 />}
                />
                <Route path="alerts" element={<AlertsAndShadowAI />} />
                <Route
                  path="audit"
                  element={<Navigate to="/activity-timeline" replace />}
                />
                <Route
                  path="decision-logs"
                  element={<Navigate to="/activity-timeline" replace />}
                />
                <Route path="cost-ledger" element={<CostLedger />} />

                {/* System & Settings */}
                <Route path="bundles" element={<Bundles />} />
                <Route path="discovery" element={<AutoDiscovery />} />
                <Route path="settings" element={<Settings />} />
                <Route path="settings/pdp" element={<PdpRoutingPage />} />

                {/* Legacy redirects */}
                <Route
                  path="blackbox-ai"
                  element={<Navigate to="/agents" replace />}
                />
                <Route
                  path="servers"
                  element={<Navigate to="/integrations" replace />}
                />
                <Route path="entities" element={<Entities />} />
                <Route
                  path="relationships"
                  element={<Navigate to="/entity-graph" replace />}
                />
                <Route
                  path="shadow-ai"
                  element={<Navigate to="/alerts" replace />}
                />
              </Route>
            </Route>
            {/* Full screen Wizard outside DashboardLayout */}
            <Route path="/wizard" element={<Wizard />} />
          </Routes>
        </Router>
      </ConfirmProvider>
    </ModeProvider>
  );
}

export default App;
