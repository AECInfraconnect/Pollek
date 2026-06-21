import { useState, useEffect } from "react";
import { X, Play, Shield, ArrowRight } from "lucide-react";
import { PolicyApi } from "../../services/api";
import type {
  PolicyPresetV2,
  ControlMode,
  DeployPresetRequest,
  PolicyPresetPreviewResponse,
} from "../../types/policy-presets";
import { PepTypeSelector } from "./PepTypeSelector";
import { PolicyPreview } from "./PolicyPreview";
import { SimulationSummary } from "./SimulationSummary";

export function PresetWizard({
  preset,
  onClose,
}: {
  preset: PolicyPresetV2;
  onClose: () => void;
}) {
  const [controlMode, setControlMode] = useState<ControlMode>(
    preset.default_control_mode || "observe"
  );
  const [selectedPeps, setSelectedPeps] = useState<string[]>(
    preset.recommended_pep_types || []
  );
  const [params, setParams] = useState<Record<string, any>>({});
  
  const [preview, setPreview] = useState<PolicyPresetPreviewResponse | null>(null);
  const [simResult, setSimResult] = useState<any>(null);
  const [loading, setLoading] = useState(false);

  // Initialize params with defaults
  useEffect(() => {
    const defaultParams: Record<string, any> = {};
    if (preset.parameters) {
      preset.parameters.forEach((p) => {
        defaultParams[p.key] = p.default_value;
      });
    }
    setParams(defaultParams);
  }, [preset]);

  const generatePreview = async () => {
    setLoading(true);
    try {
      const req: DeployPresetRequest = {
        preset_id: preset.id,
        control_mode: controlMode,
        selected_pep_types: selectedPeps as any[],
        targets: {
          agent_ids: [],
          tool_ids: [],
          resource_ids: [],
          provider_ids: [],
          path_scopes: [],
          account_scopes: [],
        },
        params,
        dry_run_first: true,
      };
      const res: any = await PolicyApi.previewPreset(preset.id, req);
      setPreview(res);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const runSimulation = async () => {
    setLoading(true);
    try {
      const req = {
        apply_request: {
          preset_id: preset.id,
          control_mode: controlMode,
          selected_pep_types: selectedPeps,
          targets: {
            agent_ids: [],
            tool_ids: [],
            resource_ids: [],
            provider_ids: [],
            path_scopes: [],
            account_scopes: [],
          },
          params,
          dry_run_first: true,
        },
        input: {
          user: "test-user",
          action: "invoke_model",
          // MOCK INPUT FOR SIMULATION
        },
      };
      const res: any = await PolicyApi.simulatePreset(preset.id, req);
      setSimResult(res);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const createDraft = async () => {
    try {
      const req: DeployPresetRequest = {
        preset_id: preset.id,
        control_mode: controlMode,
        selected_pep_types: selectedPeps as any[],
        targets: {
          agent_ids: [],
          tool_ids: [],
          resource_ids: [],
          provider_ids: [],
          path_scopes: [],
          account_scopes: [],
        },
        params,
        dry_run_first: false,
      };
      await PolicyApi.createDraftFromPreset(preset.id, req);
      onClose();
    } catch (e) {
      console.error(e);
    }
  };

  const controlModes: { id: ControlMode; label: string; desc: string }[] = [
    { id: "observe", label: "Observe Only", desc: "Log decisions without blocking" },
    { id: "warn", label: "Warn", desc: "Allow but show user warning" },
    { id: "approval", label: "Require Approval", desc: "Pause for human approval" },
    { id: "enforce", label: "Enforce", desc: "Block actively" },
    { id: "strict_deny", label: "Strict Deny", desc: "Block and isolate" },
  ];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm p-4">
      <div className="glass w-full max-w-4xl rounded-xl border shadow-2xl flex flex-col max-h-[90vh]">
        <div className="flex items-center justify-between border-b p-4">
          <h3 className="text-lg font-semibold">
            Deploy Preset: {preset.title}
          </h3>
          <button onClick={onClose} className="p-1 hover:bg-muted rounded">
            <X className="h-5 w-5" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-6 space-y-8">
          {/* Overview */}
          <div>
            <p className="text-sm text-muted-foreground">{preset.long_description}</p>
          </div>

          {/* PEP Selection */}
          <PepTypeSelector
            presetId={preset.id}
            recommendedPeps={preset.recommended_pep_types}
            selectedPeps={selectedPeps}
            onChange={setSelectedPeps}
          />

          {/* Parameters */}
          {preset.parameters && preset.parameters.length > 0 && (
            <div className="space-y-3">
              <h4 className="font-medium">Configuration Parameters</h4>
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
                      onChange={(e) => setParams({ ...params, [p.key]: e.target.value })}
                      placeholder={`e.g. ${p.examples?.[0] || ""}`}
                    />
                    <div className="text-xs text-muted-foreground">{p.description}</div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Control Mode Selector */}
          <section className="space-y-3">
            <h4 className="font-medium flex items-center gap-2">
              <Shield className="h-4 w-4" /> Enforcement Mode
            </h4>
            <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
              {controlModes.map((lvl) => {
                const isSupported = preset.supported_control_modes.includes(lvl.id);
                return (
                  <button
                    key={lvl.id}
                    disabled={!isSupported}
                    onClick={() => setControlMode(lvl.id)}
                    className={`p-3 rounded-lg border text-left transition-all ${
                      controlMode === lvl.id
                        ? "bg-primary/10 border-primary ring-1 ring-primary"
                        : "hover:bg-muted/50"
                    } ${!isSupported ? "opacity-50 cursor-not-allowed bg-muted" : ""}`}
                  >
                    <div className="font-medium text-sm mb-1">{lvl.label}</div>
                    <div className="text-xs text-muted-foreground">{lvl.desc}</div>
                  </button>
                );
              })}
            </div>
          </section>

          {/* Preview and Simulation */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 pt-4 border-t">
            {/* Left: Preview */}
            <div className="space-y-3">
              <div className="flex justify-between items-center">
                <h4 className="font-medium">Policy Artifacts</h4>
                <button
                  onClick={generatePreview}
                  disabled={loading}
                  className="text-xs px-3 py-1 bg-secondary text-secondary-foreground rounded hover:bg-secondary/80"
                >
                  Generate Preview
                </button>
              </div>
              {preview ? (
                <PolicyPreview preview={preview} />
              ) : (
                <div className="text-sm text-muted-foreground p-4 bg-muted/30 rounded border text-center">
                  Click Generate Preview to see the raw policies that will be deployed.
                </div>
              )}
            </div>

            {/* Right: Simulation */}
            <div className="space-y-3">
              <div className="flex justify-between items-center">
                <h4 className="font-medium">Dry Run Simulation</h4>
                <button
                  onClick={runSimulation}
                  disabled={loading}
                  className="text-xs px-3 py-1 flex items-center gap-1 bg-secondary text-secondary-foreground rounded hover:bg-secondary/80"
                >
                  <Play className="h-3 w-3" /> Run Test
                </button>
              </div>
              {simResult ? (
                <SimulationSummary simResult={simResult} />
              ) : (
                <div className="text-sm text-muted-foreground p-4 bg-muted/30 rounded border text-center">
                  Run a test to simulate how this policy behaves.
                </div>
              )}
            </div>
          </div>
        </div>

        <div className="border-t p-4 flex justify-between items-center bg-muted/20">
          <button
            onClick={onClose}
            className="px-4 py-2 border rounded hover:bg-muted text-sm font-medium"
          >
            Cancel
          </button>
          <button
            onClick={createDraft}
            className="px-5 py-2 bg-primary text-primary-foreground rounded hover:bg-primary/90 text-sm font-medium flex items-center gap-2 shadow-lg"
          >
            Deploy Draft <ArrowRight className="h-4 w-4" />
          </button>
        </div>
      </div>
    </div>
  );
}
