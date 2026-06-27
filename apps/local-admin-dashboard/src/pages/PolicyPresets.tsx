import { useState, useEffect } from "react";
import { FileKey, Plus, ShieldAlert, Tags } from "lucide-react";
import { PolicyApi } from "../services/api";
import { PresetWizard } from "../components/presets/PresetWizard";
import type { PolicyPresetV2, PresetCategory } from "../types/policy-presets";

export function PolicyPresets() {
  const [presets, setPresets] = useState<PolicyPresetV2[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedPreset, setSelectedPreset] = useState<PolicyPresetV2 | null>(
    null,
  );
  const [selectedCategory, setSelectedCategory] = useState<
    PresetCategory | "all"
  >("all");

  useEffect(() => {
    PolicyApi.listPresets()
      .then((res: any) => {
        setPresets(res.items || []);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  const categories = Array.from(new Set(presets.map((p) => p.category)));
  const filteredPresets =
    selectedCategory === "all"
      ? presets
      : presets.filter((p) => p.category === selectedCategory);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between border-b pb-4">
        <div>
          <h2 className="text-lg font-semibold tracking-tight flex items-center gap-2">
            <FileKey className="h-6 w-6 text-primary" /> Policy Presets V2
          </h2>
          <p className="text-muted-foreground mt-1">
            Deploy advanced guardrails using industry best practices mapping to
            OWASP and NIST frameworks.
          </p>
        </div>
      </div>

      <div className="flex gap-2 pb-2 overflow-x-auto">
        <button
          onClick={() => setSelectedCategory("all")}
          className={`px-4 py-1.5 rounded-full text-sm font-medium whitespace-nowrap transition-colors ${
            selectedCategory === "all"
              ? "bg-primary text-primary-foreground"
              : "bg-muted hover:bg-muted/80 text-foreground"
          }`}
        >
          All Categories
        </button>
        {categories.map((c) => (
          <button
            key={c}
            onClick={() => setSelectedCategory(c)}
            className={`px-4 py-1.5 rounded-full text-sm font-medium whitespace-nowrap transition-colors capitalize ${
              selectedCategory === c
                ? "bg-primary text-primary-foreground"
                : "bg-muted hover:bg-muted/80 text-foreground"
            }`}
          >
            {c.replace(/_/g, " ")}
          </button>
        ))}
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6">
        {loading ? (
          <div className="col-span-full py-12 text-center text-muted-foreground animate-pulse">
            Loading presets catalog...
          </div>
        ) : filteredPresets.length === 0 ? (
          <div className="col-span-full py-12 text-center text-muted-foreground">
            No presets found in this category.
          </div>
        ) : (
          filteredPresets.map((preset) => (
            <div
              key={preset.id}
              className="glass rounded-xl border flex flex-col transition-all hover:border-primary/50 hover:shadow-lg overflow-hidden"
            >
              <div className="p-5 flex-1 flex flex-col">
                <div className="flex items-start justify-between mb-3">
                  <h3 className="font-semibold text-lg leading-tight">
                    {preset.title}
                  </h3>
                  <span className="text-[10px] uppercase font-bold tracking-wider bg-primary/10 text-primary px-2 py-1 rounded">
                    v{preset.version}
                  </span>
                </div>

                <p className="text-sm text-muted-foreground mb-4 line-clamp-2 flex-1">
                  {preset.short_description}
                </p>

                <div className="space-y-3 mt-auto">
                  {preset.risk_tags && preset.risk_tags.length > 0 && (
                    <div className="flex items-start gap-2">
                      <ShieldAlert className="h-4 w-4 text-orange-500 mt-0.5 shrink-0" />
                      <div className="flex flex-wrap gap-1.5">
                        {preset.risk_tags.map((tag) => (
                          <span
                            key={tag}
                            className="text-[10px] px-1.5 py-0.5 rounded border bg-orange-500/5 text-orange-600 border-orange-500/20"
                          >
                            {tag}
                          </span>
                        ))}
                      </div>
                    </div>
                  )}

                  {preset.recommended_pep_types &&
                    preset.recommended_pep_types.length > 0 && (
                      <div className="flex items-start gap-2">
                        <Tags className="h-4 w-4 text-blue-500 mt-0.5 shrink-0" />
                        <div className="flex flex-wrap gap-1.5">
                          {preset.recommended_pep_types.map((pep) => (
                            <span
                              key={pep}
                              className="text-[10px] px-1.5 py-0.5 rounded border bg-blue-500/5 text-blue-600 border-blue-500/20"
                            >
                              {pep}
                            </span>
                          ))}
                        </div>
                      </div>
                    )}
                </div>
              </div>

              <div className="border-t p-3 bg-muted/10 flex gap-2">
                <button
                  onClick={() => setSelectedPreset(preset)}
                  className="flex-1 flex justify-center items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
                >
                  <Plus className="h-4 w-4" /> Deploy Guardrail
                </button>
              </div>
            </div>
          ))
        )}
      </div>

      {selectedPreset && (
        <PresetWizard
          preset={selectedPreset}
          onClose={() => setSelectedPreset(null)}
        />
      )}
    </div>
  );
}
