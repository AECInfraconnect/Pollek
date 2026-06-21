import { useState, useEffect } from "react";
import { FileKey, Plus, Eye } from "lucide-react";
import { PolicyApi } from "../services/api";
import { PresetWizard } from "../components/PresetWizard";

export function PolicyPresets() {
  const [presets, setPresets] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedPreset, setSelectedPreset] = useState<any | null>(null);

  useEffect(() => {
    PolicyApi.listPresets()
      .then((res: any) => {
        setPresets(res.items || []);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight flex items-center gap-2">
            <FileKey className="h-6 w-6 text-primary" /> Policy Presets
          </h2>
          <p className="text-muted-foreground">
            Use predefined templates to secure your system with industry best practices.
          </p>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
        {loading ? (
          <div className="col-span-full py-8 text-center text-muted-foreground">Loading presets...</div>
        ) : presets.length === 0 ? (
          <div className="col-span-full py-8 text-center text-muted-foreground">No presets available.</div>
        ) : presets.map((preset) => (
          <div key={preset.id} className="glass rounded-xl border p-5 flex flex-col gap-4 transition-all hover:border-primary/50 group">
            <div>
              <div className="flex items-start justify-between mb-2">
                <h3 className="font-semibold">{preset.name}</h3>
                <span className="text-xs bg-muted px-2 py-1 rounded text-muted-foreground">
                  {preset.category}
                </span>
              </div>
              <p className="text-sm text-muted-foreground">{preset.description}</p>
            </div>
            
            <div className="flex flex-wrap gap-2 mt-auto">
              {preset.recommended_pep_types?.map((pep: string) => (
                <span key={pep} className="text-xs px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-400">
                  {pep}
                </span>
              ))}
            </div>

            <div className="flex items-center gap-2 pt-4 mt-2 border-t border-border/50">
              <button 
                onClick={() => setSelectedPreset(preset)}
                className="flex-1 flex justify-center items-center gap-2 rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
              >
                <Plus className="h-4 w-4" /> Configure Preset
              </button>
              <button className="flex justify-center items-center rounded-md border px-3 py-2 text-sm font-medium hover:bg-muted transition-colors">
                <Eye className="h-4 w-4" />
              </button>
            </div>
          </div>
        ))}
      </div>
      {selectedPreset && <PresetWizard preset={selectedPreset} onClose={() => setSelectedPreset(null)} />}
    </div>
  );
}
