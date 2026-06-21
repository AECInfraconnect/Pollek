import { useState, useEffect } from "react";
import { Server, CheckCircle2, XCircle, AlertTriangle } from "lucide-react";
import { PolicyApi } from "../../services/api";

export function PepTypeSelector({
  presetId,
  recommendedPeps,
  selectedPeps,
  onChange,
}: {
  presetId: string;
  recommendedPeps: string[];
  selectedPeps: string[];
  onChange: (peps: string[]) => void;
}) {
  const [capabilities, setCapabilities] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    PolicyApi.checkPepCapabilities({
      preset_id: presetId,
      target_os: "linux", // In reality, we'd detect or select the target OS
      requested_pep_types: recommendedPeps,
    })
      .then((res: any) => {
        setCapabilities(res.capabilities || []);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [presetId, recommendedPeps]);

  const togglePep = (pep: string) => {
    if (selectedPeps.includes(pep)) {
      onChange(selectedPeps.filter((p) => p !== pep));
    } else {
      onChange([...selectedPeps, pep]);
    }
  };

  if (loading) {
    return <div className="text-sm text-muted-foreground">Checking PEP capabilities...</div>;
  }

  return (
    <div className="space-y-3">
      <h4 className="font-medium flex items-center gap-2">
        <Server className="h-4 w-4" /> Target PEP Selection
      </h4>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        {capabilities.map((cap) => {
          const isRecommended = recommendedPeps.includes(cap.pep_type);
          const isSelected = selectedPeps.includes(cap.pep_type);
          const isAvailable = cap.status === "available";

          return (
            <button
              key={cap.pep_type}
              onClick={() => isAvailable && togglePep(cap.pep_type)}
              disabled={!isAvailable}
              className={`p-3 border rounded-lg text-left transition-all ${
                isSelected
                  ? "border-primary bg-primary/10 ring-1 ring-primary"
                  : "hover:border-primary/50"
              } ${!isAvailable ? "opacity-50 cursor-not-allowed bg-muted" : ""}`}
            >
              <div className="flex justify-between items-start mb-1">
                <span className="font-medium text-sm flex items-center gap-2">
                  {cap.pep_type}
                  {isRecommended && (
                    <span className="text-[10px] bg-blue-500/20 text-blue-500 px-1.5 py-0.5 rounded">
                      Recommended
                    </span>
                  )}
                </span>
                {isAvailable ? (
                  <CheckCircle2 className="h-4 w-4 text-green-500" />
                ) : (
                  <XCircle className="h-4 w-4 text-red-500" />
                )}
              </div>
              <div className="text-xs text-muted-foreground flex items-center justify-between">
                <span>Maturity: {cap.maturity}</span>
                <span>Mode: {cap.mode}</span>
              </div>
              {!isAvailable && cap.reason && (
                <div className="text-xs text-red-400 mt-2 flex items-center gap-1">
                  <AlertTriangle className="h-3 w-3" /> {cap.reason}
                </div>
              )}
            </button>
          );
        })}
      </div>
    </div>
  );
}
