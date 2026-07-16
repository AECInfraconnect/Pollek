import { type ReactNode, useEffect, useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import {
  AlertTriangle,
  Eye,
  FileKey,
  ListChecks,
  Plus,
  Search,
  ShieldCheck,
  ShieldX,
} from "lucide-react";
import { CapabilityApi, PolicyApi } from "../services/api";
import type { LocalCapabilitySnapshotV2, PolicyDraft } from "../services/types";
import {
  SIMPLE_RULE_PRESETS,
  buildUserCapabilityMatrix,
  categoryLabel,
  capabilityTone,
  labelize,
} from "../features/user-activity/userActivityModel";
import type { SimpleRulePreset } from "../features/user-activity/types";
import { MasterDetailLayout } from "../components/master-detail/MasterDetailLayout";
import { PageHeader } from "../components/layout/PageHeader";
import { DetailPane } from "../components/master-detail/DetailPane";
import type { UiStatus } from "../lib/status";
import { useMode } from "../context/ModeContext";
import { isAdvanceMode } from "../lib/modes";
import { cn } from "@/lib/utils";
import { useConfirm } from "../components/ui/ConfirmDialog";
import { toast } from "sonner";
import { Collapsible } from "../components/ui";

const toneClass: Record<string, string> = {
  success: "border-emerald-500/25 bg-emerald-500/10 text-emerald-700",
  info: "border-blue-500/25 bg-blue-500/10 text-blue-700",
  warning: "border-amber-500/25 bg-amber-500/10 text-amber-700",
  neutral: "border-border bg-background text-muted-foreground",
};

type RuleRow =
  | {
      id: string;
      group: "Active rules" | "Draft policies";
      policy: PolicyDraft;
      preset?: never;
    }
  | {
      id: string;
      group: "Suggested rules";
      preset: SimpleRulePreset;
      policy?: never;
    };

function behaviorIcon(behavior: SimpleRulePreset["behavior"]) {
  if (behavior === "block") return ShieldX;
  if (behavior === "ask_first") return AlertTriangle;
  if (behavior === "allow") return ShieldCheck;
  return Eye;
}

function PolicyCard({
  policy,
  selected,
}: {
  policy: PolicyDraft;
  selected: boolean;
}) {
  const status = policy.meta?.status ?? "draft";
  return (
    <article
      className={cn(
        "rounded-lg border bg-card/60 p-4 transition-all hover:border-primary/40 hover:bg-card",
        selected &&
          "border-primary/50 bg-card shadow-md ring-1 ring-primary/50",
      )}
    >
      <div className="flex items-start gap-3">
        <div className="rounded-lg bg-primary/10 p-2 text-primary">
          <FileKey className="h-4 w-4" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-start justify-between gap-2">
            <h3 className="text-sm font-semibold">{policy.name}</h3>
            <span className="rounded-full border bg-background px-2 py-0.5 text-[11px] capitalize text-muted-foreground">
              {labelize(status)}
            </span>
          </div>
          <p className="mt-1 text-xs leading-5 text-muted-foreground">
            {policy.description ||
              "Technical policy available in Advanced Mode."}
          </p>
          <div className="mt-3 flex flex-wrap gap-1.5">
            <span className="rounded-full border px-2 py-0.5 text-[11px]">
              {labelize(policy.policy_type)}
            </span>
            <span className="rounded-full border px-2 py-0.5 text-[11px]">
              {policy.targets?.agent_ids?.length ?? 0} AI apps
            </span>
            <span className="rounded-full border px-2 py-0.5 text-[11px]">
              {policy.targets?.resource_ids?.length ?? 0} data targets
            </span>
          </div>
        </div>
      </div>
    </article>
  );
}

function PresetCard({
  preset,
  snapshot,
  selected,
}: {
  preset: SimpleRulePreset;
  snapshot: LocalCapabilitySnapshotV2 | null;
  selected: boolean;
}) {
  const Icon = behaviorIcon(preset.behavior);
  const matrix = buildUserCapabilityMatrix(snapshot);
  const capability =
    matrix.find((item) => item.category === preset.category) ??
    matrix.find((item) => item.id === "unknown");
  const tone = capability ? capabilityTone(capability.status) : "neutral";
  const statusText = capability
    ? capability.can_block
      ? "Can block here"
      : capability.can_ask_first
        ? "Can ask first"
        : capability.can_watch
          ? "Can watch now"
          : "Needs setup"
    : "Needs setup";

  return (
    <article
      className={cn(
        "rounded-lg border bg-card/60 p-4 transition-all hover:border-primary/40 hover:bg-card",
        selected &&
          "border-primary/50 bg-card shadow-md ring-1 ring-primary/50",
      )}
    >
      <div className="flex items-start gap-3">
        <div className={cn("rounded-lg p-2", toneClass[tone])}>
          <Icon className="h-4 w-4" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-start justify-between gap-2">
            <h3 className="text-sm font-semibold">{preset.label}</h3>
            <span
              className={cn(
                "rounded-full border px-2 py-0.5 text-[11px]",
                toneClass[tone],
              )}
            >
              {statusText}
            </span>
          </div>
          <p className="mt-1 text-xs leading-5 text-muted-foreground">
            {preset.description}
          </p>
          <div className="mt-3 flex flex-wrap gap-1.5">
            <span className="rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground">
              {preset.category === "unknown"
                ? "All activity"
                : categoryLabel(preset.category)}
            </span>
            <span className="rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground">
              {labelize(preset.behavior)}
            </span>
          </div>
          {capability && !capability.can_block && (
            <p className="mt-3 rounded-md border border-blue-500/20 bg-blue-500/10 p-3 text-xs text-blue-700">
              {capability.can_watch
                ? "Pollek can observe this now and guide the AI app setting until blocking is available."
                : capability.why}
            </p>
          )}
          <div className="mt-3">
            <Link
              to={`/protect?intent=${encodeURIComponent(preset.intent)}`}
              className="inline-flex h-8 items-center gap-2 rounded-md border px-3 text-xs text-primary hover:bg-primary/10"
            >
              <Plus className="h-3.5 w-3.5" />
              Start with this rule
            </Link>
          </div>
        </div>
      </div>
    </article>
  );
}

function policyStatus(policy: PolicyDraft): {
  label: string;
  status: UiStatus;
} {
  const status = policy.meta?.status ?? "draft";
  if (["published", "active", "approved", "validated"].includes(status)) {
    return { label: "Active", status: "ok" };
  }
  if (status === "draft") return { label: "Draft", status: "idle" };
  return { label: labelize(status), status: "info" };
}

function presetStatus(
  preset: SimpleRulePreset,
  snapshot: LocalCapabilitySnapshotV2 | null,
): { label: string; status: UiStatus; detail: string } {
  const capability = buildUserCapabilityMatrix(snapshot).find(
    (item) => item.category === preset.category,
  );
  if (!capability) {
    return {
      label: "Needs setup",
      status: "degraded",
      detail: "No local capability report matched this rule yet.",
    };
  }
  if (capability.can_block) {
    return {
      label: "Can block here",
      status: "ok",
      detail: capability.why,
    };
  }
  if (capability.can_ask_first) {
    return {
      label: "Can ask first",
      status: "info",
      detail: capability.why,
    };
  }
  if (capability.can_watch) {
    return {
      label: "Can watch now",
      status: "info",
      detail:
        "Pollek can observe this now and guide the AI app setting until blocking is available.",
    };
  }
  return {
    label: "Needs setup",
    status: "degraded",
    detail: capability.why,
  };
}

function RuleRecordFrame({
  row,
  title,
  subtitle,
  status,
  statusLabel,
  children,
}: {
  row: RuleRow;
  title: string;
  subtitle: string;
  status: UiStatus;
  statusLabel: string;
  children: ReactNode;
}) {
  const category = row.policy
    ? labelize(row.policy.policy_type)
    : row.preset.category === "unknown"
      ? "All activity"
      : categoryLabel(row.preset.category);
  const behavior = row.policy
    ? (row.policy.meta?.status ?? "Policy")
    : labelize(row.preset.behavior);
  const targetCount = row.policy
    ? (row.policy.targets?.agent_ids?.length ?? 0) +
      (row.policy.targets?.resource_ids?.length ?? 0)
    : 0;
  const { confirm } = useConfirm();

  const handleDelete = async () => {
    if (!row.policy?.policy_id) return;
    if (
      !(await confirm({
        title: "Delete Policy",
        description: `Are you sure you want to delete this policy? This cannot be undone.`,
        confirmText: "Delete",
        cancelText: "Cancel",
      }))
    ) {
      return;
    }
    try {
      await PolicyApi.delete(row.policy.policy_id);
      toast.success("Policy deleted successfully");
      // Trigger reload by mutating a global state or letting the parent handle it
      // since this is just a UI update for the demo we'll emit a window event
      window.dispatchEvent(new CustomEvent("refresh-policies"));
    } catch (error) {
      console.error(error);
      toast.error("Failed to delete policy");
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 border-b border-border/60 pb-4 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0">
          <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
            <ListChecks className="h-4 w-4 text-primary" />
            Rule Record
          </div>
          <h2 className="mt-1 break-words text-2xl font-bold tracking-tight">
            {title}
          </h2>
          <p className="mt-1 text-sm text-muted-foreground">{subtitle}</p>
        </div>
        <span
          className={cn(
            "inline-flex w-fit rounded-full border px-3 py-1 text-xs font-medium",
            status === "ok"
              ? "border-emerald-500/25 bg-emerald-500/10 text-emerald-700"
              : status === "degraded"
                ? "border-amber-500/25 bg-amber-500/10 text-amber-700"
                : "border-border bg-card text-muted-foreground",
          )}
        >
          {statusLabel}
        </span>
      </div>

      <div className="grid gap-4 md:grid-cols-[280px_minmax(0,1fr)] lg:grid-cols-[300px_minmax(0,1fr)_320px]">
        <aside className="space-y-3">
          <section className="rounded-lg border bg-card/50 p-4">
            <h3 className="mb-3 flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              <span className="h-1.5 w-1.5 rounded-full bg-primary" />
              Record Summary
            </h3>
            {row.policy && (
              <div className="mb-4 flex justify-end">
                <button
                  type="button"
                  onClick={handleDelete}
                  className="inline-flex h-8 items-center gap-1.5 rounded-md border border-red-500/20 bg-red-500/10 px-2.5 text-[11px] font-medium text-red-600 hover:bg-red-500/20"
                >
                  Delete Rule
                </button>
              </div>
            )}
            <div className="space-y-2 text-sm">
              <div className="border-b border-border/40 pb-2">
                <div className="text-xs text-muted-foreground">Group</div>
                <div className="mt-0.5 font-medium">{row.group}</div>
              </div>
              <div className="border-b border-border/40 pb-2">
                <div className="text-xs text-muted-foreground">Activity</div>
                <div className="mt-0.5 font-medium">{category}</div>
              </div>
              <div className="border-b border-border/40 pb-2">
                <div className="text-xs text-muted-foreground">Behavior</div>
                <div className="mt-0.5 break-words font-medium">{behavior}</div>
              </div>
              <div>
                <div className="text-xs text-muted-foreground">Targets</div>
                <div className="mt-0.5 font-medium">
                  {row.policy ? targetCount : "Preset"}
                </div>
              </div>
            </div>
          </section>
        </aside>

        <section className="min-w-0">{children}</section>

        <aside className="space-y-3 md:col-span-2 lg:col-span-1">
          <section className="rounded-lg border bg-card/50 p-4">
            <h3 className="text-sm font-semibold">Related Records</h3>
            <p className="mt-1 text-xs leading-5 text-muted-foreground">
              Review matching timeline evidence and the advanced policy record
              for this rule.
            </p>
            <div className="mt-3 flex flex-wrap gap-2">
              <Link
                to={`/activity?q=${encodeURIComponent(title)}`}
                className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
              >
                <Eye className="h-4 w-4" />
                Activity
              </Link>
              <Link
                to="/policies"
                className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
              >
                <FileKey className="h-4 w-4" />
                Advanced policies
              </Link>
            </div>
          </section>

          <section className="rounded-lg border bg-card/50 p-4">
            <h3 className="text-sm font-semibold">Capability Honesty</h3>
            <p className="mt-2 text-sm leading-6 text-muted-foreground">
              A rule can watch, ask, or block only where this OS and connector
              setup can prove support. If Pollek can only observe, use the
              activity evidence to adjust the AI app settings too.
            </p>
          </section>
        </aside>
      </div>
    </div>
  );
}

function RuleDetail({
  row,
  snapshot,
  showTechnicalDetails,
}: {
  row: RuleRow;
  snapshot: LocalCapabilitySnapshotV2 | null;
  showTechnicalDetails: boolean;
}) {
  if (row.policy) {
    const status = policyStatus(row.policy);
    return (
      <RuleRecordFrame
        row={row}
        title={row.policy.name}
        subtitle={row.group}
        status={status.status}
        statusLabel={status.label}
      >
        <DetailPane
          title="Detail Workspace"
          subtitle="Plain-language rule behavior, evidence links, and technical details."
          status={status.status}
          statusLabel={status.label}
          tabs={[
            {
              id: "overview",
              label: "Overview",
              content: (
                <div className="space-y-4">
                  <div className="rounded-lg border bg-background/60 p-4">
                    <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                      What this rule controls
                    </div>
                    <p className="mt-2 text-sm leading-6 text-muted-foreground">
                      {row.policy.description ||
                        "This technical policy can be reviewed in Advanced policies."}
                    </p>
                  </div>
                  <div className="grid gap-3 md:grid-cols-3">
                    <div className="rounded-lg border bg-background/60 p-4">
                      <div className="text-xs text-muted-foreground">Type</div>
                      <div className="mt-1 text-sm font-semibold">
                        {labelize(row.policy.policy_type)}
                      </div>
                    </div>
                    <div className="rounded-lg border bg-background/60 p-4">
                      <div className="text-xs text-muted-foreground">
                        AI apps
                      </div>
                      <div className="mt-1 text-sm font-semibold">
                        {row.policy.targets?.agent_ids?.length ?? 0}
                      </div>
                    </div>
                    <div className="rounded-lg border bg-background/60 p-4">
                      <div className="text-xs text-muted-foreground">
                        Data targets
                      </div>
                      <div className="mt-1 text-sm font-semibold">
                        {row.policy.targets?.resource_ids?.length ?? 0}
                      </div>
                    </div>
                  </div>
                </div>
              ),
            },
            {
              id: "next",
              label: "Next Steps",
              content: (
                <div className="space-y-3">
                  <div className="rounded-lg border bg-background/60 p-4">
                    <h4 className="text-sm font-semibold">
                      Confirm behavior in Activity
                    </h4>
                    <p className="mt-2 text-sm leading-6 text-muted-foreground">
                      After a rule is active, review AI Activity to confirm
                      whether the AI app was allowed, blocked, warned, or only
                      observed.
                    </p>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    <Link
                      to={`/activity?q=${encodeURIComponent(row.policy.name)}`}
                      className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
                    >
                      <Eye className="h-4 w-4" />
                      Activity
                    </Link>
                    <Link
                      to="/policies"
                      className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
                    >
                      <FileKey className="h-4 w-4" />
                      Advanced policies
                    </Link>
                  </div>
                </div>
              ),
            },
            ...(showTechnicalDetails
              ? [
                  {
                    id: "technical",
                    label: "Technical Details",
                    content: (
                      <Collapsible title="Policy JSON">
                        <pre className="overflow-auto rounded-none border-0 bg-transparent p-0 text-[11px]">
                          {JSON.stringify(row.policy, null, 2)}
                        </pre>
                      </Collapsible>
                    ),
                  },
                ]
              : []),
          ]}
        />
      </RuleRecordFrame>
    );
  }

  const status = presetStatus(row.preset, snapshot);
  return (
    <RuleRecordFrame
      row={row}
      title={row.preset.label}
      subtitle={row.group}
      status={status.status}
      statusLabel={status.label}
    >
      <DetailPane
        title="Detail Workspace"
        subtitle="Plain-language setup preview and next steps for this suggested rule."
        status={status.status}
        statusLabel={status.label}
        tabs={[
          {
            id: "overview",
            label: "Overview",
            content: (
              <div className="space-y-4">
                <div className="rounded-lg border bg-background/60 p-4">
                  <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                    Plain rule idea
                  </div>
                  <p className="mt-2 text-sm leading-6 text-muted-foreground">
                    {row.preset.description}
                  </p>
                </div>
                <div className="grid gap-3 md:grid-cols-3">
                  <div className="rounded-lg border bg-background/60 p-4">
                    <div className="text-xs text-muted-foreground">
                      Activity
                    </div>
                    <div className="mt-1 text-sm font-semibold">
                      {row.preset.category === "unknown"
                        ? "All activity"
                        : categoryLabel(row.preset.category)}
                    </div>
                  </div>
                  <div className="rounded-lg border bg-background/60 p-4">
                    <div className="text-xs text-muted-foreground">
                      Behavior
                    </div>
                    <div className="mt-1 text-sm font-semibold">
                      {labelize(row.preset.behavior)}
                    </div>
                  </div>
                  <div className="rounded-lg border bg-background/60 p-4">
                    <div className="text-xs text-muted-foreground">
                      Device support
                    </div>
                    <div className="mt-1 text-sm font-semibold">
                      {status.label}
                    </div>
                  </div>
                </div>
                <p className="rounded-lg border border-blue-500/20 bg-blue-500/10 p-4 text-sm leading-6 text-blue-700">
                  {status.detail}
                </p>
              </div>
            ),
          },
          {
            id: "start",
            label: "Start Rule",
            content: (
              <div className="space-y-3">
                <div className="rounded-lg border bg-background/60 p-4">
                  <h4 className="text-sm font-semibold">
                    Create this rule from a guided flow
                  </h4>
                  <p className="mt-2 text-sm leading-6 text-muted-foreground">
                    This opens the simple rule builder with the right intent
                    preselected. If Pollek can only observe this category on
                    your OS, use the Activity evidence to adjust the AI app
                    settings too.
                  </p>
                </div>
                <Link
                  to={`/protect?intent=${encodeURIComponent(row.preset.intent)}`}
                  className="inline-flex h-9 items-center gap-2 rounded-md bg-primary px-3 text-sm text-primary-foreground hover:bg-primary/90"
                >
                  <Plus className="h-4 w-4" />
                  Start with this rule
                </Link>
              </div>
            ),
          },
          ...(showTechnicalDetails
            ? [
                {
                  id: "technical",
                  label: "Technical Details",
                  content: (
                    <Collapsible title="Snapshot JSON">
                      <pre className="overflow-auto rounded-none border-0 bg-transparent p-0 text-[11px]">
                        {JSON.stringify(row.preset, null, 2)}
                      </pre>
                    </Collapsible>
                  ),
                },
              ]
            : []),
        ]}
      />
    </RuleRecordFrame>
  );
}

export function AllowedBlockedPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const { mode } = useMode();
  const showTechnicalDetails = isAdvanceMode(mode);
  const [policies, setPolicies] = useState<PolicyDraft[]>([]);
  const [snapshot, setSnapshot] = useState<LocalCapabilitySnapshotV2 | null>(
    null,
  );
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState(searchParams.get("q") ?? "");
  const selectedId = searchParams.get("selected") ?? undefined;

  const fetchData = () => {
    setLoading(true);
    Promise.all([
      PolicyApi.list().catch(() => [] as PolicyDraft[]),
      CapabilityApi.getSnapshotV2("desktop_simple").catch(() => null),
    ])
      .then(([policyList, capabilitySnapshot]) => {
        setPolicies(policyList);
        setSnapshot(capabilitySnapshot);
      })
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    fetchData();
    const handleRefresh = () => fetchData();
    window.addEventListener("refresh-policies", handleRefresh);
    return () => window.removeEventListener("refresh-policies", handleRefresh);
  }, []);

  const activePolicies = useMemo(
    () =>
      policies.filter((policy) =>
        ["published", "active", "approved", "validated"].includes(
          policy.meta?.status ?? "",
        ),
      ),
    [policies],
  );
  const draftPolicies = policies.filter(
    (policy) => !activePolicies.includes(policy),
  );
  const ruleRows = useMemo<RuleRow[]>(
    () => [
      ...activePolicies.map((policy) => ({
        id: `policy:${policy.policy_id}`,
        group: "Active rules" as const,
        policy,
      })),
      ...SIMPLE_RULE_PRESETS.map((preset) => ({
        id: `preset:${preset.id}`,
        group: "Suggested rules" as const,
        preset,
      })),
      ...draftPolicies.map((policy) => ({
        id: `draft:${policy.policy_id}`,
        group: "Draft policies" as const,
        policy,
      })),
    ],
    [activePolicies, draftPolicies],
  );
  const filteredRules = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return ruleRows;
    return ruleRows.filter((row) => {
      const text = row.policy
        ? [
            row.policy.name,
            row.policy.description,
            row.policy.policy_type,
            row.group,
          ]
        : [
            row.preset.label,
            row.preset.description,
            row.preset.category,
            row.preset.behavior,
            row.group,
          ];
      return text.filter(Boolean).join(" ").toLowerCase().includes(query);
    });
  }, [ruleRows, search]);

  const handleSelect = (rowId: string) => {
    const next = new URLSearchParams(searchParams);
    if (rowId) next.set("selected", rowId);
    else next.delete("selected");
    if (search.trim()) next.set("q", search.trim());
    else next.delete("q");
    setSearchParams(next, { replace: true });
  };

  return (
    <div className="space-y-5">
      {!selectedId && (
        <>
          <PageHeader
            title="Allowed & Blocked"
            subtitle="Choose what each AI app is allowed to do, and see where Pollek can only watch for now."
            icon={ListChecks}
            actions={
              <>
                <Link
                  to="/protect"
                  className="inline-flex h-9 items-center gap-2 rounded-md bg-primary px-3 text-sm text-primary-foreground hover:bg-primary/90"
                >
                  <Plus className="h-4 w-4" />
                  Create rule
                </Link>
                <Link
                  to="/policies"
                  className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
                >
                  <FileKey className="h-4 w-4" />
                  Advanced policies
                </Link>
              </>
            }
          />

          <section className="grid gap-3 sm:grid-cols-3">
            <div className="rounded-lg border bg-card/60 p-4">
              <div className="text-2xl font-semibold">
                {activePolicies.length}
              </div>
              <p className="mt-1 text-xs text-muted-foreground">Active rules</p>
            </div>
            <div className="rounded-lg border bg-card/60 p-4">
              <div className="text-2xl font-semibold">
                {draftPolicies.length}
              </div>
              <p className="mt-1 text-xs text-muted-foreground">Draft rules</p>
            </div>
            <div className="rounded-lg border bg-card/60 p-4">
              <div className="text-2xl font-semibold">
                {SIMPLE_RULE_PRESETS.length}
              </div>
              <p className="mt-1 text-xs text-muted-foreground">
                Plain presets
              </p>
            </div>
          </section>

          <section className="rounded-lg border bg-card/60 p-4">
            <label className="relative block">
              <span className="sr-only">Search rules</span>
              <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
              <input
                value={search}
                onChange={(event) => {
                  const value = event.target.value;
                  setSearch(value);
                  const next = new URLSearchParams(searchParams);
                  if (value.trim()) next.set("q", value.trim());
                  else next.delete("q");
                  next.delete("selected");
                  setSearchParams(next, { replace: true });
                }}
                placeholder="Search AI app, file, website, command, rule..."
                className="h-9 w-full rounded-md border bg-background pl-9 pr-3 text-sm"
              />
            </label>
          </section>
        </>
      )}

      <MasterDetailLayout
        items={filteredRules}
        selectedId={selectedId}
        onSelect={handleSelect}
        idSelector={(row) => row.id}
        loading={loading}
        detailBackLabel="Back to all rules"
        emptyState={
          <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
            No rules match this view yet.
          </div>
        }
        renderGroupHeader={(row, index, prevRow) => {
          if (index > 0 && prevRow?.group === row.group) return null;
          return (
            <div className="px-2 py-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              {row.group}
            </div>
          );
        }}
        renderCard={(row, selected) =>
          row.policy ? (
            <PolicyCard policy={row.policy} selected={selected} />
          ) : (
            <PresetCard
              preset={row.preset}
              snapshot={snapshot}
              selected={selected}
            />
          )
        }
        renderDetail={(row) => (
          <RuleDetail
            key={row.id}
            row={row}
            snapshot={snapshot}
            showTechnicalDetails={showTechnicalDetails}
          />
        )}
      />
    </div>
  );
}
