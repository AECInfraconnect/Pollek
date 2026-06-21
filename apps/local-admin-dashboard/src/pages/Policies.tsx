import { useState, useEffect } from "react";
import {
  FileKey,
  Plus,
  UploadCloud,
  X,
  Eye,
  Pencil,
  Trash2,
} from "lucide-react";
import { PolicyApi } from "../services/api";
import type { PolicyDraft, PolicyType } from "../services/api";

const TYPE_BADGE: Record<PolicyType, string> = {
  cedar: "bg-blue-500/15 text-blue-400",
  rego: "bg-purple-500/15 text-purple-400",
  open_fga: "bg-emerald-500/15 text-emerald-400",
  pii_redaction: "bg-amber-500/15 text-amber-400",
  route: "bg-slate-500/15 text-slate-400",
  composite: "bg-pink-500/15 text-pink-400",
};

const STATUS_BADGE: Record<string, string> = {
  draft: "bg-slate-500/15 text-slate-400",
  published: "bg-emerald-500/15 text-emerald-400",
  active: "bg-emerald-500/15 text-emerald-400",
  compiled: "bg-blue-500/15 text-blue-400",
};

export function Policies() {
  const [policies, setPolicies] = useState<PolicyDraft[]>([]);
  const [loading, setLoading] = useState(true);
  const [editorState, setEditorState] = useState<{
    mode: "create" | "edit" | "view";
    policy?: PolicyDraft;
  } | null>(null);
  const [publishing, setPublishing] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);

  const reload = () =>
    PolicyApi.list()
      .then(setPolicies)
      .catch(console.error)
      .finally(() => setLoading(false));

  useEffect(() => {
    reload();
  }, []);

  const onDelete = async (policyId: string) => {
    if (!confirm(`Are you sure you want to delete policy ${policyId}?`)) return;
    try {
      await PolicyApi.delete(policyId);
      setToast(`Deleted ${policyId}`);
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

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight flex items-center gap-2">
            <FileKey className="h-6 w-6 text-primary" /> Policy Enforcer
          </h2>
          <p className="text-muted-foreground">
            Author, compile, and publish signed policy bundles to the local
            workspace.
          </p>
        </div>
        <button
          onClick={() => setEditorState({ mode: "create" })}
          className="flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors shadow-lg shadow-primary/20"
        >
          <Plus className="h-4 w-4" /> New Policy
        </button>
      </div>

      {toast && (
        <div className="glass rounded-lg border px-4 py-3 text-sm">{toast}</div>
      )}

      <div className="glass rounded-xl overflow-hidden border">
        <table className="w-full text-sm text-left">
          <thead className="bg-muted/50 text-muted-foreground">
            <tr>
              <th className="px-6 py-4 font-medium">Name</th>
              <th className="px-6 py-4 font-medium">Type</th>
              <th className="px-6 py-4 font-medium">Status</th>
              <th className="px-6 py-4 font-medium">Targets</th>
              <th className="px-6 py-4 font-medium text-right">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {loading ? (
              <tr>
                <td
                  colSpan={5}
                  className="px-6 py-8 text-center text-muted-foreground"
                >
                  Loading policies...
                </td>
              </tr>
            ) : policies.length === 0 ? (
              <tr>
                <td
                  colSpan={5}
                  className="px-6 py-8 text-center text-muted-foreground"
                >
                  No policies yet. Create one to get started.
                </td>
              </tr>
            ) : (
              policies.map((p) => {
                const targetCount =
                  p.targets.agent_ids.length +
                  p.targets.tool_ids.length +
                  p.targets.resource_ids.length +
                  p.targets.entity_ids.length;
                return (
                  <tr
                    key={p.policy_id}
                    className="hover:bg-muted/30 transition-colors"
                  >
                    <td className="px-6 py-4">
                      <div className="font-medium">{p.name}</div>
                      <div className="text-xs text-muted-foreground">
                        {p.policy_id}
                      </div>
                    </td>
                    <td className="px-6 py-4">
                      <span
                        className={`rounded-full px-2 py-1 text-xs font-medium ${TYPE_BADGE[p.policy_type] ?? ""}`}
                      >
                        {p.policy_type}
                      </span>
                    </td>
                    <td className="px-6 py-4">
                      <span
                        className={`rounded-full px-2 py-1 text-xs font-medium ${STATUS_BADGE[p.meta.status] ?? "bg-slate-500/15 text-slate-400"}`}
                      >
                        {p.meta.status}
                      </span>
                    </td>
                    <td className="px-6 py-4 text-muted-foreground">
                      {targetCount} target(s)
                    </td>
                    <td className="px-6 py-4 text-right">
                      <div className="flex justify-end gap-2">
                        <button
                          onClick={() =>
                            setEditorState({ mode: "view", policy: p })
                          }
                          title="View Policy"
                          className="inline-flex items-center gap-1.5 rounded-md border px-2 py-1.5 text-xs font-medium hover:bg-muted/50"
                        >
                          <Eye className="h-3.5 w-3.5" />
                        </button>
                        <button
                          onClick={() =>
                            setEditorState({ mode: "edit", policy: p })
                          }
                          disabled={
                            p.meta.source === "cloud_sync" ||
                            p.meta.created_by !== "local-admin"
                          }
                          title="Edit Policy"
                          className="inline-flex items-center gap-1.5 rounded-md border px-2 py-1.5 text-xs font-medium hover:bg-muted/50 disabled:opacity-50"
                        >
                          <Pencil className="h-3.5 w-3.5" />
                        </button>
                        <button
                          onClick={() => onDelete(p.policy_id)}
                          disabled={
                            p.meta.source === "cloud_sync" ||
                            p.meta.created_by !== "local-admin"
                          }
                          title="Delete Policy"
                          className="inline-flex items-center gap-1.5 rounded-md border border-red-500/30 text-red-400 px-2 py-1.5 text-xs font-medium hover:bg-red-500/10 disabled:opacity-50 disabled:border-muted"
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </button>
                        <button
                          onClick={() => onPublish(p.policy_id)}
                          disabled={publishing === p.policy_id}
                          className="inline-flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-xs font-medium hover:bg-muted/50 disabled:opacity-50"
                        >
                          <UploadCloud className="h-3.5 w-3.5" />
                          {publishing === p.policy_id
                            ? "Publishing..."
                            : "Publish"}
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>

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

  // Track if text was manually edited so we don't overwrite user's typing
  const [isTyping, setIsTyping] = useState(mode !== "create");

  // Change template when type changes (if not typing)
  useEffect(() => {
    if (mode === "create" && !isTyping) {
      setText(DEFAULT_TEMPLATES[type] || "");
    }
  }, [type, mode, isTyping]);

  // Auto-detect engine from text
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

    // Basic Syntax Validation
    if (type === "rego" && !text.includes("package")) {
      setError(
        'Invalid OPA/Rego policy: Must contain a "package" declaration.',
      );
      setSaving(false);
      return;
    }
    if (
      type === "cedar" &&
      !text.includes("permit") &&
      !text.includes("forbid")
    ) {
      setError(
        'Invalid Cedar policy: Must contain at least one "permit" or "forbid" statement.',
      );
      setSaving(false);
      return;
    }
    if (type === "open_fga" && !text.includes("model")) {
      setError('Invalid OpenFGA model: Must contain a "model" declaration.');
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
              <option value="cedar" className="bg-background text-foreground">
                Cedar
              </option>
              <option value="rego" className="bg-background text-foreground">
                OPA / Rego
              </option>
              <option
                value="open_fga"
                className="bg-background text-foreground"
              >
                OpenFGA
              </option>
            </select>
          </div>
        </div>

        <div>
          <label
            htmlFor="policy-source"
            className="text-xs font-medium text-muted-foreground"
          >
            Policy source (compiled on the control plane, not the DEK)
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
