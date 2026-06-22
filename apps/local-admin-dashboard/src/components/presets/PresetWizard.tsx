import { useState, useEffect } from "react";
import { X, Shield, ArrowRight, ArrowLeft, CheckCircle2 } from "lucide-react";
import { DeploymentApi } from "../../services/api";
import type {
  PolicyPresetV2,
  ControlMode,
} from "../../types/policy-presets";
import { PolicyPreview } from "./PolicyPreview";
import { AgentSelector } from "../policy-deployment/AgentSelector";
import { PolicyGoalSelector } from "../policy-deployment/PolicyGoalSelector";
import { PdpRouteSelector } from "../policy-deployment/PdpRouteSelector";
import { PepCapabilityMatrix } from "../policy-deployment/PepCapabilityMatrix";
import { SimulationResults } from "../policy-deployment/SimulationResults";

export type WizardStep =
  | "scan"
  | "agents"
  | "goal"
  | "parameters"
  | "pep"
  | "pdp"
  | "preview"
  | "simulate"
  | "deploy"
  | "logs";

const STEP_ORDER: WizardStep[] = [
  "scan",
  "agents",
  "goal",
  "parameters",
  "pep",
  "pdp",
  "preview",
  "simulate",
  "deploy",
  "logs",
];

export function PresetWizard({
  preset,
  onClose,
}: {
  preset: PolicyPresetV2;
  onClose: () => void;
}) {
  const [step, setStep] = useState<WizardStep>("agents");
  const [controlMode, setControlMode] = useState<ControlMode>(
    preset.default_control_mode || "observe",
  );
  const [selectedPeps, setSelectedPeps] = useState<string[]>(
    preset.recommended_pep_types || [],
  );
  const [params, setParams] = useState<Record<string, any>>({});
  const [preview, setPreview] = useState<any | null>(null);
  const [simResult, setSimResult] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const [selectedAgents, setSelectedAgents] = useState<string[]>([]);
  const [selectedProviders, setSelectedProviders] = useState<string[]>([]);
  const [hasAgents, setHasAgents] = useState(true);
  useEffect(() => {
    const defaultParams: Record<string, any> = {};
    if (preset.parameters) {
      preset.parameters.forEach((p) => {
        defaultParams[p.key] = p.default_value;
      });
    }
    setParams(defaultParams);
  }, [preset]);

  const nextStep = () => {
    const idx = STEP_ORDER.indexOf(step);
    if (idx < STEP_ORDER.length - 1) {
      setStep(STEP_ORDER[idx + 1]);
    }
  };

  const prevStep = () => {
    const idx = STEP_ORDER.indexOf(step);
    if (idx > 0) {
      setStep(STEP_ORDER[idx - 1]);
    }
  };

  const generatePreview = async () => {
    setLoading(true);
    try {
      const req = {
        preset_id: preset.id,
        control_mode: controlMode,
        selected_pep_types: selectedPeps,
        targets: { 
          agent_ids: selectedAgents,
          provider_ids: selectedProviders,
        },
        params,
        pdp_route: "local_cedar",
      };
      const res = await DeploymentApi.preview(req);
      setPreview(res);
      nextStep();
    } catch (e: any) {
      console.error(e);
      alert("Failed to generate preview: " + (e.message || String(e)));
    } finally {
      setLoading(false);
    }
  };

  const runSimulation = async () => {
    setLoading(true);
    try {
      const req = {
        preset_id: preset.id,
        control_mode: controlMode,
        selected_pep_types: selectedPeps,
        targets: { 
          agent_ids: selectedAgents,
          provider_ids: selectedProviders,
        },
        params,
      };
      const res = await DeploymentApi.simulate(req);
      setSimResult(res);
      nextStep();
    } catch (e: any) {
      console.error(e);
      alert("Failed to simulate: " + (e.message || String(e)));
    } finally {
      setLoading(false);
    }
  };

  const executeDeploy = async () => {
    setLoading(true);
    try {
      const req = preview || {
        preset_id: preset.id,
        control_mode: controlMode,
        selected_pep_types: selectedPeps,
        targets: { 
          agent_ids: selectedAgents,
          provider_ids: selectedProviders,
        },
        params,
      };
      await DeploymentApi.deploy(req);
      nextStep();
    } catch (e: any) {
      console.error(e);
      alert("Failed to deploy: " + (e.message || String(e)));
    } finally {
      setLoading(false);
    }
  };

  const renderStepContent = () => {
    switch (step) {
      case "agents":
        return (
          <AgentSelector
            selectedAgents={selectedAgents}
            onSelectionChange={setSelectedAgents}
            selectedProviders={selectedProviders}
            onProviderSelectionChange={setSelectedProviders}
            onDataLoaded={setHasAgents}
          />
        );
      case "goal":
        return <PolicyGoalSelector preset={preset} />;
      case "parameters":
        return (
          <div className="space-y-4">
            <h4 className="font-medium">Configuration Parameters</h4>
            {preset.parameters && preset.parameters.length > 0 ? (
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4 bg-muted/20 p-4 rounded-lg border">
                {preset.parameters.map((p) => (
                  <div key={p.key} className="space-y-1 text-sm">
                    <label className="font-medium text-foreground block">
                      {p.label} {p.required && <span className="text-red-500">*</span>}
                    </label>
                    <input
                      type={p.value_type === "integer" ? "number" : "text"}
                      className="w-full bg-background border rounded px-3 py-2"
                      value={params[p.key] || ""}
                      onChange={(e) =>
                        setParams({ ...params, [p.key]: e.target.value })
                      }
                    />
                    <div className="text-xs text-muted-foreground">
                      {p.description}
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div className="text-sm text-muted-foreground">No parameters required.</div>
            )}
          </div>
        );
      case "pep":
        return (
          <PepCapabilityMatrix
            preset={preset}
            selectedPeps={selectedPeps}
            setSelectedPeps={setSelectedPeps}
          />
        );
      case "pdp":
        return (
          <PdpRouteSelector
            controlMode={controlMode}
            setControlMode={setControlMode}
          />
        );
      case "preview":
        return (
          <div className="space-y-4">
            <h4 className="font-medium">Deployment Preview</h4>
            {preview ? (
              <PolicyPreview preview={preview} />
            ) : (
              <div className="text-sm text-muted-foreground p-4 bg-muted/30 rounded border text-center">
                Click Next to generate a deployment preview.
              </div>
            )}
          </div>
        );
      case "simulate":
        return <SimulationResults simResult={simResult} />;
      case "deploy":
        return (
          <div className="space-y-4">
            <h4 className="font-medium">Execute Deployment</h4>
            <div className="text-sm text-muted-foreground p-4 bg-muted/30 rounded border text-center">
              You are about to deploy this policy to the selected agents.
            </div>
          </div>
        );
      case "logs":
        return (
          <div className="space-y-4 text-center py-12">
            <CheckCircle2 className="h-16 w-16 text-green-500 mx-auto mb-4" />
            <h4 className="font-medium text-xl">Deployment Successful</h4>
            <p className="text-muted-foreground">The policy bindings have been applied.</p>
          </div>
        );
      default:
        return null;
    }
  };

  const handleNext = () => {
    if (step === "agents") {
      if (!hasAgents) {
        alert("กรุณา Register Agent ก่อน");
        return;
      }
      if (selectedAgents.length === 0 && selectedProviders.length === 0) {
        alert("กรุณาเลือก Agent ก่อน");
        return;
      }
      nextStep();
    } else if (step === "pdp") {
      generatePreview();
    } else if (step === "preview") {
      runSimulation();
    } else if (step === "simulate") {
      executeDeploy();
    } else if (step === "logs") {
      onClose();
    } else {
      nextStep();
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm p-4">
      <div className="glass w-full max-w-4xl rounded-xl border shadow-2xl flex flex-col max-h-[90vh]">
        <div className="flex items-center justify-between border-b p-4">
          <h3 className="text-lg font-semibold flex items-center gap-2">
            <Shield className="h-5 w-5 text-primary" />
            Wizard: {preset.title}
          </h3>
          <button onClick={onClose} className="p-1 hover:bg-muted rounded text-muted-foreground">
            <X className="h-5 w-5" />
          </button>
        </div>

        <div className="flex bg-muted/10 border-b">
          <div className="flex-1 overflow-x-auto p-2 flex items-center gap-1 text-xs">
            {STEP_ORDER.filter(s => s !== "scan").map((s, i) => (
              <div
                key={s}
                className={`px-3 py-1.5 rounded-full capitalize whitespace-nowrap font-medium ${
                  step === s ? "bg-primary text-primary-foreground" : "text-muted-foreground"
                }`}
              >
                {i + 1}. {s}
              </div>
            ))}
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-6">
          {renderStepContent()}
        </div>

        <div className="border-t p-4 flex justify-between items-center bg-muted/20">
          <button
            onClick={prevStep}
            disabled={step === "agents" || step === "logs" || loading}
            className="px-4 py-2 border rounded hover:bg-muted text-sm font-medium flex items-center gap-2 disabled:opacity-50"
          >
            <ArrowLeft className="h-4 w-4" /> Back
          </button>
          
          <button
            onClick={handleNext}
            disabled={loading}
            className="px-5 py-2 bg-primary text-primary-foreground rounded hover:bg-primary/90 text-sm font-medium flex items-center gap-2 shadow-lg disabled:opacity-50"
          >
            {loading ? "Processing..." : step === "logs" ? "Finish" : step === "simulate" ? "Deploy Policy" : "Next Step"}
            {!loading && step !== "logs" && <ArrowRight className="h-4 w-4" />}
          </button>
        </div>
      </div>
    </div>
  );
}
