import { BrowserRouter as Router, Routes, Route } from "react-router-dom";
import { DashboardLayout } from "./components/layout/DashboardLayout";
import { Overview } from "./pages/Overview";
import { Agents } from "./pages/Agents";
import { Servers } from "./pages/Servers";
import { Tools } from "./pages/Tools";
import { Resources } from "./pages/Resources";
import { Policies } from "./pages/Policies";
import { Simulator } from "./pages/Simulator";
import { Bundles } from "./pages/Bundles";
import { DecisionLogs } from "./pages/DecisionLogs";
import { Entities } from "./pages/Entities";
import { Relationships } from "./pages/Relationships";
import { BlackboxAI } from "./pages/BlackboxAI";
import { Settings } from "./pages/Settings";
import { Alerts } from "./pages/Alerts";
import { AutoDiscovery } from "./pages/AutoDiscovery";
import { ShadowAI } from "./pages/ShadowAI";
import { PolicySuggestions } from "./pages/PolicySuggestions";
import { CostLedger } from "./pages/CostLedger";
import { PolicyPresets } from "./pages/PolicyPresets";

function App() {
  return (
    <Router>
      <Routes>
        <Route path="/" element={<DashboardLayout />}>
          <Route index element={<Overview />} />
          <Route path="agents" element={<Agents />} />
          <Route path="servers" element={<Servers />} />
          <Route path="tools" element={<Tools />} />
          <Route path="resources" element={<Resources />} />
          <Route path="entities" element={<Entities />} />
          <Route path="relationships" element={<Relationships />} />
          <Route path="blackbox-ai" element={<BlackboxAI />} />
          <Route path="policies" element={<Policies />} />
          <Route path="policy-presets" element={<PolicyPresets />} />
          <Route path="simulator" element={<Simulator />} />
          <Route path="bundles" element={<Bundles />} />
          <Route path="audit" element={<DecisionLogs />} />
          <Route path="alerts" element={<Alerts />} />
          <Route path="settings" element={<Settings />} />
          <Route path="discovery" element={<AutoDiscovery />} />
          <Route path="shadow-ai" element={<ShadowAI />} />
          <Route path="policy-suggestions" element={<PolicySuggestions />} />
          <Route path="cost-ledger" element={<CostLedger />} />
        </Route>
      </Routes>
    </Router>
  );
}

export default App;
