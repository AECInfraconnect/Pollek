import { useConfirm } from "../components/ui/ConfirmDialog";
import { useState, useEffect } from "react";
import { useSearchParams } from "react-router-dom";
import { Plus, X, UploadCloud, Trash2, Pencil } from "lucide-react";
import { PolicyApi } from "../services/api";
import type { PolicyDraft, PolicyType } from "../services/api";
import { MasterDetailLayout } from "../components/layout/MasterDetailLayout";
import { EntityCard } from "../components/shared/EntityCard";
import type { EntityCardProps } from "../components/shared/EntityCard";
import { Entity360Layout } from "../features/entity-360/Entity360Layout";
import { useEntity360 } from "../features/entity-graph/useEntity360";

export function Policies() {
  const { confirm } = useConfirm();
  const [params, setParams] = useSearchParams();

  const [policies, setPolicies] = useState<PolicyDraft[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedPolicyId, setSelectedPolicyIdState] = useState<string | null>(
    () => params.get("selected"),
  );
  const [editorState, setEditorState] = useState<{
    mode: "create" | "edit" | "view";
    policy?: PolicyDraft;
  } | null>(null);
  const [publishing, setPublishing] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  const reload = () => {
    setLoading(true);
    PolicyApi.list()
      .then(setPolicies)
      .catch(console.error)
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    reload();
  }, []);

  useEffect(() => {
    setSelectedPolicyIdState(params.get("selected"));
  }, [params]);

  const setSelectedPolicyId = (policyId: string | null) => {
    setSelectedPolicyIdState(policyId);
    setParams((next) => {
      if (policyId) {
        next.set("selected", policyId);
      } else {
        next.delete("selected");
      }
      return next;
    });
  };

  const onDelete = async (policyId: string) => {
    if (
      !(await confirm({
        title: "Confirm Action",
        description: `Are you sure you want to delete policy ${policyId}?`,
        danger: true,
      }))
    )
      return;
    try {
      await PolicyApi.delete(policyId);
      setToast(`Deleted ${policyId}`);
      if (selectedPolicyId === policyId) setSelectedPolicyId(null);
      reload();
    } catch (e) {
      setToast(`Delete failed: ${String(e)}`);
    } finally {
      setTimeout(() => setToast(null), 5000);
    }
  };

  const onPublish = async (policyId: string) => {
    setPublishing(policyId);
    try {
      const r = await PolicyApi.publish(policyId);
      setToast(
        `Published ${policyId} → bundle ${r.bundle_id} (build #${r.build_number})`,
      );
      reload();
    } catch (e) {
      setToast(`Publish failed: ${String(e)}`);
    } finally {
      setPublishing(null);
      setTimeout(() => setToast(null), 5000);
    }
  };

  const mappedCards: EntityCardProps[] = policies.map((p) => {
    const targetCount =
      p.targets.agent_ids.length +
      p.targets.tool_ids.length +
      p.targets.resource_ids.length +
      p.targets.entity_ids.length;

    return {
      id: p.policy_id,
      kind: "policy",
      title: p.name,
      subtitle: p.policy_id,
      status:
        p.meta.status === "active" || p.meta.status === "published"
          ? "active"
          : p.meta.status === "draft"
            ? "needs_approval"
            : "unknown",
      statusLabel: p.meta.status,
      summary: `Targets: ${targetCount}`,
      chips: [{ label: p.policy_type, tone: "neutral" }],
      lastUpdatedAt: p.meta.updated_at,
    };
  });

  const selectedPolicy = policies.find((p) => p.policy_id === selectedPolicyId);

  const masterContent = (
    <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
      {loading ? (
        <div className="text-muted-foreground p-4">Loading policies...</div>
      ) : mappedCards.length === 0 ? (
        <div className="text-muted-foreground p-4">
          No policies yet. Create one to get started.
        </div>
      ) : (
        mappedCards.map((card) => (
          <EntityCard
            key={card.id}
            {...card}
            selected={selectedPolicyId === card.id}
            onClick={() => setSelectedPolicyId(card.id)}
          />
        ))
      )}
    </div>
  );

  const detailContent = selectedPolicy ? (
    <Policy360Detail
      policy={selectedPolicy}
      publishing={publishing === selectedPolicy.policy_id}
      onEdit={() => setEditorState({ mode: "edit", policy: selectedPolicy })}
      onPublish={() => onPublish(selectedPolicy.policy_id)}
      onDelete={() => onDelete(selectedPolicy.policy_id)}
    />
  ) : null;

  return (
    <>
      <MasterDetailLayout
        title="Policy Enforcer"
        description="Author, compile, and publish signed policy bundles to the local workspace."
        actions={
          <button
            onClick={() => setEditorState({ mode: "create" })}
            className="flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors shadow-[0_0_15px_rgba(124,58,237,0.3)]"
          >
            <Plus className="h-4 w-4" /> New Policy
          </button>
        }
        masterContent={
          <>
            {toast && (
              <div className="glass rounded-lg border px-4 py-3 text-sm mb-4">
                {toast}
              </div>
            )}
            {masterContent}
          </>
        }
        detailContent={detailContent}
        onCloseDetail={() => setSelectedPolicyId(null)}
      />

      {editorState && (
        <PolicyEditor
          mode={editorState.mode}
          policy={editorState.policy}
          onClose={() => setEditorState(null)}
          onCreated={() => {
            setEditorState(null);
            reload();
          }}
        />
      )}
    </>
  );
}

function policyTargetCount(policy: PolicyDraft) {
  return (
    policy.targets.agent_ids.length +
    policy.targets.tool_ids.length +
    policy.targets.resource_ids.length +
    policy.targets.entity_ids.length
  );
}

function policySourceText(policy: PolicyDraft) {
  return policy.source?.kind === "raw_text"
    ? policy.source.text
    : JSON.stringify(policy.source, null, 2);
}

function PolicyFriendlyOverview({ policy }: { policy: PolicyDraft }) {
  return (
    <div className="space-y-5">
      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        <div className="rounded-lg border bg-muted/30 p-4">
          <div className="text-xs text-muted-foreground">Policy type</div>
          <div className="mt-1 text-sm font-medium">{policy.policy_type}</div>
        </div>
        <div className="rounded-lg border bg-muted/30 p-4">
          <div className="text-xs text-muted-foreground">Status</div>
          <div className="mt-1 text-sm font-medium">{policy.meta.status}</div>
        </div>
        <div className="rounded-lg border bg-muted/30 p-4">
          <div className="text-xs text-muted-foreground">Targets</div>
          <div className="mt-1 text-sm font-medium">
            {policyTargetCount(policy)}
          </div>
        </div>
        <div className="rounded-lg border bg-muted/30 p-4">
          <div className="text-xs text-muted-foreground">Source</div>
          <div className="mt-1 text-sm font-medium">{policy.meta.source}</div>
        </div>
      </div>

      <div className="rounded-lg border bg-muted/30 p-4">
        <h4 className="mb-3 text-sm font-semibold">Target Summary</h4>
        <dl className="grid gap-3 text-sm md:grid-cols-2">
          <div>
            <dt className="text-muted-foreground">Agents</dt>
            <dd>{policy.targets.agent_ids.join(", ") || "No agent targets"}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Tools</dt>
            <dd>{policy.targets.tool_ids.join(", ") || "No tool targets"}</dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Resources</dt>
            <dd>
              {policy.targets.resource_ids.join(", ") || "No resource targets"}
            </dd>
          </div>
          <div>
            <dt className="text-muted-foreground">Identities</dt>
            <dd>
              {policy.targets.entity_ids.join(", ") || "No identity targets"}
            </dd>
          </div>
        </dl>
      </div>

      <div className="rounded-lg border bg-muted/30 p-4">
        <h4 className="mb-2 text-sm font-semibold">Policy Source Preview</h4>
        <pre className="max-h-72 overflow-auto rounded-md border bg-background p-4 font-mono text-xs">
          {policySourceText(policy)}
        </pre>
      </div>
    </div>
  );
}

function Policy360Detail({
  policy,
  publishing,
  onEdit,
  onPublish,
  onDelete,
}: {
  policy: PolicyDraft;
  publishing: boolean;
  onEdit: () => void;
  onPublish: () => void;
  onDelete: () => void;
}) {
  const { data } = useEntity360("policy", policy.policy_id);
  const canEdit =
    policy.meta.source !== "cloud_sync" &&
    policy.meta.created_by === "local-admin";
  const actions = (
    <>
      <button
        type="button"
        onClick={onEdit}
        disabled={!canEdit}
        className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm font-medium hover:bg-muted disabled:opacity-50"
      >
        <Pencil className="h-4 w-4" />
        Edit
      </button>
      <button
        type="button"
        onClick={onPublish}
        disabled={publishing}
        className="inline-flex h-9 items-center gap-2 rounded-md border border-blue-500/25 bg-blue-500/10 px-3 text-sm font-medium text-blue-600 hover:bg-blue-500/15 disabled:opacity-50"
      >
        <UploadCloud className="h-4 w-4" />
        {publishing ? "Publishing..." : "Publish"}
      </button>
      <button
        type="button"
        onClick={onDelete}
        disabled={!canEdit}
        className="inline-flex h-9 items-center gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-3 text-sm font-medium text-red-600 hover:bg-red-500/15 disabled:opacity-50"
      >
        <Trash2 className="h-4 w-4" />
        Delete
      </button>
    </>
  );

  if (data) {
    return (
      <Entity360Layout
        data={data}
        actions={actions}
        overview={<PolicyFriendlyOverview policy={policy} />}
      />
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <h3 className="text-xl font-bold">{policy.name}</h3>
          <p className="mt-1 font-mono text-sm text-muted-foreground">
            {policy.policy_id}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">{actions}</div>
      </div>
      <PolicyFriendlyOverview policy={policy} />
    </div>
  );
}

function PolicyEditor({
  mode,
  policy,
  onClose,
  onCreated,
}: {
  mode: "create" | "edit" | "view";
  policy?: PolicyDraft;
  onClose: () => void;
  onCreated: () => void;
}) {
  const DEFAULT_TEMPLATES: Record<PolicyType, string> = {
    cedar: "permit(principal, action, resource);",
    rego: 'package authz\n\ndefault allow = false\n\nallow {\n  input.action == "read"\n}',
    open_fga:
      "model\n  schema 1.1\ntype user\ntype document\n  relations\n    define viewer: [user]",
    pii_redaction: "",
    route: "",
    composite: "",
  };

  const [name, setName] = useState(policy?.name ?? "");
  const [type, setType] = useState<PolicyType>(policy?.policy_type ?? "cedar");
  const initialText =
    policy?.source?.kind === "raw_text"
      ? policy.source.text
      : DEFAULT_TEMPLATES["cedar"];
  const [text, setText] = useState(initialText);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [isTyping, setIsTyping] = useState(mode !== "create");

  useEffect(() => {
    if (mode === "create" && !isTyping) {
      setText(DEFAULT_TEMPLATES[type] || "");
    }
  }, [type, mode, isTyping]);

  const handleTextChange = (newText: string) => {
    setText(newText);
    setIsTyping(true);
    if (mode === "create") {
      if (newText.includes("permit(") || newText.includes("forbid(")) {
        setType("cedar");
      } else if (newText.includes("package ")) {
        setType("rego");
      } else if (newText.includes("model") && newText.includes("type ")) {
        setType("open_fga");
      }
    }
  };

  const readOnly = mode === "view";
  const langFor: Record<string, string> = {
    cedar: "cedar",
    rego: "rego",
    open_fga: "fga",
  };

  const save = async () => {
    setSaving(true);
    setError(null);

    if (type === "rego" && !text.includes("package")) {
      setError("Invalid OPA/Rego policy: Must contain a package declaration.");
      setSaving(false);
      return;
    }
    if (
      type === "cedar" &&
      !text.includes("permit") &&
      !text.includes("forbid")
    ) {
      setError(
        "Invalid Cedar policy: Must contain at least one permit or forbid statement.",
      );
      setSaving(false);
      return;
    }
    if (type === "open_fga" && !text.includes("model")) {
      setError("Invalid OpenFGA model: Must contain a model declaration.");
      setSaving(false);
      return;
    }

    const now = new Date().toISOString();
    const policy_id = policy?.policy_id ?? `pol-${Date.now()}`;
    const draft: PolicyDraft = {
      meta: policy?.meta ?? {
        schema_version: "1.0",
        tenant_id: "local",
        workspace_id: "default",
        environment_id: "local",
        created_at: now,
        updated_at: now,
        created_by: "local-admin",
        updated_by: "local-admin",
        source: "manual",
        status: "draft",
        tags: [],
      },
      policy_id,
      name,
      description: policy?.description,
      policy_type: type,
      targets: policy?.targets ?? {
        agent_ids: [],
        tool_ids: [],
        resource_ids: [],
        entity_ids: [],
        route_ids: [],
      },
      source: { kind: "raw_text", language: langFor[type] ?? "text", text },
      compile_options: policy?.compile_options ?? { fail_on_warnings: true },
    };
    try {
      if (mode === "edit") {
        draft.meta.updated_at = now;
        await PolicyApi.update(policy_id, draft);
      } else {
        await PolicyApi.create(draft);
      }
      onCreated();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        className="glass w-full max-w-2xl rounded-xl border p-6 space-y-4"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold">
            {mode === "create"
              ? "New Policy"
              : mode === "edit"
                ? "Edit Policy"
                : "View Policy"}
          </h3>
          <button
            onClick={onClose}
            className="text-muted-foreground hover:text-foreground"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label
              htmlFor="policy-name"
              className="text-xs font-medium text-muted-foreground"
            >
              Name
            </label>
            <input
              id="policy-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={readOnly}
              className="mt-1 w-full rounded-md border bg-transparent px-3 py-2 text-sm disabled:opacity-50"
              placeholder="e.g. pol-net-deny"
            />
          </div>
          <div>
            <label
              htmlFor="policy-engine"
              className="text-xs font-medium text-muted-foreground"
            >
              Engine
            </label>
            <select
              id="policy-engine"
              value={type}
              onChange={(e) => setType(e.target.value as PolicyType)}
              disabled={readOnly || mode === "edit"}
              className="mt-1 w-full rounded-md border bg-background px-3 py-2 text-sm disabled:opacity-50"
            >
              <option value="cedar">Cedar</option>
              <option value="rego">OPA / Rego</option>
              <option value="open_fga">OpenFGA</option>
            </select>
          </div>
        </div>

        <div>
          <label
            htmlFor="policy-source"
            className="text-xs font-medium text-muted-foreground"
          >
            Policy source
          </label>
          <textarea
            id="policy-source"
            value={text}
            onChange={(e) => handleTextChange(e.target.value)}
            rows={10}
            disabled={readOnly}
            className="mt-1 w-full rounded-md border bg-black/30 px-3 py-2 font-mono text-xs disabled:opacity-50"
            spellCheck={false}
          />
        </div>

        {error && (
          <div className="rounded-md bg-red-500/10 px-3 py-2 text-xs text-red-400">
            {error}
          </div>
        )}

        <div className="flex justify-end gap-2">
          <button
            onClick={onClose}
            className="rounded-md border px-4 py-2 text-sm hover:bg-muted/50"
          >
            {readOnly ? "Close" : "Cancel"}
          </button>
          {!readOnly && (
            <button
              onClick={save}
              disabled={saving || !name}
              className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              {saving ? "Saving..." : "Save"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
