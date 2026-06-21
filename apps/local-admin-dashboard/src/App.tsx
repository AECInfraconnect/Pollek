import {
  BrowserRouter as Router,
  Routes,
  Route,
  Navigate,
} from "react-router-dom";
import { DashboardLayout } from "./components/layout/DashboardLayout";
import { Overview } from "./pages/Overview";
import { Resources } from "./pages/Resources";
import { Policies } from "./pages/Policies";
import { Simulator } from "./pages/Simulator";
import { Bundles } from "./pages/Bundles";
import { DecisionLogs } from "./pages/DecisionLogs";
import { Settings } from "./pages/Settings";
import { AutoDiscovery } from "./pages/AutoDiscovery";
import { PolicySuggestions } from "./pages/PolicySuggestions";
import { CostLedger } from "./pages/CostLedger";
import { PolicyPresets } from "./pages/PolicyPresets";

// Merged composite pages
import { AgentsAndModels } from "./pages/Ecosystem/AgentsAndModels";
import { Integrations } from "./pages/Ecosystem/Integrations";
import { IdentityNetwork } from "./pages/Data/IdentityNetwork";
import { AlertsAndShadowAI } from "./pages/Monitoring/AlertsAndShadowAI";

function App() {
  return (
    <Router>
      <Routes>
        <Route path="/" element={<DashboardLayout />}>
          <Route index element={<Overview />} />

          {/* AI Ecosystem */}
          <Route path="agents" element={<AgentsAndModels />} />
          <Route path="integrations" element={<Integrations />} />

          {/* Data & Context */}
          <Route path="resources" element={<Resources />} />
          <Route path="identities" element={<IdentityNetwork />} />

          {/* Security & Guardrails */}
          <Route path="policy-presets" element={<PolicyPresets />} />
          <Route path="policy-suggestions" element={<PolicySuggestions />} />
          <Route path="policies" element={<Policies />} />
          <Route path="simulator" element={<Simulator />} />

          {/* Monitoring & Activity */}
          <Route path="alerts" element={<AlertsAndShadowAI />} />
          <Route path="audit" element={<DecisionLogs />} />
          <Route path="cost-ledger" element={<CostLedger />} />

          {/* System & Settings */}
          <Route path="bundles" element={<Bundles />} />
          <Route path="discovery" element={<AutoDiscovery />} />
          <Route path="settings" element={<Settings />} />

          {/* Legacy redirects */}
          <Route
            path="blackbox-ai"
            element={<Navigate to="/agents" replace />}
          />
          <Route
            path="servers"
            element={<Navigate to="/integrations" replace />}
          />
          <Route
            path="tools"
            element={<Navigate to="/integrations" replace />}
          />
          <Route
            path="entities"
            element={<Navigate to="/identities" replace />}
          />
          <Route
            path="relationships"
            element={<Navigate to="/identities" replace />}
          />
          <Route path="shadow-ai" element={<Navigate to="/alerts" replace />} />
        </Route>
      </Routes>
    </Router>
  );
}

export default App;
