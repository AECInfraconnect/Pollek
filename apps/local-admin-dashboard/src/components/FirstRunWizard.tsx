import { useState, useEffect } from "react";
import {
  Shield,
  Search,
  ArrowRight,
  CheckCircle,
  Zap,
  RefreshCw,
} from "lucide-react";
import { RegistryApi } from "../services/api";
import type { DiscoveredAgentCandidateV2 } from "../services/types";

export function FirstRunWizard() {
  const [isOpen, setIsOpen] = useState(false);
  const [step, setStep] = useState(0);
  const [agreements, setAgreements] = useState<any[]>([]);
  const [accepted, setAccepted] = useState(false);
  const [candidates, setCandidates] = useState<DiscoveredAgentCandidateV2[]>(
    [],
  );
  const [_scanning, setScanning] = useState(false);
  const [baseControlLevel, setBaseControlLevel] = useState("observe");

  useEffect(() => {
    const isComplete = localStorage.getItem("pollek_setup_complete");
    if (!isComplete && !navigator.webdriver) {
      setIsOpen(true);
      fetch("/v1/consent/agreements")
        .then((r) => r.json())
        .then((data) => {
          if (data && data.agreements) {
            setAgreements(data.agreements);
          }
        })
        .catch((e) => console.error("Failed to load agreements:", e));
    }
  }, []);

  const handleAcceptAgreements = async () => {
    try {
      await fetch("/v1/consent", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ kind: "eula", version: "eula-2026-06" }),
      });
    } catch (e) {
      console.error("Failed to post consent:", e);
    }
    setStep(1);
  };

  const handleStartScan = async () => {
    setStep(2);
    setScanning(true);
    try {
      await RegistryApi.triggerDiscoveryScan({
        sources: ["process", "mcp_config", "browser_extension"],
        privacy_mode: true,
      });
      // Simulate waiting for scan to populate (in a real app, we poll getDiscoveryScanStatus)
      setTimeout(async () => {
        try {
          const c = await RegistryApi.listDiscoveryCandidates();
          setCandidates(c);
        } catch (e) {
          console.error(e);
        }
        setScanning(false);
        setStep(3);
      }, 4000);
    } catch (e) {
      console.error(e);
      setScanning(false);
      setStep(3);
    }
  };

  const handleComplete = () => {
    localStorage.setItem("pollek_setup_complete", "true");
    setIsOpen(false);
    // Refresh to apply changes and clear the setup state from UI flow
    window.location.reload();
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 bg-background/80 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-card text-card-foreground border shadow-2xl rounded-xl w-full max-w-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-300">
        <div className="p-6 border-b bg-muted/30">
          <h2 className="text-2xl font-bold flex items-center gap-2">
            <Shield className="h-6 w-6 text-primary" />
            Welcome to Pollek Local Enforcement Kit
          </h2>
          <p className="text-muted-foreground mt-1">
            Let's secure your local AI ecosystem in just a few steps.
          </p>
        </div>

        <div className="p-6">
          {step === 0 && (
            <div className="space-y-6 py-4">
              <div>
                <h3 className="text-xl font-semibold mb-4">
                  Agreements & Privacy
                </h3>
                <div className="bg-muted/30 rounded-lg p-4 max-h-64 overflow-y-auto space-y-4 border text-sm">
                  {agreements.length > 0 ? (
                    agreements.map((a) => (
                      <div
                        key={a.id}
                        className="border-b border-muted pb-4 last:border-0 last:pb-0"
                      >
                        <h4 className="font-medium text-base mb-1">
                          {a.title}{" "}
                          {a.required && (
                            <span className="text-red-500">*</span>
                          )}
                        </h4>
                        <p className="text-muted-foreground whitespace-pre-wrap">
                          {a.body_markdown}
                        </p>
                      </div>
                    ))
                  ) : (
                    <p className="text-muted-foreground">
                      Loading agreements...
                    </p>
                  )}
                </div>
              </div>

              <div className="flex items-center space-x-2 pt-2">
                <input
                  type="checkbox"
                  id="accept-terms"
                  checked={accepted}
                  onChange={(e) => setAccepted(e.target.checked)}
                  className="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary"
                />
                <label htmlFor="accept-terms" className="text-sm font-medium">
                  I accept the End User License Agreement and Privacy Notice
                </label>
              </div>

              <div className="flex justify-end pt-4">
                <button
                  onClick={handleAcceptAgreements}
                  disabled={!accepted}
                  className="bg-primary text-primary-foreground px-6 py-2.5 rounded-lg font-medium hover:bg-primary/90 transition shadow-md disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
                >
                  Continue <ArrowRight className="h-4 w-4" />
                </button>
              </div>
            </div>
          )}

          {step === 1 && (
            <div className="space-y-6 text-center py-8">
              <div className="mx-auto w-16 h-16 bg-primary/10 rounded-full flex items-center justify-center">
                <Search className="h-8 w-8 text-primary" />
              </div>
              <div>
                <h3 className="text-xl font-semibold">
                  Discover Local AI Agents
                </h3>
                <p className="text-muted-foreground max-w-md mx-auto mt-2">
                  We'll quickly scan your system to find running AI processes,
                  IDE extensions, and web AI agents.
                </p>
              </div>
              <button
                onClick={handleStartScan}
                className="bg-primary text-primary-foreground px-6 py-3 rounded-md font-medium inline-flex items-center gap-2 hover:bg-primary/90 transition shadow-lg shadow-primary/20"
              >
                Start Discovery Scan <ArrowRight className="h-4 w-4" />
              </button>
            </div>
          )}

          {step === 2 && (
            <div className="space-y-6 text-center py-12">
              <RefreshCw className="h-12 w-12 text-primary animate-spin mx-auto" />
              <div>
                <h3 className="text-lg font-medium">Scanning your system...</h3>
                <p className="text-muted-foreground mt-2">
                  Looking for IDEs, desktop agents, and web AI clients.
                </p>
              </div>
            </div>
          )}

          {step === 3 && (
            <div className="space-y-6">
              <div>
                <h3 className="text-lg font-semibold flex items-center gap-2">
                  <CheckCircle className="h-5 w-5 text-emerald-500" />
                  Scan Complete
                </h3>
                <p className="text-muted-foreground mt-1 text-sm">
                  Found {candidates.length} potential AI agents running locally.
                </p>
              </div>

              <div className="bg-muted/30 rounded-lg p-4 max-h-48 overflow-y-auto space-y-2 border">
                {candidates.length === 0 ? (
                  <div className="text-center py-6">
                    <p className="text-sm text-muted-foreground">
                      No agents found yet.
                    </p>
                    <p className="text-xs text-muted-foreground mt-1">
                      You can run a deeper scan later from the Auto Discovery
                      tab.
                    </p>
                  </div>
                ) : (
                  candidates.map((c) => (
                    <div
                      key={c.candidate_id}
                      className="flex justify-between items-center text-sm p-3 bg-background border rounded-lg hover:border-primary/50 transition-colors"
                    >
                      <span className="font-medium">
                        {c.display_name || c.candidate_id}
                      </span>
                      <span className="text-xs text-muted-foreground capitalize bg-muted px-2 py-1 rounded-full">
                        {c.inferred_agent_type.replace(/_/g, " ")}
                      </span>
                    </div>
                  ))
                )}
              </div>

              <div className="space-y-3 pt-4 border-t mt-6">
                <h4 className="font-medium">Set Default Security Posture</h4>
                <div className="grid grid-cols-2 gap-4">
                  <button
                    onClick={() => setBaseControlLevel("observe")}
                    className={`p-4 rounded-xl border text-left transition-all ${baseControlLevel === "observe" ? "border-primary ring-1 ring-primary bg-primary/5 shadow-md" : "hover:bg-muted/50 hover:border-muted-foreground/30"}`}
                  >
                    <div className="font-medium flex items-center justify-between">
                      Observe Only
                      {baseControlLevel === "observe" && (
                        <CheckCircle className="h-4 w-4 text-primary" />
                      )}
                    </div>
                    <div className="text-xs text-muted-foreground mt-2">
                      Monitor activity and log requests without blocking
                      anything. Best for learning.
                    </div>
                  </button>
                  <button
                    onClick={() => setBaseControlLevel("enforce")}
                    className={`p-4 rounded-xl border text-left transition-all ${baseControlLevel === "enforce" ? "border-primary ring-1 ring-primary bg-primary/5 shadow-md" : "hover:bg-muted/50 hover:border-muted-foreground/30"}`}
                  >
                    <div className="font-medium flex items-center justify-between">
                      <span className="flex items-center gap-1.5">
                        <Zap className="h-4 w-4 text-amber-500" /> Strict Guard
                      </span>
                      {baseControlLevel === "enforce" && (
                        <CheckCircle className="h-4 w-4 text-primary" />
                      )}
                    </div>
                    <div className="text-xs text-muted-foreground mt-2">
                      Block unauthorized resource access instantly based on
                      explicit policies.
                    </div>
                  </button>
                </div>
              </div>

              <div className="flex justify-end pt-6">
                <button
                  onClick={handleComplete}
                  className="bg-primary text-primary-foreground px-8 py-2.5 rounded-lg font-medium hover:bg-primary/90 transition shadow-lg shadow-primary/20"
                >
                  Finish Setup
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
