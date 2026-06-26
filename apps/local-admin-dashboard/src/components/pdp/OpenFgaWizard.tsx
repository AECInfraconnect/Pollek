import { useState } from "react";
import { Server, Database, CheckCircle, ArrowRight, X } from "lucide-react";
import { PdpRuntimeApi } from "../../services/api";

interface Props {
  isOpen: boolean;
  onClose: () => void;
  onComplete: () => void;
}

export function OpenFgaWizard({ isOpen, onClose, onComplete }: Props) {
  const [step, setStep] = useState(1);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [storeId, setStoreId] = useState<string>("");

  if (!isOpen) return null;

  const handleCheckConnection = async () => {
    setChecking(true);
    setError(null);
    try {
      // Basic check if OpenFGA is reachable
      const res = await fetch("http://localhost:8080/healthz");
      if (res.ok) {
        setStep(2);
      } else {
        setError("OpenFGA is running but returned an error.");
      }
    } catch (e: any) {
      setError("Cannot reach OpenFGA. Is the Docker container running?");
    } finally {
      setChecking(false);
    }
  };

  const handleCreateStore = async () => {
    setChecking(true);
    setError(null);
    try {
      const res = await fetch("http://localhost:8080/stores", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name: "pollek-dek-store" }),
      });
      if (!res.ok) {
        throw new Error("Failed to create store");
      }
      const data = await res.json();
      setStoreId(data.id);
      setStep(3);
    } catch (e: any) {
      setError("Failed to create store: " + (e.message || String(e)));
    } finally {
      setChecking(false);
    }
  };

  const handleRegister = async () => {
    setChecking(true);
    setError(null);
    try {
      await PdpRuntimeApi.upsert({
        id: `openfga_server-${Date.now()}`,
        name: "Local OpenFGA",
        category: "remote_connector",
        kind: "openfga_server",
        enabled: true,
        status: "ready",
        endpoint: "http://localhost:8080",
        mode: "strict_remote",
        system_managed: false,
        config_source: "manual",
        capabilities: [],
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      });
      onComplete();
      onClose();
    } catch (e: any) {
      setError("Failed to register: " + (e.message || String(e)));
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-background/80 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-card text-card-foreground border shadow-2xl rounded-xl w-full max-w-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-300">
        <div className="flex items-center justify-between p-6 border-b bg-muted/30">
          <h2 className="text-2xl font-bold flex items-center gap-2">
            <Server className="h-6 w-6 text-primary" />
            OpenFGA Setup
          </h2>
          <button
            onClick={onClose}
            className="p-2 hover:bg-muted rounded-full transition-colors"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        <div className="p-6">
          {error && (
            <div className="mb-6 p-4 bg-red-500/10 text-red-500 border border-red-500/20 rounded-md text-sm">
              {error}
            </div>
          )}

          {step === 1 && (
            <div className="space-y-6">
              <div className="flex items-start gap-4">
                <div className="w-8 h-8 rounded-full bg-primary text-primary-foreground flex items-center justify-center font-bold shrink-0">
                  1
                </div>
                <div>
                  <h3 className="text-lg font-semibold">
                    Start OpenFGA Docker Container
                  </h3>
                  <p className="text-muted-foreground mt-1">
                    OpenFGA requires a running server. The easiest way is to use
                    Docker. Run the following command in your terminal:
                  </p>
                  <pre className="mt-4 p-4 bg-muted/50 rounded-lg border font-mono text-sm overflow-x-auto text-primary">
                    docker run -p 8080:8080 -p 8081:8081 -p 3000:3000
                    openfga/openfga run
                  </pre>
                </div>
              </div>
              <div className="flex justify-end pt-4">
                <button
                  onClick={handleCheckConnection}
                  disabled={checking}
                  className="bg-primary text-primary-foreground px-6 py-2 rounded-md font-medium inline-flex items-center gap-2 hover:opacity-90 transition disabled:opacity-50"
                >
                  {checking
                    ? "Checking..."
                    : "I've started it. Check connection."}
                  {!checking && <ArrowRight className="h-4 w-4" />}
                </button>
              </div>
            </div>
          )}

          {step === 2 && (
            <div className="space-y-6">
              <div className="flex items-start gap-4">
                <div className="w-8 h-8 rounded-full bg-primary text-primary-foreground flex items-center justify-center font-bold shrink-0">
                  2
                </div>
                <div>
                  <h3 className="text-lg font-semibold">
                    Create an Authorization Store
                  </h3>
                  <p className="text-muted-foreground mt-1">
                    OpenFGA is running successfully. Now we need to create a
                    dedicated Store for Local Enforcement Kit to use.
                  </p>
                  <div className="mt-6 p-4 border rounded-lg bg-muted/30 flex items-center gap-4">
                    <Database className="h-8 w-8 text-muted-foreground" />
                    <div>
                      <div className="font-medium">
                        Store Name: pollek-dek-store
                      </div>
                      <div className="text-xs text-muted-foreground">
                        This store will hold all relationship tuples managed by
                        Local Enforcement Kit.
                      </div>
                    </div>
                  </div>
                </div>
              </div>
              <div className="flex justify-end pt-4">
                <button
                  onClick={handleCreateStore}
                  disabled={checking}
                  className="bg-primary text-primary-foreground px-6 py-2 rounded-md font-medium inline-flex items-center gap-2 hover:opacity-90 transition disabled:opacity-50"
                >
                  {checking ? "Creating..." : "Create Store"}
                  {!checking && <ArrowRight className="h-4 w-4" />}
                </button>
              </div>
            </div>
          )}

          {step === 3 && (
            <div className="space-y-6">
              <div className="flex items-start gap-4">
                <div className="w-8 h-8 rounded-full bg-primary text-primary-foreground flex items-center justify-center font-bold shrink-0">
                  3
                </div>
                <div>
                  <h3 className="text-lg font-semibold flex items-center gap-2">
                    <CheckCircle className="h-5 w-5 text-emerald-500" />
                    Setup Complete
                  </h3>
                  <p className="text-muted-foreground mt-1">
                    Your OpenFGA store has been created and is ready to be used
                    by Local Enforcement Kit.
                  </p>
                  <div className="mt-4 p-4 bg-muted/50 rounded-lg border font-mono text-sm break-all">
                    <strong>Store ID:</strong> {storeId}
                  </div>
                </div>
              </div>
              <div className="flex justify-end pt-4">
                <button
                  onClick={handleRegister}
                  disabled={checking}
                  className="bg-emerald-600 text-white px-6 py-2 rounded-md font-medium inline-flex items-center gap-2 hover:bg-emerald-700 transition shadow-lg shadow-emerald-500/20 disabled:opacity-50"
                >
                  {checking ? "Registering..." : "Register Connector"}
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
