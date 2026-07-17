import { PdpRuntimeRouting } from "../components/pdp/PdpRuntimeRouting";
import { PageHeader } from "../components/layout/PageHeader";

export function PdpRoutingPage() {
  return (
    <div className="space-y-6">
      <PageHeader
        title="PDP & Routing"
        subtitle="Choose which policy engines make decisions, and how requests are routed to them."
      />
      <PdpRuntimeRouting />
    </div>
  );
}
