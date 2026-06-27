import { useState, useEffect } from "react";
import { Plus, Cloud, Activity, Info } from "lucide-react";
import { useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import { RegistryApi } from "../services/api";
import type { BlackboxAiProvider } from "../services/types";
import { MasterDetailLayout } from "../components/master-detail/MasterDetailLayout";
import { EntityCard } from "../components/master-detail/EntityCard";
import { DetailPane } from "../components/master-detail/DetailPane";
import { EmptyState } from "../components/master-detail/EmptyState";
import { useConfirm } from "../components/ui/ConfirmDialog";
import type { UiStatus } from "../lib/status";

export function BlackboxAI({ hideHeader = false }: { hideHeader?: boolean }) {
  const [providers, setProviders] = useState<BlackboxAiProvider[]>([]);
  const [loading, setLoading] = useState(true);
  const [params, setParams] = useSearchParams();
  const selectedId = params.get("selected") ?? undefined;
  const { confirm } = useConfirm();

  const fetchProviders = () => {
    setLoading(true);
    RegistryApi.listBlackboxAiProviders()
      .then(setProviders)
      .catch(console.error)
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    fetchProviders();
  }, []);

  const select = (id: string) =>
    setParams((p) => {
      p.set("selected", id);
      return p;
    });

  const deleteProvider = async (id: string) => {
    if (
      !(await confirm({
        title: "Delete Provider",
        description:
          "Are you sure you want to delete this external model configuration?",
        danger: true,
      }))
    )
      return;
    try {
      await RegistryApi.deleteBlackboxAi(id);
      if (selectedId === id) {
        setParams((p) => {
          p.delete("selected");
          return p;
        });
      }
      toast.success("Provider deleted successfully");
      fetchProviders();
    } catch (e) {
      console.error("Failed to delete provider:", e);
      toast.error("Failed to delete provider");
    }
  };

  return (
    <div className={hideHeader ? "space-y-6" : "p-6 md:p-8 space-y-6"}>
      {!hideHeader && (
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-lg font-semibold tracking-tight">
              Blackbox AI
            </h2>
            <p className="text-sm text-muted-foreground">
              Manage external AI model providers (OpenAI, Anthropic, Google,
              etc).
            </p>
          </div>
          <button className="flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 shadow-sm">
            <Plus className="h-4 w-4" />
            Add Provider
          </button>
        </div>
      )}

      <MasterDetailLayout
        idSelector={(p) => p.provider_id}
        items={providers}
        loading={loading}
        selectedId={selectedId}
        onSelect={select}
        toolbar={
          <div className="flex items-center gap-2 mb-4">
            <input
              type="text"
              placeholder="Search providers..."
              className="px-3 py-1.5 text-sm rounded-md border bg-background"
            />
          </div>
        }
        emptyState={
          <EmptyState
            icon={Cloud}
            title="No providers found"
            description="Register a Blackbox AI provider to manage its usage policies."
            actionLabel="Add Provider"
            onAction={() => {}}
          />
        }
        renderCard={(p, selected) => {
          let status: UiStatus = "ok";

          return (
            <EntityCard
              title={p.name}
              subtitle={p.provider_type}
              icon={Cloud}
              status={status}
              statusLabel={p.meta.status || "Unknown"}
              meta={[{ label: "Auth", value: p.auth_mechanism.type }]}
              selected={selected}
            />
          );
        }}
        renderDetail={(p) => {
          let status: UiStatus = "ok";

          return (
            <DetailPane
              title={p.name}
              subtitle={p.provider_type}
              status={status}
              statusLabel={p.meta.status || "Unknown"}
              actions={[
                {
                  label: "Delete",
                  danger: true,
                  onClick: () => deleteProvider(p.provider_id),
                },
              ]}
              tabs={[
                {
                  id: "overview",
                  label: "Overview",
                  content: (
                    <div className="space-y-6">
                      <div className="grid grid-cols-2 gap-4 text-sm">
                        <div className="p-4 bg-muted/30 rounded-xl border">
                          <span className="text-muted-foreground block mb-1">
                            Auth Mechanism
                          </span>
                          <span className="capitalize">
                            {p.auth_mechanism.type}
                          </span>
                        </div>
                      </div>

                      <div>
                        <h4 className="font-medium mb-2 flex items-center gap-2 text-sm">
                          <Info className="h-4 w-4" /> Raw Data
                        </h4>
                        <pre className="text-[10px] font-mono bg-muted/50 p-4 rounded-lg overflow-x-auto border">
                          {JSON.stringify(p, null, 2)}
                        </pre>
                      </div>
                    </div>
                  ),
                },
                {
                  id: "activity",
                  label: "Activity",
                  content: (
                    <div className="flex flex-col items-center justify-center p-8 text-center border border-dashed rounded-lg text-muted-foreground">
                      <Activity className="h-8 w-8 mb-2 opacity-50" />
                      <p className="text-sm">No activity recorded yet.</p>
                    </div>
                  ),
                },
              ]}
            />
          );
        }}
      />
    </div>
  );
}
