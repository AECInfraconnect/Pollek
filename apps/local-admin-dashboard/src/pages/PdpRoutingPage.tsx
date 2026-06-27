import { PdpRuntimeRouting } from "../components/pdp/PdpRuntimeRouting";

export function PdpRoutingPage() {
  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold tracking-tight">PDP & Routing</h2>
          <p className="text-muted-foreground">
            Configure Policy Decision Point (PDP) engines and routing rules.
          </p>
        </div>
      </div>
      <PdpRuntimeRouting />
    </div>
  );
}
