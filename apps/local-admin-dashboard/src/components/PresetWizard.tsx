import { useState, useEffect } from "react";
import {
  X,
  Play,
  Shield,
  Server,
  CheckCircle2,
  AlertTriangle,
  XCircle,
  Code,
} from "lucide-react";
import { PolicyApi } from "../services/api";

export function PresetWizard({
  preset,
  onClose,
}: {
  preset: any;
  onClose: () => void;
}) {
  const [controlLevel, setControlLevel] = useState("observe_only");
  const [capabilities, setCapabilities] = useState<any[]>([]);
  const [recommendedPep, setRecommendedPep] = useState("");
  const [simResult, setSimResult] = useState<any>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (preset.previewOnly) return;
    // Check capabilities
    PolicyApi.checkPepCapabilities({
      preset_id: preset.preset_id,
      target_os: "linux",
      requested_pep_types: preset.recommended_pep_types,
    })
      .then((res: any) => {
        setCapabilities(res.capabilities || []);
        setRecommendedPep(res.recommended || "");
      })
      .catch(console.error);
  }, [preset]);

  const onSimulate = async () => {
    setLoading(true);
    try {
      const res: any = await PolicyApi.simulatePreset(preset.preset_id, {
        apply_request: {
          control_level: controlLevel,
        },
        input: {
          user: "test-user",
          action: "invoke_model",
        },
      });
      setSimResult(res.result);
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const levels = [
    { id: "off", name: "Off", desc: "Preset not active" },
    {
      id: "observe_only",
      name: "Observe Only",
      desc: "Log decision but do not block",
    },
    { id: "warn", name: "Warn", desc: "Allow but show warning" },
    {
      id: "require_approval",
      name: "Require Approval",
      desc: "Pause action until user/admin approves",
    },
    { id: "enforce", name: "Enforce", desc: "Allow/deny immediately" },
    {
      id: "strict_deny",
      name: "Strict Deny",
      desc: "Block and isolate target",
    },
  ];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm p-4">
      <div className="glass w-full max-w-3xl rounded-xl border shadow-2xl flex flex-col max-h-[90vh]">
        <div className="flex items-center justify-between border-b p-4">
          <h3 className="text-lg font-semibold">
            {preset.previewOnly ? "Policy Preview:" : "Configure Preset:"}{" "}
            {preset.display_name}
          </h3>
          <button onClick={onClose} className="p-1 hover:bg-muted rounded">
            <X className="h-5 w-5" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-6 space-y-8">
          {/* Policy Context Source Preview */}
          <section className="space-y-3">
            <h4 className="font-medium flex items-center gap-2">
              <Code className="h-4 w-4" /> Policy Context
            </h4>
            <div className="bg-muted/30 border rounded-lg p-4 font-mono text-xs overflow-x-auto whitespace-pre max-h-48 overflow-y-auto">
              {preset.template?.source || "No policy source available"}
            </div>
          </section>

          {!preset.previewOnly && (
            <>
              {/* Control Level Selector */}
              <section className="space-y-3">
                <h4 className="font-medium flex items-center gap-2">
                  <Shield className="h-4 w-4" /> Control Level
                </h4>
                <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
                  {levels.map((lvl) => {
                    const recCap = capabilities.find(
                      (c) => c.pep_type === recommendedPep,
                    );
                    const isStub = recCap && recCap.maturity === "stub";
                    const disabled =
                      isStub &&
                      (lvl.id === "enforce" || lvl.id === "strict_deny");

                    return (
                      <button
                        key={lvl.id}
                        disabled={disabled}
                        onClick={() => setControlLevel(lvl.id)}
                        className={`p-3 rounded-lg border text-left transition-all ${
                          controlLevel === lvl.id
                            ? "bg-primary/10 border-primary ring-1 ring-primary"
                            : "hover:bg-muted/50"
                        } ${disabled ? "opacity-50 cursor-not-allowed bg-muted" : ""}`}
                      >
                        <div className="font-medium text-sm flex items-center justify-between">
                          {lvl.name}
                          {disabled && (
                            <AlertTriangle className="h-3 w-3 text-yellow-500" />
                          )}
                        </div>
                        <div className="text-xs text-muted-foreground mt-1">
                          {lvl.desc}
                        </div>
                      </button>
                    );
                  })}
                </div>
              </section>

              {/* PEP Capability Panel */}
              <section className="space-y-3">
                <h4 className="font-medium flex items-center gap-2">
                  <Server className="h-4 w-4" /> PEP Capabilities
                </h4>
                <div className="bg-muted/30 border rounded-lg p-4 space-y-3">
                  <p className="text-sm">
                    Recommended PEP:{" "}
                    <strong className="text-primary">
                      {recommendedPep || "None"}
                    </strong>
                  </p>
                  <div className="space-y-2">
                    {capabilities.map((cap) => (
                      <div
                        key={cap.pep_type}
                        className="flex items-center justify-between text-sm p-2 rounded bg-background border"
                      >
                        <div className="flex items-center gap-2">
                          <span>{cap.pep_type}</span>
                          {cap.maturity && (
                            <span
                              className={`text-[10px] px-1.5 py-0.5 rounded uppercase font-bold tracking-wider ${
                                cap.maturity === "stub"
                                  ? "bg-yellow-500/20 text-yellow-600"
                                  : cap.maturity === "production"
                                    ? "bg-green-500/20 text-green-600"
                                    : "bg-blue-500/20 text-blue-600"
                              }`}
                            >
                              {cap.maturity.replace("_", " ")}
                            </span>
                          )}
                        </div>
                        <div className="flex items-center gap-2">
                          {cap.status === "available" ? (
                            <span className="text-green-500 flex items-center gap-1">
                              <CheckCircle2 className="h-4 w-4" /> Available (
                              {cap.mode})
                            </span>
                          ) : (
                            <span className="text-red-400 flex items-center gap-1">
                              <XCircle className="h-4 w-4" /> Not Available{" "}
                              <span className="text-xs text-muted-foreground ml-1">
                                ({cap.reason})
                              </span>
                            </span>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              </section>

              {/* Simulator */}
              <section className="space-y-3">
                <h4 className="font-medium flex items-center gap-2">
                  <Play className="h-4 w-4" /> Simulate Policy
                </h4>
                <div className="bg-muted/30 border rounded-lg p-4 flex flex-col gap-4">
                  <div className="flex justify-between items-center">
                    <span className="text-sm text-muted-foreground">
                      Test how this preset behaves with sample data.
                    </span>
                    <button
                      onClick={onSimulate}
                      disabled={loading}
                      className="px-3 py-1.5 bg-primary text-primary-foreground rounded text-sm hover:bg-primary/90 flex items-center gap-2"
                    >
                      <Play className="h-3 w-3" />{" "}
                      {loading ? "Simulating..." : "Run Test"}
                    </button>
                  </div>

                  {simResult && (
                    <div
                      className={`p-4 rounded border ${simResult.allowed ? "bg-green-500/10 border-green-500/20" : "bg-red-500/10 border-red-500/20"}`}
                    >
                      <div className="flex items-center gap-2 mb-2 font-semibold">
                        {simResult.decision === "error" ? (
                          <AlertTriangle className="h-4 w-4 text-orange-400" />
                        ) : simResult.allowed ? (
                          <CheckCircle2 className="h-4 w-4 text-green-500" />
                        ) : (
                          <XCircle className="h-4 w-4 text-red-500" />
                        )}
                        <span
                          className={
                            simResult.decision === "error"
                              ? "text-orange-400"
                              : simResult.allowed
                                ? "text-green-500"
                                : "text-red-500"
                          }
                        >
                          {simResult.decision.toUpperCase()}
                        </span>
                      </div>
                      <div className="text-sm">
                        {simResult.reason && (
                          <p className="mb-2">{simResult.reason}</p>
                        )}
                      </div>
                    </div>
                  )}
                </div>
              </section>
            </>
          )}
        </div>

        <div className="border-t p-4 flex justify-end gap-3 bg-muted/20">
          <button
            onClick={onClose}
            className="px-4 py-2 border rounded hover:bg-muted text-sm font-medium"
          >
            Cancel
          </button>
          {!preset.previewOnly && (
            <button className="px-4 py-2 bg-primary text-primary-foreground rounded hover:bg-primary/90 text-sm font-medium">
              Create Draft
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
