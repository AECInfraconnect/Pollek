import { type ReactNode, useEffect, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import {
  Activity,
  Bot,
  BookOpen,
  ChevronDown,
  ChevronUp,
  CheckCircle2,
  Clock3,
  CircleDollarSign,
  Database,
  FileKey,
  Fingerprint,
  FolderTree,
  Gauge,
  Globe2,
  Shield,
  ShieldAlert,
  ShieldCheck,
  Terminal,
  Trash2,
  Wrench,
} from "lucide-react";
import {
  Entity360Page,
  type DetailSection,
  type RelatedSection,
} from "../components/entity-360";
import { MasterDetailLayout } from "../components/master-detail/MasterDetailLayout";
import { PageHeader } from "../components/layout/PageHeader";
import type { RelatedListItem } from "../components/entity-360/RelatedList";
import { entityIcon } from "../features/entity-graph/graphUtils";
import type {
  Entity360Response,
  GraphNode,
} from "../features/entity-graph/types";
import { useEntity360 } from "../features/entity-graph/useEntity360";
import { useConfirm } from "../components/ui/ConfirmDialog";
import { RegistryApi, type AiAgent } from "../services/api";
import type { UiStatus } from "../lib/status";
import { cn } from "../lib/utils";
import { formatDisplayValue, renderDisplayValue } from "../lib/displayValue";
import {
  assessExpectedCapabilities,
  findAgentReferenceIntel,
  matchObserveGuideSignals,
} from "../lib/entityReferenceIntel";
import {
  ReferenceIntelInline,
  ReferenceIntelMark,
} from "../components/reference/ReferenceIntelMark";
import { ReferenceIntelGuide } from "../components/reference/ReferenceIntelGuide";
import { UserActivityApi } from "../features/user-activity/api";
import type { UserFriendlyActivityEvent } from "../features/user-activity/types";
import { AgentActivityTab } from "../components/agents/AgentActivityTab";
import { AgentEnforcementTab } from "../components/agents/AgentEnforcementTab";
import { AgentUsagePanel } from "../components/usage/AgentUsagePanel";
import { toast } from "sonner";

function useAgents() {
  const [agents, setAgents] = useState<AiAgent[]>([]);
  const [activity, setActivity] = useState<UserFriendlyActivityEvent[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([
      RegistryApi.listAgents(),
      UserActivityApi.list({ limit: 300 }),
    ])
      .then(([nextAgents, nextActivity]) => {
        setAgents(nextAgents);
        setActivity(nextActivity.items);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  return { agents, activity, loading };
}

function useDeleteAgent() {
  const { confirm } = useConfirm();
  return async (id: string) => {
    if (
      !(await confirm({
        title: "Delete Agent",
        description:
          "Delete this agent? Pollek will remove its local registry record, linked policies, setup properties, and registration metadata. If Auto Discovery finds it again later, it will appear as Pending and can be registered again.",
        confirmText: "Delete",
        danger: true,
      }))
    ) {
      return false;
    }

    try {
      await RegistryApi.deleteAgent(id);
      toast.success("Agent deleted");
      return true;
    } catch {
      toast.error("Failed to delete agent");
      return false;
    }
  };
}

function agentStatus(agent: AiAgent): {
  status: UiStatus;
  label: string;
  tone: "success" | "warning" | "danger" | "info";
} {
  if (agent.enforcement_mode === "Enforce") {
    return { status: "ok", label: "Protected", tone: "success" };
  }
  if (agent.enforcement_mode === "Observe") {
    return { status: "info", label: "Observing", tone: "info" };
  }
  if (agent.enforcement_mode === "Shadow") {
    return { status: "degraded", label: "Shadow AI", tone: "warning" };
  }
  return {
    status: "info",
    label: agent.enforcement_mode || "Registered",
    tone: "info",
  };
}

function buildRelatedSections(
  nodes: GraphNode[],
  centerId: string,
): RelatedSection[] {
  const related = nodes.filter((node) => node.id !== centerId);
  const policies = related.filter((node) => node.type === "policy");
  const tools = related.filter((node) => node.type === "tool");
  const resources = related.filter((node) => node.type === "resource");
  const others = related.filter(
    (node) => !["policy", "tool", "resource"].includes(node.type),
  );

  const sections: RelatedSection[] = [
    {
      title: "Policies",
      icon: FileKey,
      iconColor: "text-amber-600",
      items: policies.map(
        (policy): RelatedListItem => ({
          id: policy.id,
          icon: Shield,
          iconColor: "text-amber-600",
          title: policy.label,
          subtitle: policy.subtitle ?? undefined,
          href: `/policies?id=${policy.entity_id}`,
          badge: policy.status
            ? {
                label: policy.status,
                tone:
                  policy.status === "enforcing"
                    ? "success"
                    : policy.status === "observe"
                      ? "info"
                      : "neutral",
              }
            : undefined,
          meta: policy.metrics.map((metric) => ({
            label: metric.label,
            value: metric.value,
          })),
        }),
      ),
      viewAllHref: "/policies",
    },
    {
      title: "Tools",
      icon: Wrench,
      iconColor: "text-blue-600",
      items: tools.map(
        (tool): RelatedListItem => ({
          id: tool.id,
          icon: Wrench,
          iconColor: "text-blue-600",
          title: tool.label,
          subtitle: tool.subtitle ?? undefined,
          href: `/tools?id=${tool.entity_id}`,
          badge: tool.status
            ? {
                label: tool.status,
                tone: tool.status === "active" ? "success" : "neutral",
              }
            : undefined,
          meta: tool.metrics.map((metric) => ({
            label: metric.label,
            value: metric.value,
          })),
        }),
      ),
      viewAllHref: "/tools",
    },
    {
      title: "Resources",
      icon: Database,
      iconColor: "text-purple-600",
      items: resources.map(
        (resource): RelatedListItem => ({
          id: resource.id,
          icon: Database,
          iconColor: "text-purple-600",
          title: resource.label,
          subtitle: resource.subtitle ?? undefined,
          href: `/resources?id=${resource.entity_id}`,
          meta: resource.metrics.map((metric) => ({
            label: metric.label,
            value: metric.value,
          })),
        }),
      ),
      viewAllHref: "/resources",
    },
  ];

  if (others.length > 0) {
    sections.push({
      title: "Other Entities",
      icon: FolderTree,
      items: others.map(
        (other): RelatedListItem => ({
          id: other.id,
          icon: entityIcon(other.type),
          title: other.label,
          subtitle: `${other.type} - ${other.status}`,
        }),
      ),
    });
  }

  return sections;
}

function formatDateTime(value?: string) {
  if (!value) return "Not recorded";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

function summarizeSource(agent: AiAgent) {
  const source = agent.meta?.source ?? "registry";
  if (source === "discovery") return "Auto Discovery";
  if (source === "agent_self_registration") return "Agent self registration";
  if (source === "cloud_sync") return "Pollek Cloud sync";
  return source.replace(/_/g, " ");
}

function referencesForAgent(agent: AiAgent) {
  return findAgentReferenceIntel({
    name: agent.name,
    vendor: agent.vendor,
    agentType: agent.agent_type,
    runtimeName: agent.runtime?.runtime_name,
  });
}

function observedTermsForAgent(
  agent: AiAgent,
  data?: Entity360Response | null,
) {
  return [
    agent.name,
    agent.vendor,
    agent.agent_type,
    agent.runtime?.runtime_name,
    agent.runtime?.version,
    agent.identity?.process_path,
    ...(agent.capabilities ?? []),
    ...(agent.declared_tools ?? []),
    ...(agent.declared_resources ?? []),
    ...(data?.graph.nodes ?? []).flatMap((node) => [
      node.label,
      node.subtitle,
      node.type,
      node.status,
      node.mode,
    ]),
    ...(data?.activity ?? [])
      .slice(0, 12)
      .flatMap((item) => [
        item.action,
        item.explanation,
        item.tool?.label,
        item.resource?.label,
        item.resource?.type,
        item.decision,
        item.enforcement_mode,
      ]),
  ].filter(
    (term): term is string =>
      typeof term === "string" && term.trim().length > 0,
  );
}

function activityForAgent(
  agent: AiAgent,
  activity: UserFriendlyActivityEvent[],
) {
  const normalizedName = agent.name.toLowerCase();
  return activity
    .filter((item) => {
      if (item.agent_id && item.agent_id === agent.agent_id) return true;
      return item.agent_name.toLowerCase() === normalizedName;
    })
    .sort(
      (left, right) =>
        new Date(right.timestamp).getTime() -
        new Date(left.timestamp).getTime(),
    );
}

function agentDetailSections(
  agent: AiAgent,
  data: Entity360Response | null | undefined,
  statusLabel: string,
): DetailSection[] {
  const graphNodes = data?.graph.nodes ?? [];
  const relatedTools = graphNodes.filter((node) => node.type === "tool").length;
  const relatedResources = graphNodes.filter(
    (node) => node.type === "resource",
  ).length;
  const relatedPolicies = graphNodes.filter(
    (node) => node.type === "policy",
  ).length;
  const tokenBindings = agent.identity?.token_bindings ?? [];
  const source = summarizeSource(agent);
  const referenceIntel = referencesForAgent(agent);
  const capabilityChecklist = assessExpectedCapabilities(referenceIntel, [
    ...(agent.capabilities ?? []),
    ...(agent.declared_tools ?? []),
    ...(agent.declared_resources ?? []),
    agent.agent_type,
    agent.runtime?.runtime_name ?? "",
  ]);
  const lifecycle = `${formatDateTime(agent.meta?.created_at)} -> ${formatDateTime(
    agent.meta?.updated_at,
  )}`;

  const sections: DetailSection[] = [
    {
      title: "Current Status",
      description:
        "Operational state resolved from registry values and latest entity telemetry.",
      icon: Gauge,
      fields: [
        {
          label: "Status",
          value: statusLabel,
          status: statusLabel === "Protected" ? "ok" : "info",
          source,
          history: lifecycle,
        },
        {
          label: "Enforcement",
          value: agent.enforcement_mode ?? "Not configured",
          status:
            agent.enforcement_mode === "Enforce"
              ? "ok"
              : agent.enforcement_mode === "Shadow"
                ? "warning"
                : "info",
          source: "registry.enforcement_mode",
          history: `Last updated ${formatDateTime(agent.meta?.updated_at)}`,
        },
        {
          label: "Trust Level",
          value: agent.trust_level,
          status:
            agent.trust_level === "high" || agent.trust_level === "system"
              ? "ok"
              : agent.trust_level === "untrusted"
                ? "danger"
                : "warning",
          source: "registry.trust_level",
        },
        {
          label: "Runtime",
          value: `${agent.runtime?.runtime_name ?? "Unknown"}${
            agent.runtime?.version ? ` ${agent.runtime.version}` : ""
          }`,
          source: "runtime fingerprint",
          confidence: data ? "entity graph confirmed" : "registry only",
        },
      ],
    },
    {
      title: "Identity Binding",
      description:
        "How this local agent is tied to process identity, SPIFFE, and cloud-capable tokens.",
      icon: Fingerprint,
      fields: [
        {
          label: "SPIFFE ID",
          value: agent.identity?.spiffe_id ?? "Not bound",
          status: agent.identity?.spiffe_id ? "ok" : "warning",
          source: "agent identity binding",
          note: agent.identity?.spiffe_id
            ? "This agent can be traced across local and cloud records."
            : "Local-only observation is available, but enterprise tracing needs a SPIFFE binding.",
        },
        {
          label: "Process Path",
          value: agent.identity?.process_path ?? "Not captured",
          status: agent.identity?.process_path ? "info" : "unknown",
          source: "process observer",
        },
        {
          label: "User Subject",
          value: agent.identity?.user_subject ?? "Local",
          source: "OS account resolver",
        },
        {
          label: "Token Bindings",
          value: tokenBindings.length,
          status: tokenBindings.length ? "ok" : "unknown",
          source: "identity binding registry",
          note: tokenBindings.length
            ? tokenBindings
                .map((token) => `${token.kind}:${token.provider}`)
                .join(", ")
            : "No OAuth/OIDC/JWT-SVID token binding has been registered.",
        },
      ],
    },
    {
      title: "Relationships",
      description:
        "Links that make this agent actionable in policies, tool calls, and data access.",
      icon: ShieldCheck,
      fields: [
        {
          label: "Policies",
          value: relatedPolicies,
          source: "entity graph",
          status: relatedPolicies ? "ok" : "warning",
          note: relatedPolicies
            ? "Policy relationships are available for impact review."
            : "No policy is currently connected to this agent.",
        },
        {
          label: "Tools",
          value: relatedTools || agent.declared_tools?.length || 0,
          source: relatedTools ? "observed graph links" : "declared tools",
        },
        {
          label: "Resources",
          value: relatedResources || agent.declared_resources?.length || 0,
          source: relatedResources
            ? "observed graph links"
            : "declared resources",
        },
        {
          label: "Capabilities",
          value: agent.capabilities?.length ?? 0,
          source: "capability inventory",
          note:
            agent.capabilities?.slice(0, 8).join(", ") ||
            "No explicit capability tags recorded.",
        },
      ],
    },
    {
      title: "Data Sources & History",
      description:
        "Where record values came from, when they changed, and whether they are local or synced.",
      icon: Clock3,
      fields: [
        {
          label: "Primary Source",
          value: source,
          source: "object meta",
        },
        {
          label: "Created",
          value: formatDateTime(agent.meta?.created_at),
          source: "registry object meta",
        },
        {
          label: "Updated",
          value: formatDateTime(agent.meta?.updated_at),
          source: "registry object meta",
        },
        {
          label: "Labels",
          value: Object.keys(agent.labels ?? {}).length,
          source: "registry.labels",
          note: Object.entries(agent.labels ?? {})
            .map(([key, value]) => `${key}=${value}`)
            .join(", "),
        },
      ],
    },
  ];

  if (referenceIntel.length > 0) {
    sections.push({
      title: "Reference Intel",
      description:
        "Well-known external context matched from observed names, vendors, hosts, or versions. This enriches the record but is not enforcement evidence.",
      icon: BookOpen,
      fields: referenceIntel.map((reference) => ({
        label: reference.title,
        value: (
          <a
            href={reference.sourceUrl}
            target="_blank"
            rel="noreferrer"
            className="text-primary underline-offset-4 hover:underline"
          >
            {reference.category}
          </a>
        ),
        status: "info",
        source: reference.sourceLabel,
        history: `Reviewed ${reference.reviewedAt}`,
        note: `${reference.description} Control note: ${reference.controlNotes}`,
      })),
    });
  }

  if (capabilityChecklist.length > 0) {
    sections.push({
      title: "Known Capability Checklist",
      description:
        "Standard capabilities expected for matched well-known entities. Green means local evidence detected a matching capability.",
      icon: CheckCircle2,
      fields: capabilityChecklist.map((capability) => ({
        label: capability.label,
        value: capability.detected ? "Detected" : "Not observed yet",
        status: capability.detected ? "ok" : "unknown",
        source: `definition:${capability.referenceTitle}`,
        note: capability.detected
          ? "Matched against observed or declared local capability evidence."
          : "Expected by reference intel, but not yet confirmed by local evidence.",
      })),
    });
  }

  return sections;
}

export function AgentDetailView({
  agent,
  activity = [],
  onDelete,
}: {
  agent: AiAgent;
  activity?: UserFriendlyActivityEvent[];
  onDelete: () => void;
}) {
  const navigate = useNavigate();
  const { data } = useEntity360("agent", agent.agent_id);
  const { label, tone } = agentStatus(agent);
  const primaryReference = referencesForAgent(agent)[0];
  const observedTerms = observedTermsForAgent(agent, data);

  const relatedSections = data
    ? buildRelatedSections(data.graph.nodes, data.entity.id)
    : [];
  const detailSections = agentDetailSections(agent, data, label);

  return (
    <Entity360Page
      header={{
        entityType: "Agent",
        entityName: agent.name,
        icon: Bot,
        helpTopicId: "entity.agent",
        visual: primaryReference ? (
          <ReferenceIntelMark reference={primaryReference} />
        ) : undefined,
        status: { label, tone },
        badges: [
          ...(agent.runtime?.runtime_name
            ? [{ label: agent.runtime.runtime_name }]
            : []),
          ...(agent.trust_level
            ? [{ label: `Trust: ${agent.trust_level}` }]
            : []),
        ],
        subtitle: agent.identity?.spiffe_id ?? "Local process agent",
        actions: (
          <>
            <button
              type="button"
              onClick={() => navigate(`/policies?agent=${agent.agent_id}`)}
              className="inline-flex h-9 items-center gap-1.5 rounded-lg bg-primary px-4 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
            >
              <Shield className="h-3.5 w-3.5" />
              Apply Policy
            </button>
            <button
              type="button"
              onClick={onDelete}
              className="inline-flex h-9 items-center gap-1.5 rounded-lg border border-red-500/30 bg-red-500/10 px-3 text-sm font-medium text-red-600 transition-colors hover:bg-red-500/15"
              aria-label="Delete agent"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </button>
          </>
        ),
        meta: [
          { label: "Type", value: agent.agent_type },
          { label: "Version", value: agent.runtime?.version ?? "Unknown" },
          {
            label: "Identity",
            value: agent.identity?.spiffe_id ? "SPIFFE" : "Local",
          },
        ],
      }}
      aboutSection={<AgentAboutSection agent={agent} />}
      relatedSections={relatedSections}
      data={data}
      detailSections={detailSections}
      extraTabs={[
        {
          id: "observe-coverage",
          label: "Observe Coverage",
          icon: ShieldAlert,
          content: (
            <AgentObserveCoverage
              agent={agent}
              activity={activity}
              data={data}
            />
          ),
        },
        {
          id: "activity",
          label: "Activity",
          icon: Activity,
          content: <AgentActivityTab agentId={agent.agent_id} />,
        },
        {
          id: "usage",
          label: "Usage & Cost",
          icon: CircleDollarSign,
          content: (
            <AgentUsagePanel
              agentId={agent.agent_id}
              agentName={agent.name}
              agentType={agent.agent_type}
            />
          ),
        },
        {
          id: "enforcement",
          label: "Enforcement",
          icon: Shield,
          content: <AgentEnforcementTab agentId={agent.agent_id} />,
        },
        {
          id: "capabilities",
          label: "Capabilities",
          icon: Gauge,
          content: <AgentCapabilities agent={agent} data={data} />,
        },
        {
          id: "known-profile",
          label: "Known Profile",
          icon: BookOpen,
          content: primaryReference ? (
            <ReferenceIntelGuide
              reference={primaryReference}
              observedTerms={observedTerms}
            />
          ) : (
            <div className="rounded-lg border border-dashed p-6 text-sm text-muted-foreground">
              No well-known reference profile matched this agent yet.
            </div>
          ),
        },
      ]}
    />
  );
}

type CoverageState = "observed" | "watching" | "needs_setup" | "not_seen";

function coverageStateLabel(state: CoverageState) {
  if (state === "observed") return "Observed";
  if (state === "watching") return "Watching only";
  if (state === "needs_setup") return "Needs setup";
  return "No evidence yet";
}

function coverageStateClass(state: CoverageState) {
  if (state === "observed") {
    return "border-emerald-500/25 bg-emerald-500/10 text-emerald-700 dark:text-emerald-200";
  }
  if (state === "watching") {
    return "border-blue-500/25 bg-blue-500/10 text-blue-700 dark:text-blue-200";
  }
  if (state === "needs_setup") {
    return "border-amber-500/25 bg-amber-500/10 text-amber-700 dark:text-amber-200";
  }
  return "border-border bg-background text-muted-foreground";
}

function hasCapability(agent: AiAgent, terms: string[]) {
  const haystack = [
    agent.agent_type,
    agent.runtime?.runtime_name,
    ...(agent.capabilities ?? []),
    ...(agent.declared_tools ?? []),
    ...(agent.declared_resources ?? []),
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
  return terms.some((term) => haystack.includes(term));
}

function promptGuardStatus(
  agent: AiAgent,
  safetyEvents: UserFriendlyActivityEvent[],
): {
  label: string;
  state: CoverageState;
  source: string;
  detail: string;
  next: string;
} {
  if (safetyEvents.length > 0) {
    return {
      label: "Active in path",
      state: "observed",
      source: "Prompt Guard telemetry",
      detail:
        safetyEvents[0]?.plain_summary ||
        "Prompt or private-data safety telemetry is linked to this AI app.",
      next: "Open the safety center to review watched, redacted, or blocked incidents.",
    };
  }

  if (hasCapability(agent, ["prompt", "guard", "pii", "redact", "safety"])) {
    return {
      label: "Watching only",
      state: "watching",
      source: "Declared capability or definition",
      detail:
        "Pollek has a safety-related signal for this AI app, but no incident has been observed yet.",
      next: "Run the AI app through a guarded path, then verify an incident or clean watch result appears in Prompt Guard.",
    };
  }

  if (hasCapability(agent, ["browser", "web", "chat", "llm"])) {
    return {
      label: "Needs browser/proxy integration",
      state: "needs_setup",
      source: "Browser or model-surface definition",
      detail:
        "Pollek can identify this AI surface, but prompt filtering requires a browser extension, local proxy, wrapper, response filter, SDK adapter, or MCP proxy in the data path.",
      next: "Use Check setup to choose the safest integration available on this OS and AI app.",
    };
  }

  return {
    label: "Not configured",
    state: "needs_setup",
    source: "No guarded path detected",
    detail:
      "No Prompt Guard, redaction, secret, PII, or prompt-injection path is linked to this AI app yet.",
    next: "Enable Prompt Guard only where you want prompt/output safety checks, or use the AI app's own safety settings.",
  };
}

function AgentObserveCoverage({
  agent,
  activity,
  data,
}: {
  agent: AiAgent;
  activity: UserFriendlyActivityEvent[];
  data?: Entity360Response | null;
}) {
  const agentEvents = activityForAgent(agent, activity);
  const graphActivity = data?.activity ?? [];
  const graphNodes = data?.graph.nodes ?? [];
  const toolNodes = graphNodes.filter((node) => node.type === "tool");
  const resourceNodes = graphNodes.filter((node) => node.type === "resource");
  const policyNodes = graphNodes.filter((node) => node.type === "policy");
  const fileEvents = agentEvents.filter((event) => event.category === "files");
  const webEvents = agentEvents.filter((event) => event.category === "web");
  const commandEvents = agentEvents.filter(
    (event) => event.category === "commands" || event.category === "apps",
  );
  const toolEvents = agentEvents.filter((event) => event.category === "tools");
  const costEvents = agentEvents.filter(
    (event) => event.category === "cost" || event.category === "ai_models",
  );
  const safetyEvents = agentEvents.filter(
    (event) => event.category === "safety",
  );
  const guardStatus = promptGuardStatus(agent, safetyEvents);
  const eventsWithRules = agentEvents.filter((event) => event.rule_label);
  const tokenTotal =
    agentEvents.reduce((total, event) => total + (event.tokens ?? 0), 0) +
    graphActivity.reduce(
      (total, event) => total + (event.cost?.total_tokens ?? 0),
      0,
    );
  const costTotal =
    agentEvents.reduce((total, event) => total + (event.cost_usd ?? 0), 0) +
    graphActivity.reduce(
      (total, event) => total + (event.cost?.total_cost_usd ?? 0),
      0,
    );

  const coverage = [
    {
      id: "files",
      label: "Files and folders",
      icon: FolderTree,
      state:
        fileEvents.length || resourceNodes.length
          ? "observed"
          : hasCapability(agent, ["file", "filesystem", "workspace"])
            ? "watching"
            : "needs_setup",
      count: fileEvents.length + resourceNodes.length,
      detail:
        fileEvents[0]?.target_label ||
        resourceNodes[0]?.label ||
        "No file or folder path has been linked to this agent yet.",
      next: "Run Observe while the AI app reads or writes a workspace folder, or connect a filesystem observer/wrapper.",
    },
    {
      id: "web",
      label: "Websites and network",
      icon: Globe2,
      state:
        webEvents.length || hasCapability(agent, ["browser", "network", "web"])
          ? webEvents.length
            ? "observed"
            : "watching"
          : "needs_setup",
      count: webEvents.length,
      detail:
        webEvents[0]?.target_label ||
        "Browser-only AI apps may need a browser extension, proxy, or wrapper for exact website activity.",
      next: "Install or enable the browser/proxy path if you need exact website and prompt flow visibility.",
    },
    {
      id: "commands",
      label: "Commands and local apps",
      icon: Terminal,
      state:
        commandEvents.length || hasCapability(agent, ["terminal", "command"])
          ? commandEvents.length
            ? "observed"
            : "watching"
          : "needs_setup",
      count: commandEvents.length,
      detail:
        commandEvents[0]?.target_label ||
        "No command execution event has been observed for this agent.",
      next: "Use a CLI/IDE wrapper or OS process observer if this AI app can run commands.",
    },
    {
      id: "tools",
      label: "Tools and MCP",
      icon: Wrench,
      state:
        toolEvents.length || toolNodes.length || agent.declared_tools?.length
          ? toolEvents.length || toolNodes.length
            ? "observed"
            : "watching"
          : "not_seen",
      count:
        toolEvents.length +
        toolNodes.length +
        (agent.declared_tools?.length ?? 0),
      detail:
        toolEvents[0]?.target_label ||
        toolNodes[0]?.label ||
        agent.declared_tools?.[0] ||
        "No tool or MCP use has been linked yet.",
      next: "Route tool calls through a supported wrapper, MCP proxy, or plugin connector for richer evidence.",
    },
    {
      id: "cost",
      label: "AI usage and cost",
      icon: CircleDollarSign,
      state:
        costEvents.length || tokenTotal || costTotal
          ? "observed"
          : hasCapability(agent, ["model", "llm", "chat"])
            ? "watching"
            : "needs_setup",
      count:
        costEvents.length + graphActivity.filter((event) => event.cost).length,
      detail:
        tokenTotal || costTotal
          ? `${tokenTotal.toLocaleString()} tokens, $${costTotal.toFixed(4)}`
          : "Exact usage needs provider telemetry, wrapper/proxy logs, local logs, or a plugin connector.",
      next: "Open AI Usage & Cost to inspect exact vs estimated usage and improve exact tracking.",
    },
    {
      id: "safety",
      label: "Prompt Guard and private data",
      icon: ShieldCheck,
      state:
        safetyEvents.length || hasCapability(agent, ["prompt", "guard", "pii"])
          ? safetyEvents.length
            ? "observed"
            : "watching"
          : "needs_setup",
      count: safetyEvents.length,
      detail:
        safetyEvents[0]?.plain_summary ||
        "No prompt injection, secret, PII, masking, or redaction event is linked to this agent yet.",
      next: "Use a guarded prompt/output path such as wrapper, SDK adapter, MCP proxy, response filter, or browser extension.",
    },
    {
      id: "rules",
      label: "Rules and setup",
      icon: FileKey,
      state:
        eventsWithRules.length || policyNodes.length
          ? "observed"
          : agent.enforcement_mode === "Observe"
            ? "watching"
            : "needs_setup",
      count: eventsWithRules.length + policyNodes.length,
      detail:
        eventsWithRules[0]?.rule_label ||
        policyNodes[0]?.label ||
        "No rule is connected to this agent yet.",
      next: "Create a rule from an activity event, then check setup to see whether Pollek can watch, ask first, or block.",
    },
  ].map((item) => ({
    ...item,
    detail: formatDisplayValue(item.detail),
    next: formatDisplayValue(item.next),
  })) as Array<{
    id: string;
    label: string;
    icon: typeof FolderTree;
    state: CoverageState;
    count: number;
    detail: string;
    next: string;
  }>;

  const observedCount = coverage.filter(
    (item) => item.state === "observed",
  ).length;
  const needsSetupCount = coverage.filter(
    (item) => item.state === "needs_setup",
  ).length;

  return (
    <div className="space-y-4" data-testid="agent-observe-coverage">
      <section className="rounded-lg border bg-background/40 p-4">
        <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
          <div>
            <h3 className="text-sm font-semibold">
              What Pollek can see for this AI app
            </h3>
            <p className="mt-1 max-w-3xl text-sm leading-6 text-muted-foreground">
              This combines observed activity, registry relationships, declared
              capabilities, and well-known definitions. It is honest about gaps:
              watching is not the same as blocking, and browser-only apps often
              need an extension, proxy, wrapper, or plugin for exact evidence.
            </p>
          </div>
          <div className="flex flex-wrap gap-2 text-xs">
            <span className="rounded-full border bg-background px-2.5 py-1">
              {observedCount}/{coverage.length} observed
            </span>
            <span className="rounded-full border bg-background px-2.5 py-1">
              {needsSetupCount} need setup
            </span>
          </div>
        </div>
      </section>

      <section
        data-testid="agent-prompt-guard-status"
        className="rounded-lg border border-emerald-500/20 bg-emerald-500/10 p-4"
      >
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
          <div className="flex min-w-0 items-start gap-3">
            <div className="rounded-lg bg-emerald-500/15 p-2 text-emerald-700 dark:text-emerald-200">
              <ShieldCheck className="h-4 w-4" />
            </div>
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <h3 className="text-sm font-semibold">
                  Prompt Guard status for this AI app
                </h3>
                <span
                  className={cn(
                    "rounded-full border px-2 py-0.5 text-[11px] font-medium",
                    coverageStateClass(guardStatus.state),
                  )}
                >
                  {guardStatus.label}
                </span>
              </div>
              <p className="mt-2 text-sm leading-6 text-emerald-950/80 dark:text-emerald-100/80">
                {renderDisplayValue(guardStatus.detail)}
              </p>
              <div className="mt-3 flex flex-wrap gap-2 text-xs">
                <span className="rounded border border-emerald-500/20 bg-background/70 px-2 py-1">
                  Source: {guardStatus.source}
                </span>
                <span className="rounded border border-emerald-500/20 bg-background/70 px-2 py-1">
                  Incidents: {safetyEvents.length}
                </span>
              </div>
              <p className="mt-3 text-xs leading-5 text-emerald-950/75 dark:text-emerald-100/75">
                {renderDisplayValue(guardStatus.next)}
              </p>
            </div>
          </div>
          <div className="flex shrink-0 flex-wrap gap-2">
            <Link
              to="/alerts?tab=guard"
              className="inline-flex h-9 items-center gap-2 rounded-md bg-emerald-600 px-3 text-sm font-medium text-white hover:bg-emerald-700"
            >
              <ShieldCheck className="h-4 w-4" />
              Open safety center
            </Link>
            <Link
              to="/setup?category=safety"
              className="inline-flex h-9 items-center gap-2 rounded-md border border-emerald-500/25 bg-background/70 px-3 text-sm hover:bg-background"
            >
              <Wrench className="h-4 w-4" />
              Check setup
            </Link>
          </div>
        </div>
      </section>

      <div className="grid gap-3 md:grid-cols-2">
        {coverage.map((item) => {
          const Icon = item.icon;
          return (
            <section
              key={item.id}
              data-testid={`agent-observe-coverage-${item.id}`}
              className="rounded-lg border bg-background/40 p-4"
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex min-w-0 items-start gap-3">
                  <div className="rounded-lg bg-primary/10 p-2 text-primary">
                    <Icon className="h-4 w-4" />
                  </div>
                  <div className="min-w-0">
                    <h4 className="text-sm font-semibold">{item.label}</h4>
                    <p className="mt-1 break-words text-xs leading-5 text-muted-foreground">
                      {renderDisplayValue(item.detail)}
                    </p>
                  </div>
                </div>
                <span
                  className={cn(
                    "shrink-0 rounded-full border px-2 py-0.5 text-[11px] font-medium",
                    coverageStateClass(item.state as CoverageState),
                  )}
                >
                  {coverageStateLabel(item.state as CoverageState)}
                </span>
              </div>
              <div className="mt-3 flex flex-wrap gap-2 text-xs text-muted-foreground">
                <span className="rounded border bg-card/60 px-2 py-1">
                  Evidence: {item.count}
                </span>
              </div>
              <p className="mt-3 text-xs leading-5 text-muted-foreground">
                {renderDisplayValue(item.next)}
              </p>
            </section>
          );
        })}
      </div>
    </div>
  );
}

function AgentAboutSection({ agent }: { agent: AiAgent }) {
  const primaryReference = referencesForAgent(agent)[0];

  return (
    <div className="space-y-3">
      {primaryReference && (
        <PropertyRow
          label="Known Entity"
          value={<ReferenceIntelInline reference={primaryReference} />}
        />
      )}
      <PropertyRow label="Agent ID" value={agent.agent_id} />
      <PropertyRow label="Type" value={agent.agent_type} />
      <PropertyRow
        label="Runtime"
        value={agent.runtime?.runtime_name ?? "Unknown"}
      />
      <PropertyRow label="Version" value={agent.runtime?.version ?? "-"} />
      <PropertyRow label="Trust Level" value={agent.trust_level} />
      <PropertyRow label="Enforcement" value={agent.enforcement_mode ?? "-"} />
      <PropertyRow
        label="Process Path"
        value={agent.identity?.process_path ?? "-"}
      />
      <PropertyRow
        label="SPIFFE ID"
        value={agent.identity?.spiffe_id ?? "Not bound"}
      />
      <PropertyRow
        label="User Subject"
        value={agent.identity?.user_subject ?? "Local"}
      />
      <PropertyRow label="Vendor" value={agent.vendor ?? "-"} />
      <PropertyRow
        label="Declared Tools"
        value={
          agent.declared_tools?.length
            ? agent.declared_tools.join(", ")
            : "None"
        }
      />
    </div>
  );
}

function AgentCapabilities({
  agent,
  data,
}: {
  agent: AiAgent;
  data?: Entity360Response | null;
}) {
  const referenceIntel = referencesForAgent(agent);
  const observedTerms = observedTermsForAgent(agent, data);
  const expected = assessExpectedCapabilities(referenceIntel, observedTerms);
  const allCaps = [
    ...(agent.capabilities ?? []),
    ...(agent.declared_tools ?? []).map((tool) => `tool: ${tool}`),
    ...(agent.declared_resources ?? []).map(
      (resource) => `resource: ${resource}`,
    ),
  ];

  return (
    <div className="space-y-4">
      {referenceIntel[0] && (
        <ReferenceIntelGuide
          reference={referenceIntel[0]}
          observedTerms={observedTerms}
          compact
        />
      )}

      {expected.length > 0 && (
        <section className="rounded-lg border bg-background/40 p-4">
          <h3 className="text-sm font-semibold">Expected vs observed</h3>
          <p className="mt-1 text-xs leading-5 text-muted-foreground">
            These items come from well-known definitions and are compared with
            local evidence. A missing item means it has not been observed here
            yet, not that the product cannot do it.
          </p>
          <div className="mt-3 grid gap-2 sm:grid-cols-2">
            {expected.map((capability) => (
              <div
                key={`${capability.referenceId}-${capability.id}`}
                className={cn(
                  "rounded-md border p-3 text-sm",
                  capability.detected
                    ? "border-emerald-500/25 bg-emerald-500/10"
                    : "bg-card/70",
                )}
              >
                <div className="font-medium">{capability.label}</div>
                <div className="mt-1 text-xs text-muted-foreground">
                  {capability.detected
                    ? "Observed locally"
                    : "Not observed yet"}
                </div>
              </div>
            ))}
          </div>
        </section>
      )}

      <section className="rounded-lg border bg-background/40 p-4">
        <h3 className="text-sm font-semibold">Local capabilities</h3>
        <p className="mt-1 text-xs leading-5 text-muted-foreground">
          Values from registry, discovery, and entity telemetry for this device.
        </p>
        {allCaps.length ? (
          <div className="mt-3 grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
            {allCaps.map((capability) => (
              <div
                key={capability}
                className="flex items-center gap-2 rounded-lg border bg-muted/30 px-3 py-2 text-sm"
              >
                <div className="h-2 w-2 rounded-full bg-primary/60" />
                <span className="font-medium">{capability}</span>
              </div>
            ))}
          </div>
        ) : (
          <p className="mt-3 text-sm text-muted-foreground">
            No specific local capability tags have been registered for this
            agent yet.
          </p>
        )}
      </section>
    </div>
  );
}

function PropertyRow({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-2 border-b border-border/30 pb-2 last:border-0 last:pb-0">
      <span className="whitespace-nowrap text-xs text-muted-foreground">
        {label}
      </span>
      <span className="break-all text-right text-xs font-medium text-foreground/80">
        {renderDisplayValue(value)}
      </span>
    </div>
  );
}

function AgentMasterCard({
  agent,
  activity,
  selected,
}: {
  agent: AiAgent;
  activity: UserFriendlyActivityEvent[];
  selected: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  const { tone, label } = agentStatus(agent);
  const primaryReference = referencesForAgent(agent)[0];
  const agentEvents = activityForAgent(agent, activity);
  const latestEvent = agentEvents[0];
  const observedTerms = [
    ...observedTermsForAgent(agent),
    ...agentEvents.flatMap((item) => [
      item.category,
      item.action,
      item.target_kind,
      item.target_label,
      item.access_mode,
      item.result,
      item.rule_label,
    ]),
  ];
  const observeSignals = primaryReference
    ? matchObserveGuideSignals(primaryReference, observedTerms)
    : [];
  const detectedSignals = observeSignals.filter(
    (signal) => signal.detected,
  ).length;
  const totalEvidence =
    agentEvents.length +
    (agent.declared_tools?.length ?? 0) +
    (agent.declared_resources?.length ?? 0);
  const summaryText = formatDisplayValue(
    latestEvent
      ? latestEvent.plain_summary
      : (primaryReference?.description ??
          "Start Observe to collect file, web, app, tool, command, model, and safety evidence."),
  );
  const extraCapabilities = (agent.capabilities ?? []).slice(0, 5);
  const canExpand =
    summaryText.length > 130 ||
    extraCapabilities.length > 0 ||
    Boolean(agent.identity?.process_path);

  return (
    <div
      className={cn(
        "group cursor-pointer rounded-lg border bg-card/70 p-3 transition-all hover:border-primary/40 hover:bg-primary/5",
        selected && "border-primary/60 bg-primary/10 shadow-sm",
      )}
    >
      <div className="flex items-start gap-3">
        {primaryReference ? (
          <ReferenceIntelMark reference={primaryReference} />
        ) : (
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
            <Bot className="h-5 w-5" />
          </div>
        )}
        <div className="min-w-0 flex-1">
          <div className="flex items-start justify-between gap-2">
            <div className="min-w-0">
              <div className="truncate text-sm font-semibold">{agent.name}</div>
              <p className="mt-0.5 truncate text-xs text-muted-foreground">
                {primaryReference?.category ??
                  agent.runtime?.runtime_name ??
                  agent.agent_type}
              </p>
            </div>
            <span
              className={cn(
                "inline-flex shrink-0 items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-medium",
                tone === "success" && "bg-emerald-500/10 text-emerald-700",
                tone === "info" && "bg-blue-500/10 text-blue-700",
                tone === "warning" && "bg-amber-500/10 text-amber-700",
                tone === "danger" && "bg-red-500/10 text-red-700",
              )}
            >
              <span
                className={cn(
                  "h-1.5 w-1.5 rounded-full",
                  tone === "success" && "bg-emerald-500",
                  tone === "info" && "bg-blue-500",
                  tone === "warning" && "bg-amber-500",
                  tone === "danger" && "bg-red-500",
                )}
              />
              {label}
            </span>
          </div>
          <p
            className={cn(
              "mt-2 text-xs leading-5 text-muted-foreground",
              !expanded && "line-clamp-2",
            )}
          >
            {renderDisplayValue(summaryText)}
          </p>
        </div>
      </div>

      <div className="mt-3 grid grid-cols-3 gap-2">
        <div className="rounded-md border bg-background/50 p-2">
          <div className="text-[10px] uppercase text-muted-foreground">
            Evidence
          </div>
          <div className="mt-1 text-sm font-semibold">{totalEvidence}</div>
        </div>
        <div className="rounded-md border bg-background/50 p-2">
          <div className="text-[10px] uppercase text-muted-foreground">
            Activity
          </div>
          <div className="mt-1 text-sm font-semibold">{agentEvents.length}</div>
        </div>
        <div className="rounded-md border bg-background/50 p-2">
          <div className="text-[10px] uppercase text-muted-foreground">
            Signals
          </div>
          <div className="mt-1 text-sm font-semibold">
            {observeSignals.length
              ? `${detectedSignals}/${observeSignals.length}`
              : "-"}
          </div>
        </div>
      </div>

      <div className="mt-3 flex flex-wrap gap-1.5">
        <span className="rounded border border-border bg-muted/50 px-1.5 py-0.5 text-[10px] font-medium uppercase">
          {summarizeSource(agent)}
        </span>
        <span className="rounded border border-border bg-muted/50 px-1.5 py-0.5 text-[10px] font-medium uppercase">
          Trust: {agent.trust_level}
        </span>
      </div>

      {expanded && (
        <div className="mt-3 space-y-2 rounded-md border bg-background/50 p-3 text-xs">
          <div className="grid gap-2 sm:grid-cols-2">
            <div>
              <div className="text-muted-foreground">Latest activity</div>
              <div className="mt-0.5 font-medium">
                {latestEvent
                  ? `${formatDisplayValue(latestEvent.result_label)} - ${formatDisplayValue(
                      latestEvent.target_label,
                    )}`
                  : "No timeline event yet"}
              </div>
            </div>
            <div>
              <div className="text-muted-foreground">Runtime</div>
              <div className="mt-0.5 font-medium">
                {agent.runtime?.runtime_name || agent.agent_type}
              </div>
            </div>
          </div>
          {agent.identity?.process_path && (
            <div>
              <div className="text-muted-foreground">Observed process</div>
              <div className="mt-0.5 break-all font-medium">
                {agent.identity.process_path}
              </div>
            </div>
          )}
          {extraCapabilities.length > 0 && (
            <div>
              <div className="text-muted-foreground">Capabilities</div>
              <div className="mt-1 flex flex-wrap gap-1.5">
                {extraCapabilities.map((capability) => (
                  <span
                    key={capability}
                    className="rounded border bg-muted/50 px-1.5 py-0.5"
                  >
                    {capability.replace(/[_.:-]+/g, " ")}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {canExpand && (
        <button
          type="button"
          aria-expanded={expanded}
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
            setExpanded((current) => !current);
          }}
          onKeyDown={(event) => event.stopPropagation()}
          className="mt-3 inline-flex h-7 items-center gap-1 rounded-md border bg-background px-2 text-[11px] font-medium text-muted-foreground hover:bg-muted hover:text-foreground"
        >
          {expanded ? (
            <>
              Show less <ChevronUp className="h-3 w-3" />
            </>
          ) : (
            <>
              Show more <ChevronDown className="h-3 w-3" />
            </>
          )}
        </button>
      )}
    </div>
  );
}

export default function AgentsV2() {
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedId = searchParams.get("id") ?? undefined;
  const { agents, activity, loading } = useAgents();
  const deleteAgent = useDeleteAgent();

  const handleSelect = (id: string) => {
    if (id) setSearchParams({ id });
    else setSearchParams({});
  };

  const knownProfiles = agents.filter(
    (agent) => referencesForAgent(agent).length > 0,
  ).length;
  const agentsWithActivity = agents.filter(
    (agent) => activityForAgent(agent, activity).length > 0,
  ).length;
  const protectedAgents = agents.filter(
    (agent) => agent.enforcement_mode === "Enforce",
  ).length;
  const observedOnlyAgents = agents.filter(
    (agent) => agent.enforcement_mode === "Observe",
  ).length;

  return (
    <div className="space-y-4">
      {!selectedId && (
        <PageHeader
          title="Agents & Models"
          subtitle="AI apps and agents found on this device — what Pollek knows, what it has seen, and what to watch next."
          icon={Bot}
        />
      )}

      {loading ? (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {[1, 2, 3].map((item) => (
            <div
              key={item}
              className="h-32 animate-pulse rounded-xl border bg-muted/30"
            />
          ))}
        </div>
      ) : agents.length === 0 ? (
        <div className="flex flex-col items-center justify-center rounded-xl border border-dashed p-12 text-center">
          <Bot className="mb-3 h-10 w-10 text-muted-foreground/50" />
          <p className="text-sm font-medium">No agents discovered yet</p>
          <p className="mt-1 text-xs text-muted-foreground">
            Run a scan to discover AI agents on this device.
          </p>
        </div>
      ) : (
        <div className="space-y-4">
          {!selectedId && (
            <div className="grid gap-3 md:grid-cols-4">
              <div className="rounded-xl border bg-card/60 p-4">
                <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  <Bot className="h-3.5 w-3.5" />
                  Found
                </div>
                <div className="mt-2 text-2xl font-semibold">
                  {agents.length}
                </div>
                <p className="mt-1 text-xs text-muted-foreground">
                  AI apps and local agents
                </p>
              </div>
              <div className="rounded-xl border bg-card/60 p-4">
                <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  <BookOpen className="h-3.5 w-3.5" />
                  Known
                </div>
                <div className="mt-2 text-2xl font-semibold">
                  {knownProfiles}
                </div>
                <p className="mt-1 text-xs text-muted-foreground">
                  matched with reference definitions
                </p>
              </div>
              <div className="rounded-xl border bg-card/60 p-4">
                <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  <Activity className="h-3.5 w-3.5" />
                  Observed
                </div>
                <div className="mt-2 text-2xl font-semibold">
                  {agentsWithActivity}
                </div>
                <p className="mt-1 text-xs text-muted-foreground">
                  have timeline evidence
                </p>
              </div>
              <div className="rounded-xl border bg-card/60 p-4">
                <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  <ShieldCheck className="h-3.5 w-3.5" />
                  Guarded
                </div>
                <div className="mt-2 text-2xl font-semibold">
                  {protectedAgents}
                </div>
                <p className="mt-1 text-xs text-muted-foreground">
                  {observedOnlyAgents} watch-only setups
                </p>
              </div>
            </div>
          )}

          <MasterDetailLayout
            items={agents}
            selectedId={selectedId}
            onSelect={handleSelect}
            idSelector={(agent) => agent.agent_id}
            masterLayout="grid"
            masterListClassName="grid gap-4 lg:grid-cols-2 2xl:grid-cols-3"
            detailBackLabel="Back to all agents"
            renderCard={(agent, selected) => (
              <AgentMasterCard
                agent={agent}
                activity={activity}
                selected={selected}
              />
            )}
            renderDetail={(agent) => (
              <AgentDetailView
                key={agent.agent_id}
                agent={agent}
                activity={activity}
                onDelete={() => {
                  void deleteAgent(agent.agent_id).then((deleted) => {
                    if (deleted) setSearchParams({});
                  });
                }}
              />
            )}
            emptyState={null}
          />
        </div>
      )}
    </div>
  );
}
