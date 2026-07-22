import { useConfirm } from "../components/ui/ConfirmDialog";
import { toast } from "sonner";
import { useState, useEffect } from "react";
import {
  Search,
  ShieldAlert,
  Info,
  Activity,
  Eye,
  Play,
  CheckCircle,
  RefreshCw,
  Link2,
} from "lucide-react";
import { useSearchParams } from "react-router-dom";
import {
  LocalObserveApi,
  RegistryApi,
  type AiAgent,
  type LocalObserveRefreshResponse,
} from "../services/api";
import type {
  AgentObserveActivity,
  DiscoveredAgentCandidateV2,
  DiscoveryEnrichmentSession,
  DiscoveryCapabilityInventory,
  DiscoveryScanJob,
} from "../services/types";
import { MasterDetailLayout } from "../components/master-detail/MasterDetailLayout";
import { EntityCard } from "../components/master-detail/EntityCard";
import { DetailPane } from "../components/master-detail/DetailPane";
import { StatusChip } from "../components/master-detail/StatusChip";
import { EmptyState } from "../components/master-detail/EmptyState";
import type { UiStatus } from "../lib/status";
import { SimplePolicyWizard } from "../components/simple/SimplePolicyWizard";
import { findAgentReferenceIntel } from "../lib/entityReferenceIntel";
import { ReferenceIntelMark } from "../components/reference/ReferenceIntelMark";
import { ReferenceIntelGuide } from "../components/reference/ReferenceIntelGuide";
import { ContextualHelp } from "../components/help/ContextualHelp";
import { useMode } from "../context/ModeContext";
import { isAdvanceMode } from "../lib/modes";
import { Collapsible } from "../components/ui";
import {
  buildLifecycleContext,
  candidateHasScanId,
  deriveAgentLifecycle,
  latestScanIdForCandidate,
  matchesLifecycleFilter,
  summarizeLifecycles,
  type AgentLifecycle,
  type LifecycleFilter,
} from "../lib/agentLifecycle";
import { AgentLifecycleBadges } from "../components/discovery/AgentLifecycleBadge";
import { LifecycleSummary } from "../components/discovery/LifecycleSummary";

const DEEP_SCAN_SOURCES = [
  "process",
  "mcp_config",
  "local_model",
  "ide_extension",
  "cli_agent",
  "container",
  "browser_extension",
  "installed_app",
  "web_ai",
  "python_framework",
];

const SOURCE_LABELS: Record<string, { label: string; detail: string }> = {
  process: {
    label: "Running apps",
    detail: "Process metadata for local desktop and CLI AI apps.",
  },
  mcp_config: {
    label: "MCP configs",
    detail: "Known MCP server config files and safe metadata.",
  },
  local_model: {
    label: "Local model servers",
    detail: "OpenAI-compatible and common local model endpoints.",
  },
  ide_extension: {
    label: "IDE agents",
    detail: "Installed coding assistants and IDE extension metadata.",
  },
  cli_agent: {
    label: "CLI agents",
    detail: "Command-line AI agent processes and known configs.",
  },
  container: {
    label: "Containers",
    detail: "Container metadata that looks like AI tooling.",
  },
  browser_extension: {
    label: "Browser connectors",
    detail: "Browser extension or connector metadata when available.",
  },
  installed_app: {
    label: "Installed apps",
    detail: "Desktop app install records and known AI app signatures.",
  },
  web_ai: {
    label: "AI websites",
    detail:
      "Browser/window metadata for ChatGPT, Claude, DeepSeek, and similar surfaces.",
  },
  python_framework: {
    label: "Frameworks",
    detail: "Local framework/library metadata such as agent or LLM SDK usage.",
  },
};

function friendlyCapabilityLabel(tag: string) {
  const normalized = tag.toLowerCase();
  if (normalized.includes("llm.chat")) return "AI chat";
  if (normalized.includes("web.chat")) return "Web AI session";
  if (normalized.includes("code.agentic")) return "Coding agent";
  if (normalized.includes("tool.use")) return "Can use tools";
  if (normalized.includes("mcp")) return "MCP tools or resources";
  if (normalized.includes("local.model")) return "Local model server";
  if (normalized.includes("net.egress") || normalized.includes("network")) {
    return "Network access";
  }
  if (normalized.includes("file")) return "File access";
  if (normalized.includes("prompt")) return "Prompt and data safety";
  return tag
    .replace(/[_.:-]+/g, " ")
    .split(" ")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function friendlyCapabilityDetail(tag: string) {
  const normalized = tag.toLowerCase();
  if (normalized.includes("llm.chat")) {
    return "Likely supports AI conversation or model prompts.";
  }
  if (normalized.includes("web.chat")) {
    return "Detected as an AI website or browser-based chat surface.";
  }
  if (normalized.includes("code.agentic")) {
    return "Likely can plan or edit code through an IDE, CLI, or coding agent.";
  }
  if (normalized.includes("tool.use") || normalized.includes("mcp")) {
    return "May call tools, connectors, or MCP resources when integrated.";
  }
  if (normalized.includes("local.model")) {
    return "Looks like a local model endpoint or model runtime.";
  }
  if (normalized.includes("net.egress") || normalized.includes("network")) {
    return "May connect to websites or network destinations.";
  }
  if (normalized.includes("file")) {
    return "May read or write local files depending on app permissions.";
  }
  return "Reference or discovery metadata suggests this capability, but local activity still needs observation.";
}

function friendlySemanticLabel(value?: string) {
  if (!value) return "Unknown";
  return value
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/[_.:-]+/g, " ")
    .split(" ")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function friendlyAuthorityDetail(value?: string) {
  switch (value) {
    case "local_device":
      return "Pollek saw a local app, process, CLI, or installed component on this device.";
    case "local_browser_profile":
      return "Pollek saw a browser tab, window, session, or domain on this browser profile.";
    case "remote_workspace":
      return "The AI work likely runs in a cloud workspace; local visibility comes from browser or connector metadata.";
    case "remote_model_api":
      return "This looks like a provider/API endpoint, not a local controllable app by itself.";
    case "local_network":
      return "Pollek saw a local network endpoint or local model server.";
    case "mcp_remote_server":
      return "Pollek saw an MCP or tool surface that may be controlled through the calling AI app.";
    default:
      return "Pollek needs more evidence or user confirmation before treating this as a controllable AI app.";
  }
}

function friendlyDuplicatePolicy(value?: string) {
  switch (value) {
    case "child_surface":
      return "Controlled through parent";
    case "related_endpoint":
      return "Related surface";
    case "provider_endpoint":
      return "Provider endpoint";
    case "merged_duplicate":
      return "Merged duplicate";
    case "needs_human_confirmation":
      return "Needs review";
    default:
      return "Standalone";
  }
}

function normalizedRegistrationKey(value: unknown) {
  return typeof value === "string" && value.trim().length > 0
    ? value.trim().toLowerCase()
    : undefined;
}

function isRegistrationKey(value: string | undefined): value is string {
  return typeof value === "string" && value.length > 0;
}

function candidateRegistrationKeys(candidate: DiscoveredAgentCandidateV2) {
  const labels = candidate.labels ?? {};
  const keys = [
    candidate.candidate_id,
    candidate.suggested_registration?.agent_id,
    labels.registered_agent_id,
    labels.discovery_candidate_id,
    labels.discovery_candidate_merge_key,
    candidate.evidence?.find((evidence) => evidence.merge_key)?.merge_key,
    [
      candidate.display_name,
      candidate.vendor,
      candidate.suggested_registration?.runtime_name,
    ]
      .filter(Boolean)
      .join("|"),
  ];

  return Array.from(
    new Set(keys.map(normalizedRegistrationKey).filter(isRegistrationKey)),
  );
}

function agentRegistrationKeys(agent: AiAgent) {
  const labels = agent.labels ?? {};
  const keys = [
    agent.agent_id,
    labels.discovery_candidate_id,
    labels.discovery_candidate_merge_key,
    [agent.name, agent.vendor, agent.runtime?.runtime_name]
      .filter(Boolean)
      .join("|"),
  ];

  return Array.from(
    new Set(keys.map(normalizedRegistrationKey).filter(isRegistrationKey)),
  );
}

function lifecycleToUiStatus(lifecycle: AgentLifecycle): UiStatus {
  if (lifecycle.presence === "uninstalled") return "failed";
  if (lifecycle.presence === "dormant") return "idle";
  if (lifecycle.presence === "running" || lifecycle.governance === "registered")
    return "ok";
  if (lifecycle.governance === "pending" || lifecycle.governance === "new")
    return "degraded";
  return "info";
}

export function AutoDiscovery() {
  const { confirm } = useConfirm();
  const { mode } = useMode();
  const showTechnicalDetails = isAdvanceMode(mode);

  const [candidates, setCandidates] = useState<DiscoveredAgentCandidateV2[]>(
    [],
  );
  const [loading, setLoading] = useState(true);
  const [scanJob, setScanJob] = useState<DiscoveryScanJob | null>(null);
  const [params, setParams] = useSearchParams();
  const selectedId = params.get("selected") ?? undefined;
  const [protectTarget, setProtectTarget] = useState<string | null>(null);
  const [confirmingId, setConfirmingId] = useState<string | null>(null);
  const [confirmTarget, setConfirmTarget] =
    useState<DiscoveredAgentCandidateV2 | null>(null);
  const [editName, setEditName] = useState("");
  const [filter, setFilter] = useState<"all" | "pending" | "registered">("all");
  const [lifecycleFilter, setLifecycleFilter] =
    useState<LifecycleFilter>("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [isScanning, setIsScanning] = useState(false);
  const [isFinalizingScan, setIsFinalizingScan] = useState(false);
  const [isObserving, setIsObserving] = useState(false);
  const [observeResult, setObserveResult] =
    useState<LocalObserveRefreshResponse | null>(null);
  const [scans, setScans] = useState<DiscoveryScanJob[]>([]);
  const [scanFilter, setScanFilter] = useState<string>("latest");
  const [capabilityInventories, setCapabilityInventories] = useState<
    Record<string, DiscoveryCapabilityInventory>
  >({});
  const [capabilityLoadingId, setCapabilityLoadingId] = useState<string | null>(
    null,
  );
  const [enrichmentSessions, setEnrichmentSessions] = useState<
    Record<string, DiscoveryEnrichmentSession>
  >({});
  const [enrichmentBusyId, setEnrichmentBusyId] = useState<string | null>(null);
  const [activityViews, setActivityViews] = useState<
    Record<string, AgentObserveActivity>
  >({});
  const [activityLoadingId, setActivityLoadingId] = useState<string | null>(
    null,
  );

  const scanBusy =
    isScanning ||
    isFinalizingScan ||
    scanJob?.status === "queued" ||
    scanJob?.status === "running";

  const scanButtonLabel = isFinalizingScan
    ? "Updating Results"
    : scanBusy
      ? "Scanning"
      : "Deep Scan";

  const clearHistory = async () => {
    if (
      !(await confirm({
        title: "Confirm",
        description: "Are you sure you want to drop all discovery history?",
        danger: true,
      }))
    )
      return;
    try {
      await RegistryApi.clearDiscoveryCandidates();
      setCapabilityInventories({});
      void fetchCandidates();
      toast.success("Discovery history cleared");
    } catch (e) {
      console.error(e);
      toast.error("Failed to clear history");
    }
  };

  const fetchCandidates = async (
    options: { showLoading?: boolean } = {},
  ): Promise<DiscoveredAgentCandidateV2[]> => {
    const showLoading = options.showLoading ?? true;
    if (showLoading) setLoading(true);
    try {
      const [discovered, agents] = await Promise.all([
        RegistryApi.listDiscoveryCandidates(),
        RegistryApi.listAgents(),
      ]);
      const agentIds = new Set(agents.map((a) => a.agent_id));
      const registeredKeyToAgentId = new Map<string, string>();
      for (const agent of agents) {
        for (const key of agentRegistrationKeys(agent)) {
          registeredKeyToAgentId.set(key, agent.agent_id);
        }
      }

      const mergedCandidates = discovered.map((c) => {
        const directAgentId =
          (agentIds.has(c.candidate_id) && c.candidate_id) ||
          (c.labels?.registered_agent_id &&
          agentIds.has(c.labels.registered_agent_id)
            ? c.labels.registered_agent_id
            : undefined);
        const mappedAgentId =
          directAgentId ||
          candidateRegistrationKeys(c)
            .map((key) => registeredKeyToAgentId.get(key))
            .find(Boolean);

        if (mappedAgentId) {
          return {
            ...c,
            status: "registered",
            labels: {
              ...(c.labels ?? {}),
              registered_agent_id: mappedAgentId,
            },
            _agent_id: mappedAgentId,
          };
        }

        if (c.status === "registered") {
          const labels = { ...(c.labels ?? {}) };
          delete labels.registered_agent_id;
          return { ...c, status: "pending_approval", labels };
        }
        return c;
      });

      mergedCandidates.sort(
        (a, b) =>
          new Date(b.first_seen).getTime() - new Date(a.first_seen).getTime(),
      );

      setCandidates(mergedCandidates);
      return mergedCandidates;
    } catch {
      return [];
    } finally {
      if (showLoading) setLoading(false);
    }
  };

  const fetchScans = async (): Promise<DiscoveryScanJob[]> => {
    try {
      const nextScans = await RegistryApi.listDiscoveryScans();
      nextScans.sort(
        (a, b) =>
          new Date(b.started_at || b.finished_at || 0).getTime() -
          new Date(a.started_at || a.finished_at || 0).getTime(),
      );
      setScans(nextScans);

      const active = nextScans.find(
        (scan) => scan.status === "queued" || scan.status === "running",
      );
      if (active) {
        setScanJob(active);
      }
      return nextScans;
    } catch (e) {
      console.error(e);
      return [];
    }
  };

  useEffect(() => {
    void fetchCandidates();
    void fetchScans();
  }, []);

  useEffect(() => {
    if (!selectedId || capabilityInventories[selectedId]) return;
    void loadCapabilities(selectedId, false);
  }, [selectedId]);

  useEffect(() => {
    if (!selectedId || activityViews[selectedId]) return;
    const candidate = candidates.find((c) => c.candidate_id === selectedId);
    if (candidate) void loadAgentActivity(candidate, false);
  }, [selectedId, candidates]);

  const observeIdsForCandidate = (candidate: DiscoveredAgentCandidateV2) => {
    const primary =
      candidate.labels?.registered_agent_id ||
      candidate.suggested_registration?.agent_id ||
      candidate.candidate_id;
    const altIds = [
      candidate.candidate_id,
      candidate.suggested_registration?.agent_id ?? "",
    ].filter((id) => id && id !== primary);
    return { primary, altIds: Array.from(new Set(altIds)) };
  };

  const loadAgentActivity = async (
    candidate: DiscoveredAgentCandidateV2,
    manual: boolean,
  ) => {
    setActivityLoadingId(candidate.candidate_id);
    try {
      const { primary, altIds } = observeIdsForCandidate(candidate);
      const view = await RegistryApi.getAgentObserveActivity(primary, {
        altIds,
        limit: 500,
      });
      setActivityViews((prev) => ({
        ...prev,
        [candidate.candidate_id]: view,
      }));
      if (manual) toast.success("Agent activity refreshed");
      return view;
    } catch (e) {
      console.error("Failed to load agent activity:", e);
      if (manual) toast.error("Failed to load agent activity");
      return null;
    } finally {
      setActivityLoadingId(null);
    }
  };

  const loadCapabilities = async (candidateId: string, persist: boolean) => {
    setCapabilityLoadingId(candidateId);
    try {
      const inventory = persist
        ? await RegistryApi.retrieveDiscoveryCandidateCapabilities(candidateId)
        : await RegistryApi.getDiscoveryCandidateCapabilities(candidateId);
      setCapabilityInventories((prev) => ({
        ...prev,
        [candidateId]: inventory,
      }));
      if (persist) {
        toast.success("Capability inventory refreshed");
      }
      return inventory;
    } catch (e) {
      console.error("Failed to load capability inventory:", e);
      if (persist) {
        toast.error("Failed to refresh capability inventory");
      }
      return null;
    } finally {
      setCapabilityLoadingId(null);
    }
  };

  const startEnrichment = async (candidateId: string) => {
    setEnrichmentBusyId(candidateId);
    try {
      const session = await RegistryApi.startDiscoveryCandidateEnrichment(
        candidateId,
        {
          sources: [
            "official_site",
            "package_registry",
            "github_metadata",
            "mcp_manifest",
          ],
        },
      );
      setEnrichmentSessions((prev) => ({
        ...prev,
        [candidateId]: session,
      }));
      toast.success("Definition enrichment is ready for review");
    } catch (error) {
      console.error("Failed to start enrichment:", error);
      toast.error("Failed to prepare enrichment");
    } finally {
      setEnrichmentBusyId(null);
    }
  };

  const approveEnrichment = async (candidateId: string) => {
    const session = enrichmentSessions[candidateId];
    if (!session) return;
    setEnrichmentBusyId(candidateId);
    try {
      const approved = await RegistryApi.approveDiscoveryCandidateEnrichment(
        session.session_id,
        session.source_plan.map((source) => source.source_id),
      );
      setEnrichmentSessions((prev) => ({
        ...prev,
        [candidateId]: approved,
      }));
      toast.success("Safe source plan approved");
    } catch (error) {
      console.error("Failed to approve enrichment:", error);
      toast.error("Failed to approve enrichment");
    } finally {
      setEnrichmentBusyId(null);
    }
  };

  const submitEnrichment = async (candidateId: string) => {
    const session = enrichmentSessions[candidateId];
    if (!session) return;
    setEnrichmentBusyId(candidateId);
    try {
      const submitted = await RegistryApi.submitDiscoveryCandidateEnrichment(
        session.session_id,
      );
      setEnrichmentSessions((prev) => ({
        ...prev,
        [candidateId]: submitted,
      }));
      await fetchCandidates({ showLoading: false });
      toast.success("Learned profile saved locally");
    } catch (error) {
      console.error("Failed to save learned profile:", error);
      toast.error("Failed to save learned profile");
    } finally {
      setEnrichmentBusyId(null);
    }
  };

  const settleScanResults = async (
    expectedCount = 0,
    expectedScanId?: string,
  ) => {
    setIsFinalizingScan(true);
    let previousDigest = "";
    let stableReads = 0;

    try {
      for (let attempt = 0; attempt < 10; attempt += 1) {
        const next = await fetchCandidates({ showLoading: attempt === 0 });
        const scoped = expectedScanId
          ? next.filter((candidate) =>
              candidateHasScanId(candidate, expectedScanId),
            )
          : next;
        const digest = scoped
          .map(
            (c) =>
              `${c.candidate_id}:${c.status}:${c.last_seen}:${c.evidence?.length ?? 0}`,
          )
          .sort()
          .join("|");

        const hasExpectedCount =
          expectedCount === 0 || scoped.length >= expectedCount;
        if (attempt > 0 && digest === previousDigest && hasExpectedCount) {
          stableReads += 1;
        } else {
          stableReads = 0;
        }
        previousDigest = digest;

        if (stableReads >= 2) break;
        await new Promise((resolve) => setTimeout(resolve, 900));
      }
    } finally {
      setIsFinalizingScan(false);
    }
  };

  useEffect(() => {
    let interval: ReturnType<typeof setInterval> | undefined;
    let cancelled = false;
    if (
      scanJob &&
      (scanJob.status === "queued" || scanJob.status === "running")
    ) {
      interval = setInterval(async () => {
        try {
          const status = await RegistryApi.getDiscoveryScanStatus(
            scanJob.scan_id,
          );
          setScanJob(status);
          if (
            status.status === "completed" ||
            status.status === "partial" ||
            status.status === "failed"
          ) {
            if (interval) clearInterval(interval);
            if (!cancelled) {
              await settleScanResults(status.candidates_found, status.scan_id);
              await fetchScans();
            }
          }
        } catch (e) {
          console.error(e);
        }
      }, 2000);
    }
    return () => {
      cancelled = true;
      if (interval) clearInterval(interval);
    };
  }, [scanJob]);

  const select = (id: string) =>
    setParams((p) => {
      if (id) {
        p.set("selected", id);
      } else {
        p.delete("selected");
      }
      return p;
    });

  const deleteCandidate = async (c: any) => {
    if (
      !(await confirm({
        title:
          c.status === "registered"
            ? "Delete Registered Agent"
            : "Confirm Action",
        description:
          c.status === "registered"
            ? "Are you sure you want to delete this AI Agent? This will unregister the agent and delete its discovery candidate."
            : "Are you sure you want to delete this candidate?",
        danger: true,
      }))
    )
      return;
    try {
      if (c.status === "registered" && c._agent_id) {
        try {
          await RegistryApi.deleteAgent(c._agent_id);
        } catch (e) {
          console.error("Failed to delete agent", e);
        }
      }
      await RegistryApi.deleteDiscoveryCandidate(c.candidate_id);
      setCapabilityInventories((prev) => {
        const next = { ...prev };
        delete next[c.candidate_id];
        return next;
      });
      if (selectedId === c.candidate_id) {
        setParams((p) => {
          p.delete("selected");
          return p;
        });
      }
      toast.success("Deleted successfully");
      void fetchCandidates();
    } catch (e) {
      console.error("Failed to delete candidate:", e);
      toast.error("Failed to delete candidate");
    }
  };

  const capabilityTags = (candidate: DiscoveredAgentCandidateV2) => {
    const fromLabels = Object.keys(candidate.labels ?? {})
      .filter((key) => key.startsWith("capability:"))
      .map((key) => key.slice("capability:".length));
    return Array.from(
      new Set([...(candidate.capability_tags ?? []), ...fromLabels]),
    ).sort();
  };

  const displayNameForCandidate = (candidate: DiscoveredAgentCandidateV2) => {
    const browserName = browserNameForCandidate(candidate);

    return (candidate.display_name || "AI Agent").replace(
      /\s+\((Web|Browser)\)$/i,
      ` (${browserName || "Browser"})`,
    );
  };

  const browserNameForCandidate = (candidate: DiscoveredAgentCandidateV2) =>
    candidate.evidence
      ?.map((e) => e.data?.browser_name)
      .find(
        (name) =>
          typeof name === "string" && name.length > 0 && name !== "Browser",
      );

  const evidenceSourcesForCandidate = (candidate: DiscoveredAgentCandidateV2) =>
    Array.from(new Set(candidate.evidence?.map((e) => e.source) ?? []))
      .map((source) => source.replace(/_/g, " "))
      .join(", ");

  const referenceForCandidate = (candidate: DiscoveredAgentCandidateV2) =>
    findAgentReferenceIntel({
      name: displayNameForCandidate(candidate),
      vendor: candidate.vendor,
      agentType: candidate.inferred_agent_type,
    })[0];

  const scanIdForCandidate = (candidate: DiscoveredAgentCandidateV2) =>
    latestScanIdForCandidate(candidate);

  const scanForCandidate = (candidate: DiscoveredAgentCandidateV2) => {
    const scanId = scanIdForCandidate(candidate);
    return scans.find((scan) => scan.scan_id === scanId);
  };

  const scanLabelForCandidate = (candidate: DiscoveredAgentCandidateV2) => {
    const scan = scanForCandidate(candidate);
    if (!scan) {
      return "Previous scan";
    }

    const startedAt = scan.started_at || scan.finished_at;
    const timeLabel = startedAt
      ? new Date(startedAt).toLocaleString(undefined, {
          month: "short",
          day: "numeric",
          hour: "2-digit",
          minute: "2-digit",
        })
      : scan.scan_id.slice(0, 12);
    return `${timeLabel} - ${scan.status}`;
  };

  const scanSortTime = (candidate: DiscoveredAgentCandidateV2) => {
    const scan = scanForCandidate(candidate);
    return new Date(
      scan?.started_at || scan?.finished_at || candidate.last_seen,
    ).getTime();
  };

  const latestScanId = scans[0]?.scan_id || scanJob?.scan_id;
  const selectedScanId =
    scanFilter === "latest"
      ? latestScanId
      : scanFilter === "all"
        ? undefined
        : scanFilter;
  const selectedScan =
    selectedScanId != null
      ? scans.find((scan) => scan.scan_id === selectedScanId) ||
        (scanJob?.scan_id === selectedScanId ? scanJob : undefined)
      : (scans[0] ?? scanJob ?? undefined);
  const scanScopedCandidates = candidates.filter((candidate) => {
    if (scanFilter === "all") return true;
    if (!selectedScanId) return false;
    return candidateHasScanId(candidate, selectedScanId);
  });

  // Human-facing lifecycle: is each agent running now, merely installed, gone,
  // and where does it sit in the governance workflow. Derived from the scan
  // list + candidate recency/evidence so every card reads consistently.
  const lifecycleCtx = buildLifecycleContext(scans, scanJob);
  const lifecycleForCandidate = (
    c: DiscoveredAgentCandidateV2,
  ): AgentLifecycle => deriveAgentLifecycle(c, lifecycleCtx);
  const lifecycleCounts = summarizeLifecycles(
    scanScopedCandidates,
    lifecycleCtx,
  );

  const visibleCandidates = scanScopedCandidates
    .filter((c) => {
      if (filter === "registered") return c.status === "registered";
      if (filter === "pending") return c.status !== "registered";
      return true;
    })
    .filter((c) =>
      matchesLifecycleFilter(lifecycleForCandidate(c), lifecycleFilter),
    )
    .filter((c) => {
      const query = searchQuery.trim().toLowerCase();
      if (!query) return true;
      return [
        displayNameForCandidate(c),
        c.vendor,
        c.inferred_agent_type,
        c.canonical_service_id,
        c.surface_group_id,
        c.authority_boundary,
        c.entity_role,
        c.duplicate_policy,
        c.observe_scope,
        c.enforce_scope,
        c.grouping_reason,
        browserNameForCandidate(c),
        evidenceSourcesForCandidate(c),
        ...(capabilityTags(c) ?? []),
        ...Object.keys(c.labels ?? {}),
        ...Object.values(c.labels ?? {}),
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase()
        .includes(query);
    })
    .sort((a, b) => {
      const scanDelta = scanSortTime(b) - scanSortTime(a);
      if (scanDelta !== 0) return scanDelta;
      return new Date(b.last_seen).getTime() - new Date(a.last_seen).getTime();
    });

  const registeredCount = scanScopedCandidates.filter(
    (candidate) => candidate.status === "registered",
  ).length;
  const pendingCount = scanScopedCandidates.length - registeredCount;
  const knownCount = scanScopedCandidates.filter(
    (candidate) => referenceForCandidate(candidate) != null,
  ).length;
  const evidenceCount = scanScopedCandidates.reduce(
    (total, candidate) => total + (candidate.evidence?.length ?? 0),
    0,
  );
  const coverageScan = selectedScan ?? scans[0] ?? scanJob ?? null;
  const latestScanSources = new Set(coverageScan?.sources ?? []);
  const checkedSourceCount = DEEP_SCAN_SOURCES.filter((source) =>
    latestScanSources.has(source),
  ).length;

  const openConfirmDialog = (candidate: DiscoveredAgentCandidateV2) => {
    setConfirmTarget(candidate);
    setEditName(displayNameForCandidate(candidate));
  };

  const submitConfirmAgent = async () => {
    if (!confirmTarget) return;
    setConfirmingId(confirmTarget.candidate_id);
    try {
      const response = await RegistryApi.registerDiscoveryCandidate(
        confirmTarget.candidate_id,
        {
          agent_name: editName,
        },
      );
      toast.success(`Confirmed ${response.agent_name ?? editName}`);
      setConfirmTarget(null);
      void fetchCandidates();
    } catch (e) {
      console.error("Failed to confirm agent:", e);
      toast.error("Failed to confirm agent");
    } finally {
      setConfirmingId(null);
    }
  };

  const triggerScan = async () => {
    if (scanBusy) return;
    setIsScanning(true);
    try {
      const result = await RegistryApi.triggerDiscoveryScan({
        sources: DEEP_SCAN_SOURCES,
        scan_mode: "deep",
        source_timeout_secs: 12,
        total_deadline_secs: 75,
        privacy_mode: true,
      });
      setScanJob({
        scan_id: result.scan_id,
        tenant_id: "local",
        status: result.status as any,
        sources: DEEP_SCAN_SOURCES,
        candidates_found: 0,
      });
      if (
        result.status === "completed" ||
        result.status === "partial" ||
        result.status === "failed"
      ) {
        await settleScanResults(
          (result as any).candidates_found ?? 0,
          result.scan_id,
        );
        await fetchScans();
      } else {
        void fetchScans();
      }
    } catch (e) {
      console.error(e);
    } finally {
      setTimeout(() => setIsScanning(false), 500);
    }
  };

  const observeNow = async () => {
    setIsObserving(true);
    try {
      const result = await LocalObserveApi.refresh({ include_estimates: true });
      setObserveResult(result);
      await Promise.all([
        fetchCandidates({ showLoading: false }),
        fetchScans(),
      ]);
      toast.success(
        `Observed ${result.candidates_found} AI app(s), ${result.resource_events} resource event(s), and ${result.tool_events} tool event(s).`,
      );
    } catch (e) {
      console.error(e);
      toast.error(
        e instanceof Error ? e.message : "Local observe refresh failed",
      );
    } finally {
      setIsObserving(false);
    }
  };

  const observedTermsForCandidate = (
    candidate: DiscoveredAgentCandidateV2,
  ): Array<string | undefined | null> => [
    displayNameForCandidate(candidate),
    candidate.vendor,
    candidate.inferred_agent_type,
    candidate.canonical_service_id,
    candidate.surface_group_id,
    candidate.authority_boundary,
    candidate.entity_role,
    candidate.duplicate_policy,
    candidate.observe_scope,
    candidate.enforce_scope,
    candidate.grouping_reason,
    browserNameForCandidate(candidate),
    evidenceSourcesForCandidate(candidate),
    ...(candidate.capability_tags ?? []),
    ...Object.keys(candidate.labels ?? {}),
    ...Object.values(candidate.labels ?? {}),
    ...(candidate.evidence ?? []).flatMap((evidence) => [
      evidence.source,
      (evidence as any).source_path_redacted,
      (evidence as any).merge_key,
      JSON.stringify(evidence.data ?? {}),
    ]),
    ...(candidate.discovered_mcp_servers ?? []).flatMap((server: any) => [
      server.server_name,
      server.transport,
      JSON.stringify(server),
    ]),
  ];

  return (
    <div className="space-y-4">
      {!selectedId && (
        <>
          <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
            <div>
              <h2 className="text-lg font-semibold tracking-tight">
                <span className="inline-flex items-center gap-2">
                  Auto Discovery
                  <ContextualHelp topicId="discovery.auto_scan" />
                </span>
              </h2>
              <p className="text-sm text-muted-foreground">
                Find and manage local AI agents, MCP servers, and model
                endpoints.
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <button
                onClick={clearHistory}
                className="rounded-lg bg-red-500/10 border border-red-500/20 px-4 py-2 text-sm font-medium text-red-400 hover:bg-red-500/20 shadow-sm transition-colors"
              >
                Clear History
              </button>
              <button
                onClick={observeNow}
                disabled={isObserving}
                className="flex items-center gap-2 rounded-lg border bg-background px-4 py-2 text-sm font-medium hover:bg-muted shadow-sm disabled:opacity-50"
              >
                <Eye
                  className={`h-4 w-4 ${isObserving ? "animate-pulse" : ""}`}
                />
                {isObserving ? "Observing" : "Observe Now"}
              </button>
              <button
                onClick={triggerScan}
                disabled={scanBusy}
                className="flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 shadow-sm disabled:opacity-50"
              >
                {scanBusy ? (
                  <Activity className="h-4 w-4 animate-spin" />
                ) : (
                  <Search className="h-4 w-4" />
                )}
                {scanButtonLabel}
              </button>
            </div>
          </div>

          {observeResult && (
            <section className="rounded-lg border bg-card/60 p-4 text-sm">
              <div className="flex flex-col gap-2 lg:flex-row lg:items-center lg:justify-between">
                <div>
                  <h3 className="font-semibold">Latest observe refresh</h3>
                  <p className="text-xs leading-5 text-muted-foreground">
                    {observeResult.candidates_found} AI app(s),{" "}
                    {observeResult.resource_events} resource event(s),{" "}
                    {observeResult.tool_events} tool event(s),{" "}
                    {observeResult.exact_usage_events} exact usage event(s), and{" "}
                    {observeResult.estimated_usage_events} estimated usage
                    event(s) were written into the activity timeline.
                  </p>
                </div>
                <span className="rounded-full border bg-background px-2.5 py-1 text-xs text-muted-foreground">
                  {observeResult.capture_quality.join(", ") ||
                    "metadata observed"}
                </span>
              </div>
            </section>
          )}

          <Collapsible
            className="rounded-xl bg-card/60"
            title={
              <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                <div>
                  <div className="text-sm font-semibold">Discovery summary</div>
                  <div className="text-xs text-muted-foreground">
                    Found, registered, known profiles, and evidence signals for
                    the selected scan view.
                  </div>
                </div>
                <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
                  <span className="rounded-full border bg-background px-2.5 py-1">
                    {scanScopedCandidates.length} found
                  </span>
                  <span className="rounded-full border bg-background px-2.5 py-1">
                    {registeredCount} registered
                  </span>
                  <span className="rounded-full border bg-background px-2.5 py-1">
                    {evidenceCount} evidence
                  </span>
                </div>
              </div>
            }
          >
            <div className="grid gap-3 md:grid-cols-4">
              <div className="rounded-xl border bg-card/60 p-4">
                <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  <Search className="h-3.5 w-3.5" />
                  Found
                </div>
                <div className="mt-2 text-2xl font-semibold">
                  {scanScopedCandidates.length}
                </div>
                <p className="mt-1 text-xs text-muted-foreground">
                  discovery candidates
                </p>
              </div>
              <div className="rounded-xl border bg-card/60 p-4">
                <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  <CheckCircle className="h-3.5 w-3.5" />
                  Registered
                </div>
                <div className="mt-2 text-2xl font-semibold">
                  {registeredCount}
                </div>
                <p className="mt-1 text-xs text-muted-foreground">
                  {pendingCount} still need review
                </p>
              </div>
              <div className="rounded-xl border bg-card/60 p-4">
                <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  <Info className="h-3.5 w-3.5" />
                  Known
                </div>
                <div className="mt-2 text-2xl font-semibold">{knownCount}</div>
                <p className="mt-1 text-xs text-muted-foreground">
                  matched reference profiles
                </p>
              </div>
              <div className="rounded-xl border bg-card/60 p-4">
                <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  <Eye className="h-3.5 w-3.5" />
                  Evidence
                </div>
                <div className="mt-2 text-2xl font-semibold">
                  {evidenceCount}
                </div>
                <p className="mt-1 text-xs text-muted-foreground">
                  local metadata signals
                </p>
              </div>
            </div>
          </Collapsible>

          <Collapsible
            className="rounded-xl bg-card/60"
            title={
              <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                <div>
                  <div className="text-sm font-semibold">
                    Scan source coverage
                  </div>
                  <div className="text-xs text-muted-foreground">
                    What was checked, what needs setup, and what remains
                    metadata-only.
                  </div>
                </div>
                <span className="w-fit rounded-full border bg-background px-3 py-1 text-xs text-muted-foreground">
                  {checkedSourceCount}/{DEEP_SCAN_SOURCES.length} sources
                  checked
                </span>
              </div>
            }
          >
            <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
              <div>
                <p className="mt-1 max-w-3xl text-sm leading-6 text-muted-foreground">
                  Discovery means Pollek found AI apps or surfaces from local
                  metadata. Observe means Pollek later saw real activity. This
                  scan coverage shows what was checked, what still needs setup,
                  and what remains metadata-only.
                </p>
              </div>
            </div>
            <div className="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-5">
              {DEEP_SCAN_SOURCES.map((source) => {
                const copy = SOURCE_LABELS[source] ?? {
                  label: source.replace(/_/g, " "),
                  detail: "Local metadata source.",
                };
                const checked = latestScanSources.has(source);
                const running =
                  coverageScan?.status === "queued" ||
                  coverageScan?.status === "running";
                const label = checked
                  ? running
                    ? "Checking"
                    : "Checked"
                  : source === "browser_extension"
                    ? "Needs connector"
                    : "Not checked";
                return (
                  <div
                    key={source}
                    className="rounded-lg border bg-background/60 p-3"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <div className="min-w-0 truncate text-sm font-medium">
                        {copy.label}
                      </div>
                      <span
                        className={`shrink-0 rounded-full px-2 py-0.5 text-[11px] ${
                          checked
                            ? "bg-emerald-500/10 text-emerald-700"
                            : "bg-amber-500/10 text-amber-700"
                        }`}
                      >
                        {label}
                      </span>
                    </div>
                    <p className="mt-2 text-xs leading-5 text-muted-foreground">
                      {copy.detail}
                    </p>
                  </div>
                );
              })}
            </div>
            <p className="mt-3 text-xs leading-5 text-muted-foreground">
              Privacy guardrail: discovery uses metadata such as process names,
              browser titles, domains, configs, and redacted paths. It does not
              read prompts, responses, email bodies, or file contents for this
              view.
            </p>
          </Collapsible>
        </>
      )}

      <MasterDetailLayout
        items={visibleCandidates}
        loading={loading}
        selectedId={selectedId}
        onSelect={select}
        idSelector={(c) => c.candidate_id}
        masterLayout="grid"
        masterListClassName="grid gap-4 xl:grid-cols-2 2xl:grid-cols-3"
        detailBackLabel="Back to all discovered AI apps"
        toolbar={
          <div className="flex flex-col gap-3 mb-4">
            <LifecycleSummary
              counts={lifecycleCounts}
              active={lifecycleFilter}
              onSelect={setLifecycleFilter}
            />
            <div className="flex flex-col md:flex-row md:items-center gap-2 justify-between">
              <div className="flex items-center gap-2">
                {(["all", "pending", "registered"] as const).map((f) => (
                  <button
                    key={f}
                    onClick={() => setFilter(f)}
                    className={`px-3 py-1 text-xs font-medium rounded-full transition-colors ${
                      filter === f
                        ? "bg-primary text-primary-foreground"
                        : "bg-muted text-muted-foreground hover:bg-muted/80"
                    }`}
                  >
                    {f === "all"
                      ? "All statuses"
                      : f.charAt(0).toUpperCase() + f.slice(1)}
                  </button>
                ))}
              </div>
              <div className="flex items-center gap-2">
                <span className="text-xs text-muted-foreground">Scan:</span>
                <select
                  aria-label="Filter discovery candidates by scan"
                  value={scanFilter}
                  onChange={(e) => setScanFilter(e.target.value)}
                  className="rounded-md border bg-background px-3 py-1 text-xs outline-none focus:ring-1 focus:ring-primary"
                >
                  <option value="latest">Latest Scan</option>
                  <option value="all">All Scans</option>
                  {scans.map((scan) => (
                    <option key={scan.scan_id} value={scan.scan_id}>
                      {new Date(
                        scan.started_at || scan.finished_at || 0,
                      ).toLocaleString()}{" "}
                      - {scan.status}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={searchQuery}
                onChange={(event) => setSearchQuery(event.target.value)}
                placeholder="Search candidates..."
                className="px-3 py-1.5 text-sm rounded-md border bg-background w-full"
              />
            </div>
          </div>
        }
        emptyState={
          <EmptyState
            icon={Search}
            title="No candidates discovered"
            description="Run a Deep Scan to automatically find AI components running locally."
            actionLabel={scanButtonLabel}
            onAction={triggerScan}
            actionBusy={scanBusy}
          />
        }
        renderGroupHeader={(c, _, prev) => {
          const scanId = scanIdForCandidate(c) || "legacy";
          const prevScanId = prev ? scanIdForCandidate(prev) || "legacy" : null;
          if (scanId === prevScanId) return null;
          const isLatest = scanId === scanIdForCandidate(visibleCandidates[0]);
          return (
            <div className="px-2 py-1 mt-4 first:mt-0 mb-1 text-xs font-semibold text-muted-foreground uppercase tracking-wider">
              {isLatest ? "Latest Scan" : "Scan"} - {scanLabelForCandidate(c)}
            </div>
          );
        }}
        renderCard={(c, selected) => {
          const lifecycle = lifecycleForCandidate(c);
          const status = lifecycleToUiStatus(lifecycle);
          const caps = capabilityTags(c);
          const isRegistered = c.status === "registered";
          const browserName = browserNameForCandidate(c);
          const primaryReference = referenceForCandidate(c);
          const candidateStatusLabel = lifecycle.isLive
            ? "Live now"
            : `Seen ${lifecycle.lastSeenLabel}`;

          return (
            <EntityCard
              title={displayNameForCandidate(c)}
              subtitle={c.inferred_agent_type}
              summary={
                primaryReference?.observeGuide?.summary ||
                `Detected from ${
                  evidenceSourcesForCandidate(c) || "local metadata"
                }.`
              }
              icon={ShieldAlert}
              visual={
                primaryReference ? (
                  <ReferenceIntelMark reference={primaryReference} />
                ) : undefined
              }
              status={status}
              statusLabel={candidateStatusLabel}
              headerBadges={<AgentLifecycleBadges lifecycle={lifecycle} />}
              meta={[
                {
                  label: "Control",
                  value: friendlyDuplicatePolicy(c.duplicate_policy),
                },
                {
                  label: "Boundary",
                  value: friendlySemanticLabel(c.authority_boundary),
                },
                {
                  label: "Evidence",
                  value: `${c.evidence?.length ?? 0} signal(s)`,
                },
                {
                  label: "Confidence",
                  value: `${(c.confidence * 100).toFixed(0)}%`,
                },
                {
                  label: "Discovered",
                  value: new Date(c.first_seen).toLocaleString(undefined, {
                    month: "short",
                    day: "numeric",
                    hour: "2-digit",
                    minute: "2-digit",
                  }),
                },
                {
                  label: "Scan",
                  value: scanLabelForCandidate(c),
                },
                ...(c.control_parent_id
                  ? [
                      {
                        label: "Parent",
                        value: c.control_parent_id,
                      },
                    ]
                  : []),
                {
                  label: "Capabilities",
                  value:
                    caps.length > 0
                      ? caps.slice(0, 3).map(friendlyCapabilityLabel).join(", ")
                      : "Unknown",
                },
                ...(showTechnicalDetails && caps.length > 0
                  ? [{ label: "Technical tags", value: caps.join(", ") }]
                  : []),
                ...(primaryReference
                  ? [{ label: "Known", value: primaryReference.title }]
                  : []),
                ...(browserName
                  ? [
                      {
                        label: "Browser",
                        value: browserName,
                      },
                    ]
                  : []),
              ]}
              actions={
                isRegistered
                  ? []
                  : [
                      {
                        label:
                          confirmingId === c.candidate_id
                            ? "Registering..."
                            : "Register Agent",
                        icon: CheckCircle,
                        primary: true,
                        disabled: confirmingId === c.candidate_id,
                        onClick: () => openConfirmDialog(c),
                      },
                    ]
              }
              selected={selected}
            />
          );
        }}
        renderDetail={(c) => {
          const lifecycle = lifecycleForCandidate(c);
          const status = lifecycleToUiStatus(lifecycle);
          const caps = capabilityTags(c);
          const isRegistered = c.status === "registered";
          const browserName = browserNameForCandidate(c);
          const sourceSummary = evidenceSourcesForCandidate(c);
          const primaryReference = referenceForCandidate(c);
          const inventory = capabilityInventories[c.candidate_id];
          const entity = inventory?.entity;
          const canonicalCapabilities = inventory?.capabilities ?? [];
          const relationships = inventory?.relationships ?? [];
          const capabilityLoading = capabilityLoadingId === c.candidate_id;
          const activityView = activityViews[c.candidate_id];
          const activityLoading = activityLoadingId === c.candidate_id;
          const enrichmentSession = enrichmentSessions[c.candidate_id];
          const enrichmentBusy = enrichmentBusyId === c.candidate_id;
          const candidateStatusLabel = lifecycle.isLive
            ? "Live now"
            : `Seen ${lifecycle.lastSeenLabel}`;

          const actions = [
            ...(isRegistered
              ? []
              : [
                  {
                    label:
                      confirmingId === c.candidate_id
                        ? "Registering..."
                        : "Register Agent",
                    primary: true,
                    icon: CheckCircle,
                    disabled: confirmingId === c.candidate_id,
                    onClick: () => openConfirmDialog(c),
                  },
                ]),
            {
              label: "Protect",
              primary: isRegistered,
              onClick: () => setProtectTarget(c.candidate_id),
            },
            {
              label: "Delete",
              danger: true,
              onClick: () => deleteCandidate(c),
            },
          ];
          const tabs = [
            {
              id: "overview",
              label: "Overview",
              content: (
                <div className="space-y-6">
                  <div className="p-4 bg-muted/30 rounded-xl border">
                    <h4 className="text-sm font-semibold mb-3">
                      Friendly Details
                    </h4>
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-3 text-sm">
                      <div>
                        <span className="text-muted-foreground block">
                          Provider
                        </span>
                        <span className="font-medium">
                          {c.vendor || "Unknown"}
                        </span>
                      </div>
                      <div>
                        <span className="text-muted-foreground block">
                          Runtime
                        </span>
                        <span className="font-medium">
                          {browserName || c.inferred_agent_type}
                        </span>
                      </div>
                      <div>
                        <span className="text-muted-foreground block">
                          Detected Via
                        </span>
                        <span className="font-medium">
                          {sourceSummary || "Discovery evidence"}
                        </span>
                      </div>
                      <div>
                        <span className="text-muted-foreground block">
                          Scan
                        </span>
                        <span className="font-medium">
                          {scanLabelForCandidate(c)}
                        </span>
                      </div>
                      <div>
                        <span className="text-muted-foreground block">
                          Control relationship
                        </span>
                        <span className="font-medium">
                          {friendlyDuplicatePolicy(c.duplicate_policy)}
                        </span>
                      </div>
                      <div>
                        <span className="text-muted-foreground block">
                          Boundary
                        </span>
                        <span className="font-medium">
                          {friendlySemanticLabel(c.authority_boundary)}
                        </span>
                      </div>
                    </div>
                  </div>

                  <div className="grid gap-4 md:grid-cols-2">
                    <div className="rounded-xl border bg-muted/30 p-4">
                      <h4 className="text-sm font-semibold">
                        What Pollek can see here
                      </h4>
                      <p className="mt-2 text-sm leading-6 text-muted-foreground">
                        {friendlyAuthorityDetail(c.authority_boundary)}
                      </p>
                      <div className="mt-3 grid gap-2 text-xs">
                        <div className="rounded-lg border bg-background/70 p-3">
                          <span className="block text-muted-foreground">
                            Observe scope
                          </span>
                          <span className="font-medium">
                            {friendlySemanticLabel(c.observe_scope)}
                          </span>
                        </div>
                        <div className="rounded-lg border bg-background/70 p-3">
                          <span className="block text-muted-foreground">
                            Control path
                          </span>
                          <span className="font-medium">
                            {friendlySemanticLabel(c.enforce_scope)}
                          </span>
                        </div>
                      </div>
                    </div>
                    <div className="rounded-xl border bg-muted/30 p-4">
                      <h4 className="text-sm font-semibold">
                        Identity and grouping
                      </h4>
                      <div className="mt-2 space-y-2 text-sm">
                        <div>
                          <span className="text-muted-foreground">
                            Canonical service:
                          </span>{" "}
                          <span className="font-medium">
                            {friendlySemanticLabel(c.canonical_service_id)}
                          </span>
                        </div>
                        <div>
                          <span className="text-muted-foreground">
                            Surface group:
                          </span>{" "}
                          <span className="font-medium">
                            {friendlySemanticLabel(c.surface_group_id)}
                          </span>
                        </div>
                        {c.control_parent_id && (
                          <div>
                            <span className="text-muted-foreground">
                              Controlled through:
                            </span>{" "}
                            <span className="break-all font-medium">
                              {c.control_parent_id}
                            </span>
                          </div>
                        )}
                      </div>
                      {c.grouping_reason && (
                        <p className="mt-3 rounded-lg border bg-background/70 p-3 text-xs leading-5 text-muted-foreground">
                          {c.grouping_reason}
                        </p>
                      )}
                      {c.duplicate_policy === "needs_human_confirmation" && (
                        <p className="mt-3 rounded-lg border border-amber-500/25 bg-amber-500/10 p-3 text-xs leading-5 text-amber-700">
                          This was detected from weak or network-only evidence.
                          Review it before registering or applying controls.
                        </p>
                      )}
                    </div>
                  </div>

                  {c.observation_coverage &&
                    c.observation_coverage.length > 0 && (
                      <div className="rounded-xl border bg-muted/30 p-4">
                        <h4 className="text-sm font-semibold">
                          Observability coverage
                        </h4>
                        <p className="mt-1 text-sm text-muted-foreground">
                          What Pollek can observe for this{" "}
                          {c.inferred_agent_type} agent, and how each signal is
                          collected.
                        </p>
                        <div className="mt-3 grid gap-2 md:grid-cols-2">
                          {c.observation_coverage.map((signal) => (
                            <div
                              key={signal.signal}
                              className="rounded-lg border bg-background/70 p-3"
                            >
                              <div className="flex items-center justify-between gap-2">
                                <span className="text-sm font-medium">
                                  {signal.label}
                                </span>
                                <span
                                  className={`rounded-full px-2 py-0.5 text-[11px] font-medium ${
                                    signal.status === "active"
                                      ? "bg-emerald-500/15 text-emerald-700"
                                      : signal.status === "available"
                                        ? "bg-sky-500/15 text-sky-700"
                                        : "bg-muted text-muted-foreground"
                                  }`}
                                >
                                  {signal.status === "active"
                                    ? "Observed"
                                    : signal.status === "available"
                                      ? "Available"
                                      : "N/A"}
                                </span>
                              </div>
                              <p className="mt-1 text-xs text-muted-foreground">
                                {signal.method === "not_applicable"
                                  ? "Not applicable to this agent type."
                                  : `via ${signal.method}`}
                              </p>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}

                  {c.related_surfaces?.length > 0 && (
                    <div className="rounded-xl border bg-muted/30 p-4">
                      <h4 className="text-sm font-semibold">
                        Related surfaces
                      </h4>
                      <p className="mt-1 text-sm text-muted-foreground">
                        These surfaces were seen with this AI app and may be
                        controlled through the parent app instead of separately.
                      </p>
                      <div className="mt-3 grid gap-2 md:grid-cols-2">
                        {c.related_surfaces.map((surface) => (
                          <div
                            key={`${surface.service_id}-${surface.control_parent_id ?? "self"}`}
                            className="rounded-lg border bg-background/70 p-3"
                          >
                            <div className="flex items-start justify-between gap-2">
                              <div>
                                <div className="text-sm font-medium">
                                  {surface.display_name}
                                </div>
                                <div className="text-xs text-muted-foreground">
                                  {friendlySemanticLabel(surface.entity_role)} ·{" "}
                                  {friendlySemanticLabel(
                                    surface.authority_boundary,
                                  )}
                                </div>
                              </div>
                              <span className="rounded-full bg-primary/10 px-2 py-1 text-xs text-primary">
                                {(surface.confidence * 100).toFixed(0)}%
                              </span>
                            </div>
                            {surface.grouping_reason && (
                              <p className="mt-2 text-xs leading-5 text-muted-foreground">
                                {surface.grouping_reason}
                              </p>
                            )}
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  <div className="rounded-xl border bg-muted/30 p-4">
                    <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                      <div>
                        <h4 className="text-sm font-semibold">
                          Improve this definition
                        </h4>
                        <p className="mt-1 max-w-2xl text-sm leading-6 text-muted-foreground">
                          Build a local learned profile from this candidate,
                          safe public metadata source choices, and the current
                          evidence. This does not install packages, execute
                          code, invoke MCP tools, or read prompts.
                        </p>
                      </div>
                      <button
                        type="button"
                        onClick={() => void startEnrichment(c.candidate_id)}
                        disabled={enrichmentBusy}
                        className="inline-flex items-center justify-center gap-2 rounded-lg border bg-background px-3 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
                      >
                        <RefreshCw
                          className={`h-4 w-4 ${
                            enrichmentBusy ? "animate-spin" : ""
                          }`}
                        />
                        {enrichmentSession ? "Rebuild profile" : "Enrich"}
                      </button>
                    </div>

                    {enrichmentSession && (
                      <div className="mt-4 space-y-3">
                        <div className="grid gap-2 md:grid-cols-2">
                          {enrichmentSession.source_plan.map((source) => (
                            <div
                              key={source.source_id}
                              className="rounded-lg border bg-background/70 p-3"
                            >
                              <div className="flex items-start justify-between gap-2">
                                <div>
                                  <div className="text-sm font-medium">
                                    {source.label}
                                  </div>
                                  <div className="mt-1 text-xs text-muted-foreground">
                                    {friendlySemanticLabel(source.safety)}
                                  </div>
                                </div>
                                <span className="rounded-full bg-primary/10 px-2 py-1 text-xs text-primary">
                                  {source.allowed ? "Selected" : "Optional"}
                                </span>
                              </div>
                            </div>
                          ))}
                        </div>

                        <div className="rounded-lg border bg-background/70 p-3 text-xs leading-5 text-muted-foreground">
                          <div className="font-medium text-foreground">
                            Privacy guardrails
                          </div>
                          <div className="mt-1">
                            {enrichmentSession.privacy_guardrails.join(" · ")}
                          </div>
                        </div>

                        {enrichmentSession.research_result && (
                          <div className="rounded-lg border bg-background/70 p-3 text-sm">
                            <div className="font-medium">Enrichment result</div>
                            <p className="mt-1 text-muted-foreground">
                              {enrichmentSession.research_result.summary}
                            </p>
                          </div>
                        )}

                        <div className="flex flex-wrap gap-2">
                          {enrichmentSession.status ===
                            "waiting_for_consent" && (
                            <button
                              type="button"
                              onClick={() =>
                                void approveEnrichment(c.candidate_id)
                              }
                              disabled={enrichmentBusy}
                              className="inline-flex items-center justify-center gap-2 rounded-lg bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
                            >
                              Approve safe sources
                            </button>
                          )}
                          {enrichmentSession.status === "researched" && (
                            <button
                              type="button"
                              onClick={() =>
                                void submitEnrichment(c.candidate_id)
                              }
                              disabled={enrichmentBusy}
                              className="inline-flex items-center justify-center gap-2 rounded-lg bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
                            >
                              Save local profile
                            </button>
                          )}
                          {enrichmentSession.status === "submitted" && (
                            <span className="rounded-full bg-emerald-500/10 px-3 py-2 text-sm font-medium text-emerald-700">
                              Local learned profile saved
                            </span>
                          )}
                        </div>
                      </div>
                    )}
                  </div>

                  <ReferenceIntelGuide
                    reference={primaryReference}
                    observedTerms={observedTermsForCandidate(c)}
                  />

                  <div className="grid gap-4 md:grid-cols-2">
                    <div className="rounded-xl border bg-muted/30 p-4">
                      <h4 className="text-sm font-semibold">
                        Definition and reference version
                      </h4>
                      <p className="mt-2 text-sm leading-6 text-muted-foreground">
                        {primaryReference
                          ? `${primaryReference.title} matched from ${primaryReference.sourceLabel}. Reviewed ${primaryReference.reviewedAt}.`
                          : "No curated reference profile matched yet. This record is based only on local discovery evidence."}
                      </p>
                    </div>
                    <div className="rounded-xl border bg-muted/30 p-4">
                      <h4 className="text-sm font-semibold">
                        Browser limitation
                      </h4>
                      <p className="mt-2 text-sm leading-6 text-muted-foreground">
                        {browserName || sourceSummary.includes("web")
                          ? "Browser metadata can identify ChatGPT, Claude, DeepSeek, Manus, Antigravity, or similar AI surfaces, but exact prompts, responses, files, and tokens require a browser extension, proxy, wrapper, SDK adapter, or plugin in the data path."
                          : "This candidate was not primarily identified as a browser AI surface, but exact activity still depends on the available local observer, wrapper, proxy, or plugin path."}
                      </p>
                    </div>
                  </div>

                  <div className="grid grid-cols-2 gap-4 text-sm">
                    <div className="p-4 bg-muted/30 rounded-xl border">
                      <span className="text-muted-foreground block mb-1">
                        Confidence
                      </span>
                      <span className="font-semibold">
                        {(c.confidence * 100).toFixed(0)}%
                      </span>
                    </div>
                    <div className="p-4 bg-muted/30 rounded-xl border">
                      <span className="text-muted-foreground block mb-1">
                        Risk Score
                      </span>
                      <span className="font-semibold text-amber-500">
                        {c.risk_score}
                      </span>
                    </div>
                    <div className="p-4 bg-muted/30 rounded-xl border">
                      <span className="text-muted-foreground block mb-1">
                        First Seen
                      </span>
                      <span>{new Date(c.first_seen).toLocaleString()}</span>
                    </div>
                    <div className="p-4 bg-muted/30 rounded-xl border">
                      <span className="text-muted-foreground block mb-1">
                        Last Seen
                      </span>
                      <span>{new Date(c.last_seen).toLocaleString()}</span>
                    </div>
                  </div>

                  <div className="p-4 bg-muted/30 rounded-xl border">
                    <h4 className="text-sm font-semibold mb-3">
                      Detected Capabilities
                    </h4>
                    {caps.length > 0 ? (
                      <div className="grid gap-2 md:grid-cols-2">
                        {caps.map((cap) => (
                          <div
                            key={cap}
                            className="rounded-lg border bg-background/70 p-3"
                          >
                            <div className="text-sm font-medium">
                              {friendlyCapabilityLabel(cap)}
                            </div>
                            <p className="mt-1 text-xs leading-5 text-muted-foreground">
                              {friendlyCapabilityDetail(cap)}
                            </p>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <p className="text-sm text-muted-foreground">
                        No capabilities inferred yet.
                      </p>
                    )}
                  </div>

                  {c.discovered_mcp_servers &&
                    c.discovered_mcp_servers.length > 0 && (
                      <div className="p-4 bg-muted/30 rounded-xl border">
                        <h4 className="text-sm font-semibold mb-2">
                          Discovered MCP Servers
                        </h4>
                        <ul className="text-sm space-y-1.5 text-muted-foreground">
                          {c.discovered_mcp_servers.map(
                            (mcp: any, i: number) => (
                              <li key={i} className="flex items-center gap-2">
                                <Play className="h-3 w-3 text-primary" />
                                <span className="text-foreground/80">
                                  {mcp.server_name} ({mcp.transport})
                                </span>
                              </li>
                            ),
                          )}
                        </ul>
                      </div>
                    )}
                </div>
              ),
            },
            {
              id: "capabilities",
              label: "Capabilities",
              content: (
                <div className="space-y-5">
                  <div className="flex flex-col gap-3 rounded-xl border bg-muted/30 p-4 md:flex-row md:items-center md:justify-between">
                    <div>
                      <h4 className="text-sm font-semibold">
                        Canonical Entity Inventory
                      </h4>
                      <p className="mt-1 text-sm text-muted-foreground">
                        Derived from local discovery metadata only. No MCP tools
                        are invoked and no resource contents are read.
                      </p>
                    </div>
                    <button
                      onClick={() =>
                        void loadCapabilities(c.candidate_id, true)
                      }
                      disabled={capabilityLoading}
                      className="inline-flex items-center justify-center gap-2 rounded-lg border bg-background px-3 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
                    >
                      <RefreshCw
                        className={`h-4 w-4 ${
                          capabilityLoading ? "animate-spin" : ""
                        }`}
                      />
                      Refresh inventory
                    </button>
                  </div>

                  <div className="grid grid-cols-1 gap-3 text-sm md:grid-cols-3">
                    <div className="rounded-xl border bg-muted/30 p-4">
                      <span className="block text-muted-foreground">
                        Entity Kind
                      </span>
                      <span className="font-semibold">
                        {entity?.entity_kind?.replace(/_/g, " ") ||
                          c.inferred_agent_type}
                      </span>
                    </div>
                    <div className="rounded-xl border bg-muted/30 p-4">
                      <span className="block text-muted-foreground">
                        Privacy
                      </span>
                      <span className="font-semibold">
                        {entity?.privacy_profile?.replace(/_/g, " ") ||
                          "metadata only"}
                      </span>
                    </div>
                    <div className="rounded-xl border bg-muted/30 p-4">
                      <span className="block text-muted-foreground">
                        Collection Cost
                      </span>
                      <span className="font-semibold">
                        {entity?.performance_cost_class?.replace(/_/g, " ") ||
                          "passive metadata"}
                      </span>
                    </div>
                  </div>

                  <div className="rounded-xl border bg-muted/30 p-4">
                    <div className="mb-3 flex items-center justify-between gap-2">
                      <h4 className="text-sm font-semibold">Capabilities</h4>
                      <span className="rounded-full bg-background px-2 py-1 text-xs text-muted-foreground">
                        {canonicalCapabilities.length} found
                      </span>
                    </div>
                    {canonicalCapabilities.length > 0 ? (
                      <div className="space-y-3">
                        {canonicalCapabilities.map((capability) => (
                          <div
                            key={capability.capability_id}
                            className="rounded-lg border bg-background/70 p-3"
                          >
                            <div className="flex flex-wrap items-start justify-between gap-2">
                              <div>
                                <div className="font-medium">
                                  {capability.name}
                                </div>
                                <div className="text-xs text-muted-foreground">
                                  {capability.capability_kind.replace(
                                    /_/g,
                                    " ",
                                  )}{" "}
                                  from {capability.source}
                                </div>
                              </div>
                              <span className="rounded-full bg-primary/10 px-2 py-1 text-xs text-primary">
                                {(capability.confidence * 100).toFixed(0)}%
                              </span>
                            </div>
                            {capability.description && (
                              <p className="mt-2 text-sm text-muted-foreground">
                                {capability.description}
                              </p>
                            )}
                            <div className="mt-3 flex flex-wrap gap-2">
                              {[
                                ...capability.modality,
                                ...capability.actions,
                                ...capability.risk_tags,
                              ].map((tag) => (
                                <span
                                  key={`${capability.capability_id}-${tag}`}
                                  className="rounded-md border px-2 py-1 text-xs text-muted-foreground"
                                >
                                  {tag.replace(/_/g, " ")}
                                </span>
                              ))}
                            </div>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <p className="text-sm text-muted-foreground">
                        Select Refresh inventory to derive capabilities from the
                        latest discovery evidence.
                      </p>
                    )}
                  </div>

                  <div className="rounded-xl border bg-muted/30 p-4">
                    <div className="mb-3 flex items-center justify-between gap-2">
                      <h4 className="text-sm font-semibold">Relationships</h4>
                      <span className="rounded-full bg-background px-2 py-1 text-xs text-muted-foreground">
                        {relationships.length} link(s)
                      </span>
                    </div>
                    {relationships.length > 0 ? (
                      <div className="space-y-2">
                        {relationships.map((relationship) => (
                          <div
                            key={relationship.relationship_id}
                            className="flex items-start gap-2 rounded-lg border bg-background/70 p-3 text-sm"
                          >
                            <Link2 className="mt-0.5 h-4 w-4 text-primary" />
                            <div>
                              <div className="font-medium">
                                {relationship.relation.replace(/_/g, " ")}
                              </div>
                              <div className="break-all text-xs text-muted-foreground">
                                {relationship.subject_candidate_id} to{" "}
                                {relationship.object_candidate_id}
                              </div>
                            </div>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <p className="text-sm text-muted-foreground">
                        No relationships derived yet.
                      </p>
                    )}
                  </div>
                </div>
              ),
            },
            {
              id: "activity",
              label: "Activity",
              content: (
                <div className="space-y-5">
                  <div className="flex flex-col gap-3 rounded-xl border bg-muted/30 p-4 md:flex-row md:items-center md:justify-between">
                    <div>
                      <h4 className="text-sm font-semibold">
                        Observed Activity
                      </h4>
                      <p className="mt-1 text-sm text-muted-foreground">
                        Resource access, tool calls, and token/cost usage
                        observed for this AI app only. Run Observe Now to
                        collect fresh events.
                      </p>
                    </div>
                    <button
                      onClick={() => void loadAgentActivity(c, true)}
                      disabled={activityLoading}
                      className="inline-flex items-center justify-center gap-2 rounded-lg border bg-background px-3 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
                    >
                      <RefreshCw
                        className={`h-4 w-4 ${
                          activityLoading ? "animate-spin" : ""
                        }`}
                      />
                      Refresh activity
                    </button>
                  </div>

                  <div className="grid grid-cols-2 gap-3 text-sm md:grid-cols-4">
                    <div className="rounded-xl border bg-muted/30 p-4">
                      <span className="block text-muted-foreground">
                        Events
                      </span>
                      <span className="text-lg font-semibold">
                        {activityView?.counts.total_events ?? 0}
                      </span>
                    </div>
                    <div className="rounded-xl border bg-muted/30 p-4">
                      <span className="block text-muted-foreground">
                        AI Requests
                      </span>
                      <span className="text-lg font-semibold">
                        {activityView?.usage.request_count ?? 0}
                      </span>
                    </div>
                    <div className="rounded-xl border bg-muted/30 p-4">
                      <span className="block text-muted-foreground">
                        Tokens
                      </span>
                      <span className="text-lg font-semibold">
                        {(
                          activityView?.usage.total_tokens ?? 0
                        ).toLocaleString()}
                      </span>
                      <span className="block text-xs text-muted-foreground">
                        {(
                          activityView?.usage.input_tokens ?? 0
                        ).toLocaleString()}{" "}
                        in /{" "}
                        {(
                          activityView?.usage.output_tokens ?? 0
                        ).toLocaleString()}{" "}
                        out
                      </span>
                    </div>
                    <div className="rounded-xl border bg-muted/30 p-4">
                      <span className="block text-muted-foreground">Cost</span>
                      <span className="text-lg font-semibold">
                        {(activityView?.usage.total_cost ?? 0).toLocaleString(
                          undefined,
                          { maximumFractionDigits: 4 },
                        )}{" "}
                        {activityView?.usage.currency ?? "USD"}
                      </span>
                      <span className="block text-xs text-muted-foreground">
                        {activityView?.usage.exact_events ?? 0} exact /{" "}
                        {activityView?.usage.estimated_events ?? 0} estimated
                      </span>
                    </div>
                  </div>

                  {(activityView?.usage.by_model.length ?? 0) > 0 && (
                    <div className="rounded-xl border bg-muted/30 p-4">
                      <h4 className="mb-3 text-sm font-semibold">
                        Usage by Model
                      </h4>
                      <div className="space-y-2">
                        {activityView?.usage.by_model.map((row) => (
                          <div
                            key={row.model}
                            className="flex flex-wrap items-center justify-between gap-2 rounded-lg border bg-background/70 p-3 text-sm"
                          >
                            <span className="font-medium">{row.model}</span>
                            <span className="text-muted-foreground">
                              {row.request_count} request(s) -{" "}
                              {row.total_tokens.toLocaleString()} tokens -{" "}
                              {row.total_cost.toLocaleString(undefined, {
                                maximumFractionDigits: 4,
                              })}{" "}
                              {activityView?.usage.currency ?? "USD"}
                            </span>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  <div className="rounded-xl border bg-muted/30 p-4">
                    <div className="mb-3 flex items-center justify-between gap-2">
                      <h4 className="text-sm font-semibold">
                        Resources Accessed
                      </h4>
                      <span className="rounded-full bg-background px-2 py-1 text-xs text-muted-foreground">
                        {activityView?.resources.length ?? 0} target(s)
                      </span>
                    </div>
                    {(activityView?.resources.length ?? 0) > 0 ? (
                      <div className="space-y-2">
                        {activityView?.resources.map((resource) => (
                          <div
                            key={resource.target}
                            className="rounded-lg border bg-background/70 p-3 text-sm"
                          >
                            <div className="flex flex-wrap items-center justify-between gap-2">
                              <span className="break-all font-medium">
                                {resource.target}
                              </span>
                              <span className="rounded-full bg-primary/10 px-2 py-1 text-xs text-primary">
                                {resource.access_count} access(es)
                              </span>
                            </div>
                            <div className="mt-1 text-xs text-muted-foreground">
                              {resource.resource_type} -{" "}
                              {resource.verbs.join(", ")} - last seen{" "}
                              {new Date(resource.last_seen).toLocaleString()}
                            </div>
                          </div>
                        ))}
                      </div>
                    ) : (
                      <p className="text-sm text-muted-foreground">
                        No resource access has been observed for this AI app
                        yet.
                      </p>
                    )}
                  </div>

                  <div className="rounded-xl border bg-muted/30 p-4">
                    <div className="mb-3 flex items-center justify-between gap-2">
                      <h4 className="text-sm font-semibold">Recent Events</h4>
                      <span className="rounded-full bg-background px-2 py-1 text-xs text-muted-foreground">
                        {activityView?.counts.denied_actions ?? 0} denied
                      </span>
                    </div>
                    {(activityView?.activity.length ?? 0) > 0 ? (
                      <div className="space-y-2">
                        {activityView?.activity
                          .slice(-30)
                          .reverse()
                          .map((item, index) => (
                            <div
                              key={`${item.timestamp}-${index}`}
                              className="flex flex-wrap items-center justify-between gap-2 rounded-lg border bg-background/70 p-3 text-sm"
                            >
                              <div className="min-w-0">
                                <span className="font-medium">
                                  {item.event_type.replace(/_/g, " ")}
                                </span>
                                <span className="ml-2 break-all text-muted-foreground">
                                  {item.resource}
                                </span>
                              </div>
                              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                                {item.decision && (
                                  <span
                                    className={`rounded-full px-2 py-0.5 ${
                                      item.decision === "deny"
                                        ? "bg-destructive/10 text-destructive"
                                        : "bg-primary/10 text-primary"
                                    }`}
                                  >
                                    {item.decision}
                                  </span>
                                )}
                                <span>
                                  {new Date(item.timestamp).toLocaleString()}
                                </span>
                              </div>
                            </div>
                          ))}
                      </div>
                    ) : (
                      <p className="text-sm text-muted-foreground">
                        No activity events yet. Use Observe Now, or install a
                        control method (MCP wrapper, proxy, or browser
                        extension) to capture live per-agent activity.
                      </p>
                    )}
                  </div>
                </div>
              ),
            },
            {
              id: "advanced",
              label: "Advanced Evidence",
              content: (
                <div className="space-y-4">
                  <p className="text-sm text-muted-foreground mb-2">
                    Raw evidence details and system telemetry used to identify
                    this candidate.
                  </p>
                  <Collapsible title="Raw Evidence JSON">
                    <pre className="text-[10px] font-mono bg-transparent p-0 rounded-none overflow-x-auto border-0">
                      {JSON.stringify(c.evidence, null, 2)}
                    </pre>
                  </Collapsible>
                  <h4 className="font-medium mt-4 mb-2 flex items-center gap-2 text-sm">
                    <Info className="h-4 w-4" /> Full JSON payload
                  </h4>
                  <Collapsible title="Full JSON Payload">
                    <pre className="text-[10px] font-mono bg-transparent p-0 rounded-none overflow-x-auto border-0">
                      {JSON.stringify(c, null, 2)}
                    </pre>
                  </Collapsible>
                </div>
              ),
            },
          ];

          return (
            <div className="flex h-full flex-col overflow-hidden rounded-xl bg-card/40">
              <div className="border-b px-5 py-4">
                <div className="flex flex-col gap-3 xl:flex-row xl:items-start xl:justify-between">
                  <div className="min-w-0">
                    <div className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
                      Discovery Candidate
                    </div>
                    <h3 className="mt-1 break-words text-xl font-semibold tracking-tight">
                      {displayNameForCandidate(c)}
                    </h3>
                    <p className="mt-1 text-sm text-muted-foreground">
                      {primaryReference?.category ?? c.inferred_agent_type} -
                      detected from {sourceSummary || "local metadata"}
                    </p>
                    <div className="mt-3">
                      <AgentLifecycleBadges lifecycle={lifecycle} size="md" />
                    </div>
                  </div>
                  <StatusChip status={status} label={candidateStatusLabel} />
                </div>
              </div>

              <div className="grid min-h-0 flex-1 gap-4 overflow-y-auto p-4 xl:grid-cols-[240px_minmax(0,1fr)] 2xl:grid-cols-[260px_minmax(0,1fr)_300px]">
                <aside className="space-y-3">
                  <section className="rounded-lg border bg-background/50 p-4">
                    <h3 className="mb-3 flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      <span className="h-1.5 w-1.5 rounded-full bg-primary" />
                      Candidate Summary
                    </h3>
                    <div className="space-y-2 text-sm">
                      <div className="border-b border-border/40 pb-2">
                        <div className="text-xs text-muted-foreground">
                          Provider
                        </div>
                        <div className="mt-0.5 break-words font-medium">
                          {c.vendor || primaryReference?.vendor || "Unknown"}
                        </div>
                      </div>
                      <div className="border-b border-border/40 pb-2">
                        <div className="text-xs text-muted-foreground">
                          Runtime
                        </div>
                        <div className="mt-0.5 break-words font-medium">
                          {browserName || c.inferred_agent_type}
                        </div>
                      </div>
                      <div className="border-b border-border/40 pb-2">
                        <div className="text-xs text-muted-foreground">
                          Confidence
                        </div>
                        <div className="mt-0.5 font-medium">
                          {(c.confidence * 100).toFixed(0)}%
                        </div>
                      </div>
                      <div className="border-b border-border/40 pb-2">
                        <div className="text-xs text-muted-foreground">
                          Risk score
                        </div>
                        <div className="mt-0.5 font-medium text-amber-600">
                          {c.risk_score}
                        </div>
                      </div>
                      <div className="border-b border-border/40 pb-2">
                        <div className="text-xs text-muted-foreground">
                          Evidence
                        </div>
                        <div className="mt-0.5 font-medium">
                          {c.evidence?.length ?? 0} signal(s)
                        </div>
                      </div>
                      <div className="border-b border-border/40 pb-2">
                        <div className="text-xs text-muted-foreground">
                          Last seen
                        </div>
                        <div className="mt-0.5 break-words font-medium capitalize">
                          {lifecycle.lastSeenLabel}
                        </div>
                      </div>
                      <div>
                        <div className="text-xs text-muted-foreground">
                          Scan
                        </div>
                        <div className="mt-0.5 break-words font-medium">
                          {scanLabelForCandidate(c)}
                        </div>
                      </div>
                    </div>
                  </section>
                </aside>

                <section className="min-w-0">
                  <DetailPane
                    title="Detail Workspace"
                    subtitle="Review friendly details, canonical capabilities, and raw evidence for this candidate."
                    status={status}
                    statusLabel={isRegistered ? "Registered" : "Pending"}
                    tabs={tabs}
                  />
                </section>

                <aside className="space-y-3 xl:col-span-2 2xl:col-span-1">
                  <section className="rounded-lg border bg-background/50 p-4">
                    <h3 className="text-sm font-semibold">Actions</h3>
                    <div className="mt-3 flex flex-wrap gap-2">
                      {actions.map((action) => {
                        const ActionIcon = action.icon;
                        return (
                          <button
                            key={action.label}
                            type="button"
                            onClick={action.onClick}
                            disabled={action.disabled}
                            className={`inline-flex h-9 items-center gap-2 rounded-md px-3 text-sm font-medium disabled:opacity-50 ${
                              action.danger
                                ? "border border-red-500/30 bg-red-500/10 text-red-700 hover:bg-red-500/15"
                                : action.primary
                                  ? "bg-primary text-primary-foreground hover:bg-primary/90"
                                  : "border bg-background hover:bg-muted"
                            }`}
                          >
                            {ActionIcon && <ActionIcon className="h-4 w-4" />}
                            {action.label}
                          </button>
                        );
                      })}
                    </div>
                  </section>

                  <section className="rounded-lg border bg-background/50 p-4">
                    <h3 className="text-sm font-semibold">What this means</h3>
                    <p className="mt-2 text-sm leading-6 text-muted-foreground">
                      Pollek found this AI-related entity from metadata and
                      local signals. Confirm it when the identity looks right,
                      then observe activity before adding stricter controls.
                    </p>
                  </section>

                  {primaryReference ? (
                    <section className="rounded-lg border bg-background/50 p-4">
                      <h3 className="text-sm font-semibold">Known Profile</h3>
                      <div className="mt-3">
                        <ReferenceIntelMark reference={primaryReference} />
                      </div>
                      <p className="mt-3 text-xs leading-5 text-muted-foreground">
                        Matched definitions explain what this AI app commonly
                        does. Local evidence remains the source of truth.
                      </p>
                    </section>
                  ) : null}

                  <section className="rounded-lg border bg-background/50 p-4">
                    <h3 className="text-sm font-semibold">Privacy</h3>
                    <p className="mt-2 text-xs leading-5 text-muted-foreground">
                      Discovery uses local metadata such as process names,
                      browser titles, config references, and redacted paths. It
                      does not read file contents or raw prompts for this view.
                    </p>
                  </section>
                </aside>
              </div>
            </div>
          );
        }}
      />

      {protectTarget && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
          <div
            className="fixed inset-0 bg-background/80 backdrop-blur-sm transition-opacity"
            onClick={() => setProtectTarget(null)}
          />
          <div className="relative z-50 w-full max-w-3xl rounded-xl border bg-card p-6 shadow-lg max-h-[90vh] overflow-y-auto">
            <button
              onClick={() => setProtectTarget(null)}
              className="absolute right-4 top-4 rounded-sm opacity-70 ring-offset-background transition-opacity hover:opacity-100"
            >
              <span className="sr-only">Close</span>
              <svg
                width="15"
                height="15"
                viewBox="0 0 15 15"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
              >
                <path
                  d="M12.8536 2.85355C13.0488 2.65829 13.0488 2.34171 12.8536 2.14645C12.6583 1.95118 12.3417 1.95118 12.1464 2.14645L7.5 6.79289L2.85355 2.14645C2.65829 1.95118 2.34171 1.95118 2.14645 2.14645C1.95118 2.34171 1.95118 2.65829 2.14645 2.85355L6.79289 7.5L2.14645 12.1464C1.95118 12.3417 1.95118 12.6583 2.14645 12.8536C2.34171 13.0488 2.65829 13.0488 2.85355 12.8536L7.5 8.20711L12.1464 12.8536C12.3417 13.0488 12.6583 13.0488 12.8536 12.8536C13.0488 12.6583 13.0488 12.3417 12.8536 12.1464L8.20711 7.5L12.8536 2.85355Z"
                  fill="currentColor"
                  fillRule="evenodd"
                  clipRule="evenodd"
                ></path>
              </svg>
            </button>
            <SimplePolicyWizard
              agents={visibleCandidates.map((c) => ({
                id: c.candidate_id,
                label: displayNameForCandidate(c),
              }))}
              initialTarget={protectTarget}
              onComplete={() => {
                setProtectTarget(null);
                toast.success("Protection applied successfully");
                void fetchCandidates();
              }}
              onCancel={() => setProtectTarget(null)}
            />
          </div>
        </div>
      )}

      {confirmTarget && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
          <div
            className="fixed inset-0 bg-background/80 backdrop-blur-sm transition-opacity"
            onClick={() => setConfirmTarget(null)}
          />
          <div className="relative z-50 w-full max-w-md rounded-xl border bg-card p-6 shadow-lg">
            <h3 className="text-lg font-semibold mb-4">Register AI app</h3>
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium mb-1 text-muted-foreground">
                  Friendly Name
                </label>
                <input
                  type="text"
                  value={editName}
                  onChange={(e) => setEditName(e.target.value)}
                  className="w-full px-3 py-2 text-sm rounded-md border bg-background"
                  autoFocus
                />
              </div>
              <p className="text-sm text-muted-foreground">
                This AI app will appear in My AI Apps and can be managed with
                rules, setup, and activity history.
              </p>
              <div className="flex justify-end gap-2 mt-6">
                <button
                  onClick={() => setConfirmTarget(null)}
                  className="px-4 py-2 text-sm font-medium rounded-md border bg-background hover:bg-muted"
                >
                  Cancel
                </button>
                <button
                  onClick={submitConfirmAgent}
                  disabled={confirmingId === confirmTarget.candidate_id}
                  className="px-4 py-2 text-sm font-medium rounded-md bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
                >
                  {confirmingId === confirmTarget.candidate_id
                    ? "Registering..."
                    : "Register Agent"}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
