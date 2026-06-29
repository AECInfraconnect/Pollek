import { useConfirm } from "../components/ui/ConfirmDialog";
import { toast } from "sonner";
import { useState, useEffect, useRef } from "react";
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
  type LocalObserveRefreshResponse,
} from "../services/api";
import type {
  DiscoveredAgentCandidateV2,
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
    detail: "Browser/window metadata for ChatGPT, Claude, DeepSeek, and similar surfaces.",
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
  const [searchQuery, setSearchQuery] = useState("");
  const [isScanning, setIsScanning] = useState(false);
  const [isFinalizingScan, setIsFinalizingScan] = useState(false);
  const [isObserving, setIsObserving] = useState(false);
  const [observeResult, setObserveResult] =
    useState<LocalObserveRefreshResponse | null>(null);
  const [scans, setScans] = useState<DiscoveryScanJob[]>([]);
  const [scanFilter, setScanFilter] = useState<string>("latest");
  const scanTriggered = useRef(false);
  const [capabilityInventories, setCapabilityInventories] = useState<
    Record<string, DiscoveryCapabilityInventory>
  >({});
  const [capabilityLoadingId, setCapabilityLoadingId] = useState<string | null>(
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

      const registeredFingerprints = new Set(
        discovered
          .filter(
            (c) => agentIds.has(c.candidate_id) || c.status === "registered",
          )
          .map((c) => c.evidence?.[0]?.merge_key || c.display_name),
      );

      const mergedCandidates = discovered.map((c) => {
        const fp = c.evidence?.[0]?.merge_key || c.display_name;
        if (agentIds.has(c.candidate_id)) {
          return { ...c, status: "registered", _agent_id: c.candidate_id };
        }
        const registeredPeer = discovered.find(
          (peer) =>
            agentIds.has(peer.candidate_id) &&
            (peer.evidence?.[0]?.merge_key || peer.display_name) === fp,
        );
        if (registeredPeer) {
          return {
            ...c,
            status: "registered",
            _agent_id: registeredPeer.candidate_id,
          };
        }
        if (registeredFingerprints.has(fp)) {
          return { ...c, status: "registered" };
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

    if (!scanTriggered.current) {
      scanTriggered.current = true;
      void triggerScan();
    }
  }, []);

  useEffect(() => {
    if (!selectedId || capabilityInventories[selectedId]) return;
    void loadCapabilities(selectedId, false);
  }, [selectedId]);

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

  const settleScanResults = async (expectedCount = 0) => {
    setIsFinalizingScan(true);
    let previousDigest = "";
    let stableReads = 0;

    try {
      for (let attempt = 0; attempt < 10; attempt += 1) {
        const next = await fetchCandidates({ showLoading: attempt === 0 });
        const digest = next
          .map(
            (c) =>
              `${c.candidate_id}:${c.status}:${c.last_seen}:${c.evidence?.length ?? 0}`,
          )
          .sort()
          .join("|");

        const hasExpectedCount =
          expectedCount === 0 || next.length >= expectedCount;
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
              await settleScanResults(status.candidates_found);
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
    candidate.last_scan_id ||
    candidate.scan_ids?.[candidate.scan_ids.length - 1] ||
    candidate.evidence
      ?.map((e) => e.data?.scan_id)
      .find((id) => typeof id === "string" && id.length > 0);

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
    return `${timeLabel} • ${scan.status}`;
  };

  const scanSortTime = (candidate: DiscoveredAgentCandidateV2) => {
    const scan = scanForCandidate(candidate);
    return new Date(
      scan?.started_at || scan?.finished_at || candidate.last_seen,
    ).getTime();
  };

  const visibleCandidates = candidates
    .filter((c) => {
      if (filter === "registered") return c.status === "registered";
      if (filter === "pending") return c.status !== "registered";
      return true;
    })
    .filter((c) => {
      if (scanFilter === "all") return true;
      const cScanId = scanIdForCandidate(c);
      if (scanFilter === "latest") {
        const latestScanId = scans[0]?.scan_id || scanJob?.scan_id;
        return cScanId === latestScanId || (!latestScanId && !cScanId);
      }
      return cScanId === scanFilter;
    })
    .filter((c) => {
      const query = searchQuery.trim().toLowerCase();
      if (!query) return true;
      return [
        displayNameForCandidate(c),
        c.vendor,
        c.inferred_agent_type,
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

  const registeredCount = candidates.filter(
    (candidate) => candidate.status === "registered",
  ).length;
  const pendingCount = candidates.length - registeredCount;
  const knownCount = candidates.filter(
    (candidate) => referenceForCandidate(candidate) != null,
  ).length;
  const evidenceCount = candidates.reduce(
    (total, candidate) => total + (candidate.evidence?.length ?? 0),
    0,
  );
  const latestScan = scans[0] ?? scanJob ?? null;
  const latestScanSources = new Set(latestScan?.sources ?? []);
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
        await settleScanResults((result as any).candidates_found ?? 0);
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
    <div className="p-6 md:p-8 space-y-6">
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

          <section className="grid gap-3 md:grid-cols-4">
            <div className="rounded-xl border bg-card/60 p-4">
              <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                <Search className="h-3.5 w-3.5" />
                Found
              </div>
              <div className="mt-2 text-2xl font-semibold">
                {candidates.length}
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
          </section>

          <section className="rounded-xl border bg-card/60 p-4">
            <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
              <div>
                <h3 className="text-sm font-semibold">Scan source coverage</h3>
                <p className="mt-1 max-w-3xl text-sm leading-6 text-muted-foreground">
                  Discovery means Pollek found AI apps or surfaces from local
                  metadata. Observe means Pollek later saw real activity. This
                  scan coverage shows what was checked, what still needs setup,
                  and what remains metadata-only.
                </p>
              </div>
              <span className="rounded-full border bg-background px-3 py-1 text-xs text-muted-foreground">
                {checkedSourceCount}/{DEEP_SCAN_SOURCES.length} sources checked
              </span>
            </div>
            <div className="mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-5">
              {DEEP_SCAN_SOURCES.map((source) => {
                const copy = SOURCE_LABELS[source] ?? {
                  label: source.replace(/_/g, " "),
                  detail: "Local metadata source.",
                };
                const checked = latestScanSources.has(source);
                const running =
                  latestScan?.status === "queued" ||
                  latestScan?.status === "running";
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
          </section>
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
                    {f.charAt(0).toUpperCase() + f.slice(1)}
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
                      {new Date(scan.started_at || scan.finished_at || 0).toLocaleString()} • {scan.status}
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
            actionLabel="Deep Scan"
            onAction={triggerScan}
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
          let status: UiStatus = "idle";
          if (c.status === "registered") status = "ok";
          else if (c.status === "pending_approval") status = "degraded";
          const caps = capabilityTags(c);
          const isRegistered = c.status === "registered";
          const browserName = browserNameForCandidate(c);
          const primaryReference = referenceForCandidate(c);

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
              statusLabel={isRegistered ? "Registered" : "Pending"}
              meta={[
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
                            ? "Confirming..."
                            : "Confirm Agent",
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
          let status: UiStatus = "idle";
          if (c.status === "registered") status = "ok";
          else if (c.status === "pending_approval") status = "degraded";
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

          const actions = [
            ...(isRegistered
              ? []
              : [
                  {
                    label:
                      confirmingId === c.candidate_id
                        ? "Confirming..."
                        : "Confirm Agent",
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
                    </div>
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
                  </div>
                  <StatusChip
                    status={status}
                    label={isRegistered ? "Registered" : "Pending"}
                  />
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
            <h3 className="text-lg font-semibold mb-4">
              Confirm Agent Registration
            </h3>
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
                This agent will be registered and appear in the Agents & Models
                inventory.
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
                    ? "Confirming..."
                    : "Confirm"}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
