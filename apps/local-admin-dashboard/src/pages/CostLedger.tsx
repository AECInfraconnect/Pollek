import { useEffect, useMemo, useState } from "react";
import {
  Activity,
  AlertTriangle,
  Bot,
  CircleDollarSign,
  Clock,
  FileText,
  Gauge,
  Info,
  ListFilter,
  RefreshCw,
  Search,
  Server,
  ShieldAlert,
  ShieldCheck,
  Wifi,
  Zap,
} from "lucide-react";
import { toast } from "sonner";
import {
  LocalObserveApi,
  RegistryApi,
  UsageApi,
  type AiUsageEventPage,
  type AiUsageSummary,
  type LocalObserveRefreshResponse,
} from "../services/api";
import { RegisterControlBar } from "../components/RegisterControlBar";
import { ObserveAccuracyPanel } from "../components/observe/ObserveAccuracyPanel";
import { useMode } from "../context/ModeContext";
import { isAdvanceMode } from "../lib/modes";
import type { ObserveInputKind } from "../services/api";

type RangeKey = "5m" | "1h" | "24h" | "7d" | "month";

type AgentLabel = {
  name: string;
  kind?: string;
};

type SyncCounts = {
  pending: number;
  sent: number;
  acked: number;
  failed: number;
};

type UsageEvidence = {
  exact: number;
  estimated: number;
  captureQuality: string[];
  latest?: string;
};

type ModelPoolStats = {
  poolKey: string;
  provider: string;
  model: string;
  calls: number;
  tokens: number;
  cost: number;
  exact: number;
  estimated: number;
  sharedAgents: number;
};

type AgentAttributionRow = {
  agentKey: string;
  agentName: string;
  agentType?: string;
  surfaces: string[];
  calls: number;
  tokens: number;
  cost: number;
  exact: number;
  estimated: number;
  pools: ModelPoolStats[];
  sharedPoolCount: number;
};

type AiUsageEvent = AiUsageEventPage["items"][number];
type UsageEventFilter = "all" | "exact" | "estimated" | "pending";

const ranges: Array<{ key: RangeKey; label: string; bucket: string }> = [
  { key: "5m", label: "5m", bucket: "1m" },
  { key: "1h", label: "1h", bucket: "1m" },
  { key: "24h", label: "24h", bucket: "1h" },
  { key: "7d", label: "7d", bucket: "1d" },
  { key: "month", label: "Month", bucket: "1d" },
];

function fromForRange(range: RangeKey) {
  const now = new Date();
  if (range === "month") {
    return new Date(now.getFullYear(), now.getMonth(), 1).toISOString();
  }
  const minutes =
    range === "5m" ? 5 : range === "1h" ? 60 : range === "24h" ? 1440 : 10080;
  return new Date(now.getTime() - minutes * 60_000).toISOString();
}

function money(value: number, currency = "USD") {
  return new Intl.NumberFormat(undefined, {
    style: "currency",
    currency,
    maximumFractionDigits: value < 1 ? 4 : 2,
  }).format(value || 0);
}

function number(value: number) {
  return new Intl.NumberFormat().format(value || 0);
}

function statusClass(status?: string) {
  if (status === "hard_exceeded") return "text-red-600 bg-red-500/10";
  if (status === "soft_exceeded") return "text-amber-600 bg-amber-500/10";
  return "text-emerald-600 bg-emerald-500/10";
}

function eventIsEstimated(event: AiUsageEvent) {
  return Boolean(event.tokens?.estimated || event.cost?.estimated);
}

function syncLabel(status?: string) {
  if (!status || status === "pending") return "Local only";
  if (status === "sent") return "Sent";
  if (status === "acked") return "Synced";
  if (status === "failed") return "Sync failed";
  return status.replace(/_/g, " ");
}

function usageEventKind(event: AiUsageEvent) {
  const fallback = event as AiUsageEvent & {
    kind?: string;
    type?: string;
    event_type?: string;
  };
  return (
    event.event_kind ||
    fallback.kind ||
    fallback.type ||
    fallback.event_type ||
    "usage_event"
  );
}

function usageEventTitle(event: AiUsageEvent) {
  const provider = event.provider || "AI provider";
  const model = event.model || "unknown model";
  const eventKind = usageEventKind(event);
  const kind = eventKind.replace(/_/g, " ");
  if (eventKind.includes("model_call")) {
    return `${provider} ${model}`;
  }
  if (event.tool_name) return `${event.tool_name} tool usage`;
  return kind.charAt(0).toUpperCase() + kind.slice(1);
}

function usageEstimateReason(event: AiUsageEvent) {
  const metadata = event.metadata as Record<string, unknown>;
  const quality =
    typeof metadata.capture_quality === "string"
      ? metadata.capture_quality.replace(/_/g, " ")
      : "";
  if (!eventIsEstimated(event)) {
    return event.provider_request_id
      ? "Exact provider or wrapper usage was attached to this event."
      : "Usage was recorded as exact by the local collector.";
  }
  if (quality.includes("browser")) {
    return "Estimated from browser or surface metadata because provider usage was not attached.";
  }
  if (event.tokens?.source) {
    return `Estimated from ${String(event.tokens.source).replace(/_/g, " ")} because exact provider tokens were unavailable.`;
  }
  if (!event.provider_request_id) {
    return "Estimated because this event has no provider request id or provider usage payload.";
  }
  return "Estimated by local fallback because exact token/cost telemetry was unavailable.";
}

function usageEventMatches(event: AiUsageEvent, query: string) {
  if (!query.trim()) return true;
  const haystack = [
    usageEventKind(event),
    event.provider,
    event.model,
    event.agent_id,
    event.agent_type,
    event.surface,
    event.tool_name,
    event.resource_type,
    event.status,
    event.cloud_sync_status,
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
  return haystack.includes(query.trim().toLowerCase());
}

function buildUsageEvidence(
  events: AiUsageEventPage["items"] = [],
): UsageEvidence {
  const captureQuality = new Set<string>();
  let exact = 0;
  let estimated = 0;
  let latest: string | undefined;

  for (const event of events) {
    if (!latest || event.occurred_at > latest) latest = event.occurred_at;
    const metadata = event.metadata as Record<string, unknown>;
    const quality = metadata.capture_quality;
    if (typeof quality === "string" && quality) captureQuality.add(quality);

    if (event.tokens?.estimated || event.cost?.estimated) {
      estimated += 1;
    } else {
      exact += 1;
    }
  }

  return {
    exact,
    estimated,
    captureQuality: Array.from(captureQuality).sort(),
    latest,
  };
}

function buildAgentAttribution(
  events: AiUsageEvent[],
  agentLabels: Map<string, AgentLabel>,
): AgentAttributionRow[] {
  const poolAgents = new Map<string, Set<string>>();
  for (const event of events) {
    const agentKey = usageAgentKey(event);
    const poolKey = usagePoolKey(event);
    if (!poolAgents.has(poolKey)) poolAgents.set(poolKey, new Set());
    poolAgents.get(poolKey)!.add(agentKey);
  }

  const rows = new Map<
    string,
    Omit<AgentAttributionRow, "surfaces" | "pools"> & {
      surfaces: Set<string>;
      pools: Map<string, ModelPoolStats>;
    }
  >();

  for (const event of events) {
    const agentKey = usageAgentKey(event);
    const label = agentLabels.get(agentKey);
    const estimated = eventIsEstimated(event);
    const provider = normalizedProvider(event.provider);
    const model = event.model || "Unknown model";
    const poolKey = usagePoolKey(event);
    const tokens = event.tokens?.total_tokens ?? 0;
    const cost = event.cost?.total_cost ?? 0;
    const row =
      rows.get(agentKey) ??
      ({
        agentKey,
        agentName: agentName(agentKey, agentLabels),
        agentType: label?.kind || event.agent_type || undefined,
        surfaces: new Set<string>(),
        calls: 0,
        tokens: 0,
        cost: 0,
        exact: 0,
        estimated: 0,
        pools: new Map<string, ModelPoolStats>(),
        sharedPoolCount: 0,
      } satisfies Omit<AgentAttributionRow, "surfaces" | "pools"> & {
        surfaces: Set<string>;
        pools: Map<string, ModelPoolStats>;
      });

    row.calls += 1;
    row.tokens += tokens;
    row.cost += cost;
    if (estimated) row.estimated += 1;
    else row.exact += 1;
    if (event.surface) row.surfaces.add(event.surface);
    if (event.agent_type) row.surfaces.add(event.agent_type);

    const pool =
      row.pools.get(poolKey) ??
      ({
        poolKey,
        provider,
        model,
        calls: 0,
        tokens: 0,
        cost: 0,
        exact: 0,
        estimated: 0,
        sharedAgents: poolAgents.get(poolKey)?.size ?? 1,
      } satisfies ModelPoolStats);
    pool.calls += 1;
    pool.tokens += tokens;
    pool.cost += cost;
    if (estimated) pool.estimated += 1;
    else pool.exact += 1;
    row.pools.set(poolKey, pool);
    rows.set(agentKey, row);
  }

  return Array.from(rows.values())
    .map((row) => {
      const pools = Array.from(row.pools.values()).sort(
        (a, b) =>
          b.cost - a.cost ||
          b.tokens - a.tokens ||
          a.model.localeCompare(b.model),
      );
      return {
        ...row,
        surfaces: Array.from(row.surfaces).sort(),
        pools,
        sharedPoolCount: pools.filter((pool) => pool.sharedAgents > 1).length,
      };
    })
    .sort(
      (a, b) =>
        b.cost - a.cost ||
        b.tokens - a.tokens ||
        a.agentName.localeCompare(b.agentName),
    );
}

function usageAgentKey(event: AiUsageEvent) {
  const extended = event as AiUsageEvent & {
    shadow_candidate_id?: string | null;
    app_id?: string | null;
  };
  return (
    event.agent_id ||
    extended.shadow_candidate_id ||
    extended.app_id ||
    event.surface ||
    event.agent_type ||
    "unknown-agent"
  );
}

function normalizedProvider(provider?: string | null) {
  const value = (provider || "Unknown provider").trim();
  if (!value) return "Unknown provider";
  if (value.toLowerCase() === "google") return "Google";
  if (value.toLowerCase() === "openai") return "OpenAI";
  return value;
}

function usagePoolKey(event: AiUsageEvent) {
  const provider = normalizedProvider(event.provider).toLowerCase();
  const model = (event.model || "unknown-model").toLowerCase();
  const metadata = (event.metadata ?? {}) as Record<string, unknown>;
  const explicitPool =
    typeof metadata.billing_pool === "string"
      ? metadata.billing_pool
      : typeof metadata.billing_account === "string"
        ? metadata.billing_account
        : typeof metadata.project_id === "string"
          ? metadata.project_id
          : "";
  return explicitPool
    ? `${provider}:${explicitPool}:${model}`
    : `${provider}:shared-credit:${model}`;
}

function providerBillingHint(provider: string) {
  const normalized = provider.toLowerCase();
  if (normalized.includes("azure")) {
    return "Azure OpenAI usage can be billed by subscription, resource, deployment, region, and model. Keep local app attribution separate before cloud-resource rollup.";
  }
  if (normalized.includes("openai")) {
    return "OpenAI usage can be grouped by project, API key, user, model, or service tier. Keep app-level attribution first, then reconcile with project/provider usage.";
  }
  if (normalized.includes("anthropic") || normalized.includes("claude")) {
    return "Anthropic organization reports can group by model, workspace, description, geo, or speed; local app attribution should remain separate before workspace rollup.";
  }
  if (
    normalized.includes("google") ||
    normalized.includes("gemini") ||
    normalized.includes("vertex")
  ) {
    return "Gemini/Vertex responses expose usage metadata; billed cost may still roll up under a Google Cloud project or shared credit pool.";
  }
  if (normalized.includes("bedrock") || normalized.includes("aws")) {
    return "Amazon Bedrock usage is account/region/model based and may route through inference profiles, so shared credit pools need a separate rollup.";
  }
  if (normalized.includes("xai") || normalized.includes("grok")) {
    return "xAI/Grok usage is usually OpenAI-compatible at response level, while billing may still roll up by API key, team, model, or account credit pool.";
  }
  if (normalized.includes("groq")) {
    return "Groq chat responses are OpenAI-compatible, but provider billing and rate limits can be keyed to organization, project, model, or hardware tier.";
  }
  if (normalized.includes("together")) {
    return "Together AI exposes OpenAI-compatible usage for many models; costs may pool by API key, organization, model owner, or routed endpoint.";
  }
  if (normalized.includes("openrouter")) {
    return "OpenRouter routes one request to many possible upstream providers. Keep local app usage first, then reconcile by OpenRouter route, upstream model, and shared credit balance.";
  }
  if (normalized.includes("perplexity")) {
    return "Perplexity/Sonar usage is usually OpenAI-compatible, while billing may combine model usage and search/reasoning tiers under one account.";
  }
  if (normalized.includes("fireworks")) {
    return "Fireworks usage often follows OpenAI-compatible token fields, but billing can roll up by serverless endpoint, deployment, model, or account credits.";
  }
  if (normalized.includes("cerebras")) {
    return "Cerebras inference can look OpenAI-compatible at response level; reconcile local usage against account/model billing before treating costs as exact.";
  }
  if (normalized.includes("cohere")) {
    return "Cohere responses distinguish token counts and billed units; reconcile app-level usage before provider billed-unit rollup.";
  }
  if (normalized.includes("mistral")) {
    return "Mistral chat responses include a usage object; billing can still be shared by key, workspace, or provider platform.";
  }
  if (normalized.includes("replicate")) {
    return "Replicate billing can be hardware/runtime based instead of pure tokens. Treat token fields as usage evidence and reconcile with provider cost records.";
  }
  if (normalized.includes("huggingface")) {
    return "Hugging Face inference can route through hosted endpoints, router providers, or local models; token usage and billed cost may come from different ledgers.";
  }
  if (
    normalized.includes("deepseek") ||
    normalized.includes("ollama") ||
    normalized.includes("lm studio") ||
    normalized.includes("local")
  ) {
    return "Local or OpenAI-compatible providers may expose response usage, but billed cost is often estimated unless a provider billing source is connected.";
  }
  return "Unknown or custom providers use generic response-usage and billing-pool grouping. Treat shared credits as a provider-level rollup, not an agent identity.";
}

function poolDisplayName(pool: ModelPoolStats) {
  const suffix =
    pool.sharedAgents > 1 ? ` / shared by ${pool.sharedAgents} apps` : "";
  return `${pool.provider} / ${pool.model}${suffix}`;
}

export function CostLedger() {
  const { mode } = useMode();
  const showTechnicalDetails = isAdvanceMode(mode);
  const [loading, setLoading] = useState(false);
  const [observeLoading, setObserveLoading] = useState(false);
  const [range, setRange] = useState<RangeKey>("24h");
  const [summary, setSummary] = useState<AiUsageSummary | null>(null);
  const [observeResult, setObserveResult] =
    useState<LocalObserveRefreshResponse | null>(null);
  const [usageEvidence, setUsageEvidence] = useState<UsageEvidence>({
    exact: 0,
    estimated: 0,
    captureQuality: [],
  });
  const [usageEvents, setUsageEvents] = useState<AiUsageEvent[]>([]);
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null);
  const [eventFilter, setEventFilter] = useState<UsageEventFilter>("all");
  const [eventSearch, setEventSearch] = useState("");
  const [agentLabels, setAgentLabels] = useState<Map<string, AgentLabel>>(
    new Map(),
  );
  const [syncCounts, setSyncCounts] = useState<SyncCounts>({
    pending: 0,
    sent: 0,
    acked: 0,
    failed: 0,
  });
  const [live, setLive] = useState(false);
  const [accuracyDialogKind, setAccuracyDialogKind] =
    useState<ObserveInputKind | null>(null);

  const activeRange = ranges.find((item) => item.key === range) ?? ranges[2];

  const fetchUsage = async (showSpinner = true) => {
    if (showSpinner) setLoading(true);
    try {
      const from = fromForRange(range);
      const [usage, events, agents, candidates] = await Promise.all([
        UsageApi.getSummary({ from, bucket: activeRange.bucket }),
        UsageApi.getEvents({ from, limit: 100 }),
        RegistryApi.listAgents().catch(() => []),
        RegistryApi.listDiscoveryCandidates().catch(() => []),
      ]);

      const names = new Map<string, AgentLabel>();
      for (const agent of agents) {
        names.set(agent.agent_id, {
          name: agent.name || agent.agent_id,
          kind: agent.agent_type,
        });
      }
      for (const candidate of candidates) {
        const displayName = candidate.display_name || candidate.candidate_id;
        const extended = candidate as typeof candidate & {
          matched_signature_id?: string;
        };
        names.set(candidate.candidate_id, {
          name: displayName,
          kind: candidate.inferred_agent_type,
        });
        for (const key of [
          candidate.canonical_service_id,
          candidate.surface_group_id,
          extended.matched_signature_id,
          candidate.labels?.canonical_service_id,
          candidate.labels?.surface_group_id,
        ]) {
          if (key) {
            names.set(key, {
              name: displayName,
              kind: candidate.inferred_agent_type,
            });
          }
        }
        const suggestedAgentId = candidate.suggested_registration?.agent_id;
        if (suggestedAgentId) {
          names.set(suggestedAgentId, {
            name: displayName,
            kind: candidate.inferred_agent_type,
          });
        }
      }
      for (const event of events.items ?? []) {
        const resolved =
          (event.shadow_candidate_id &&
            names.get(event.shadow_candidate_id)?.name) ||
          (event.agent_type && names.get(event.agent_type)?.name);
        if (event.agent_id && resolved) {
          names.set(event.agent_id, {
            name: resolved,
            kind: event.agent_type ?? undefined,
          });
        }
      }

      const counts: SyncCounts = { pending: 0, sent: 0, acked: 0, failed: 0 };
      for (const event of events.items ?? []) {
        const status = event.cloud_sync_status || "pending";
        if (status in counts) counts[status as keyof SyncCounts] += 1;
      }

      setSummary(usage);
      setUsageEvents(events.items ?? []);
      setSelectedEventId((current) => {
        if (
          current &&
          events.items?.some((event) => event.event_id === current)
        ) {
          return current;
        }
        return events.items?.[0]?.event_id ?? null;
      });
      setAgentLabels(names);
      setSyncCounts(counts);
      setUsageEvidence(buildUsageEvidence(events.items ?? []));
    } finally {
      if (showSpinner) setLoading(false);
    }
  };

  const observeNow = async () => {
    setObserveLoading(true);
    try {
      const result = await LocalObserveApi.refresh({ include_estimates: true });
      setObserveResult(result);
      toast.success(
        `Observed ${result.exact_usage_events} exact usage event(s), ${result.estimated_usage_events} labeled fallback event(s).`,
      );
      await fetchUsage(false);
    } catch (error) {
      console.error(error);
      toast.error(
        error instanceof Error ? error.message : "Local observe refresh failed",
      );
    } finally {
      setObserveLoading(false);
    }
  };

  useEffect(() => {
    fetchUsage();
  }, [range]);

  useEffect(() => {
    const source = new EventSource(UsageApi.streamUrl());
    const refresh = () => fetchUsage(false);
    source.addEventListener("open", () => setLive(true));
    source.addEventListener("error", () => setLive(false));
    source.addEventListener("ai_usage_event", refresh);
    source.addEventListener("ai_budget_alert", refresh);
    const timer = window.setInterval(refresh, 10_000);
    return () => {
      source.close();
      window.clearInterval(timer);
    };
  }, [range]);

  const totals = summary?.totals;
  const currency = summary?.currency || "USD";
  const topAgent = summary?.by_agent?.[0];
  const topProvider = summary?.by_provider?.[0];
  const topModel = summary?.by_model?.[0];
  const visibleUsageEvents = useMemo(
    () =>
      usageEvents.filter((event) => {
        if (!usageEventMatches(event, eventSearch)) return false;
        if (eventFilter === "exact") return !eventIsEstimated(event);
        if (eventFilter === "estimated") return eventIsEstimated(event);
        if (eventFilter === "pending") {
          return (
            !event.cloud_sync_status || event.cloud_sync_status === "pending"
          );
        }
        return true;
      }),
    [eventFilter, eventSearch, usageEvents],
  );
  const selectedUsageEvent =
    visibleUsageEvents.find((event) => event.event_id === selectedEventId) ??
    visibleUsageEvents[0] ??
    null;
  const agentAttributionRows = useMemo(
    () => buildAgentAttribution(usageEvents, agentLabels),
    [agentLabels, usageEvents],
  );

  const budgetStatus = useMemo(() => {
    const statuses = summary?.by_agent
      ?.map((row) => row.budget?.status)
      .filter(Boolean);
    if (statuses?.includes("hard_exceeded")) return "hard_exceeded";
    if (statuses?.includes("soft_exceeded")) return "soft_exceeded";
    return "ok";
  }, [summary]);

  const tokenBreakdown = [
    { label: "Input", value: totals?.input_tokens ?? 0 },
    { label: "Output", value: totals?.output_tokens ?? 0 },
    { label: "Cached", value: totals?.cached_input_tokens ?? 0 },
    { label: "Reasoning", value: totals?.reasoning_output_tokens ?? 0 },
    { label: "Tool", value: totals?.tool_tokens ?? 0 },
    { label: "Multimodal", value: totals?.multimodal_tokens ?? 0 },
  ];

  return (
    <div className="space-y-5">
      <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
        <div>
          <h2 className="text-2xl font-bold tracking-tight text-foreground">
            AI Usage & Cost
          </h2>
          <div className="mt-2 flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
            <span
              className={`inline-flex items-center gap-1 rounded-full px-2 py-1 ${
                live ? "bg-emerald-500/10 text-emerald-600" : "bg-muted"
              }`}
            >
              <Wifi className="h-3.5 w-3.5" />
              {live ? "Live" : "Polling"}
            </span>
            <span>
              {summary?.from ? new Date(summary.from).toLocaleString() : ""}
            </span>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <div className="inline-flex rounded-lg border bg-background p-1">
            {ranges.map((item) => (
              <button
                key={item.key}
                onClick={() => setRange(item.key)}
                className={`rounded-md px-3 py-1.5 text-sm ${
                  range === item.key
                    ? "bg-primary text-primary-foreground"
                    : "text-muted-foreground hover:text-foreground"
                }`}
              >
                {item.label}
              </button>
            ))}
          </div>
          <button
            onClick={observeNow}
            disabled={observeLoading}
            className="inline-flex h-10 items-center justify-center rounded-lg border bg-background px-3 text-sm font-medium transition-colors hover:bg-muted disabled:pointer-events-none disabled:opacity-50"
          >
            <Search
              className={`mr-2 h-4 w-4 ${observeLoading ? "animate-pulse" : ""}`}
            />
            Observe Now
          </button>
          <button
            onClick={() => fetchUsage()}
            disabled={loading}
            className="inline-flex h-10 items-center justify-center rounded-lg bg-primary px-3 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
          >
            <RefreshCw
              className={`mr-2 h-4 w-4 ${loading ? "animate-spin" : ""}`}
            />
            Refresh
          </button>
        </div>
      </div>

      {/* Compact inline metric strip */}
      <div className="flex flex-wrap items-center gap-3 rounded-lg border border-border/60 bg-card/30 px-4 py-2.5">
        <span className="inline-flex items-center gap-1.5 text-sm">
          <CircleDollarSign className="h-3.5 w-3.5 text-muted-foreground" />
          <span className="font-semibold tabular-nums">
            {money(totals?.total_cost ?? 0, currency)}
          </span>
          <span className="text-xs text-muted-foreground">spend</span>
          <span className="text-[10px] text-muted-foreground/70">
            ({number(totals?.request_count ?? 0)} calls)
          </span>
        </span>
        <span className="h-4 w-px bg-border/60" />
        <span className="inline-flex items-center gap-1.5 text-sm">
          <Zap className="h-3.5 w-3.5 text-muted-foreground" />
          <span className="font-semibold tabular-nums">
            {number(totals?.total_tokens ?? 0)}
          </span>
          <span className="text-xs text-muted-foreground">tokens</span>
          <span className="text-[10px] text-muted-foreground/70">
            ({number(totals?.cached_input_tokens ?? 0)} cached)
          </span>
        </span>
        <span className="h-4 w-px bg-border/60" />
        <span className="inline-flex items-center gap-1.5 text-sm">
          <Gauge className="h-3.5 w-3.5 text-muted-foreground" />
          <span className={`font-semibold ${statusClass(budgetStatus)}`}>
            {budgetStatus === "ok" ? "OK" : budgetStatus.replace("_", " ")}
          </span>
          <span className="text-xs text-muted-foreground">budget</span>
        </span>
        <span className="h-4 w-px bg-border/60" />
        <span className="inline-flex items-center gap-1.5 text-sm">
          <Server className="h-3.5 w-3.5 text-muted-foreground" />
          <span
            className={`font-semibold ${statusClass(syncCounts.failed ? "hard_exceeded" : syncCounts.pending ? "soft_exceeded" : "ok")}`}
          >
            {syncCounts.acked}/
            {Object.values(syncCounts).reduce((a, b) => a + b, 0)}
          </span>
          <span className="text-xs text-muted-foreground">synced</span>
        </span>
      </div>

      <UsageProvenancePanel
        evidence={usageEvidence}
        observeResult={observeResult}
        onImprove={() => setAccuracyDialogKind("local_usage_log_path")}
      />

      <ObserveAccuracyPanel
        compact
        forceDialogKind={accuracyDialogKind}
        onForceDialogClosed={() => setAccuracyDialogKind(null)}
        onChanged={() => void fetchUsage(false)}
      />

      <UsageEventLedger
        events={visibleUsageEvents}
        selected={selectedUsageEvent}
        selectedId={selectedUsageEvent?.event_id ?? null}
        onSelect={setSelectedEventId}
        agentLabels={agentLabels}
        currency={currency}
        filter={eventFilter}
        onFilter={setEventFilter}
        search={eventSearch}
        onSearch={setEventSearch}
        onSetup={() => setAccuracyDialogKind("local_usage_log_path")}
      />

      {showTechnicalDetails && (
        <div className="grid gap-3 xl:grid-cols-[1.15fr_0.85fr]">
          <section className="glass rounded-lg p-5">
            <div className="mb-4 flex items-center justify-between">
              <h3 className="font-semibold">Token Classes</h3>
              <Clock className="h-4 w-4 text-muted-foreground" />
            </div>
            <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {tokenBreakdown.map((item) => (
                <div key={item.label} className="rounded-lg border p-3">
                  <div className="text-xs text-muted-foreground">
                    {item.label}
                  </div>
                  <div className="mt-1 text-lg font-semibold tabular-nums">
                    {number(item.value)}
                  </div>
                </div>
              ))}
            </div>
          </section>

          <section className="glass rounded-lg p-5">
            <div className="mb-4 flex items-center justify-between">
              <h3 className="font-semibold">Top Usage</h3>
              <Activity className="h-4 w-4 text-muted-foreground" />
            </div>
            <div className="space-y-3">
              <TopRow
                label="Agent"
                value={agentName(topAgent?.key, agentLabels)}
                cost={topAgent?.total_cost}
                currency={currency}
              />
              <TopRow
                label="Provider"
                value={topProvider?.label || "-"}
                cost={topProvider?.total_cost}
                currency={currency}
              />
              <TopRow
                label="Model"
                value={topModel?.label || "-"}
                cost={topModel?.total_cost}
                currency={currency}
              />
            </div>
          </section>
        </div>
      )}

      <AgentFirstAttributionSection
        rows={agentAttributionRows}
        summaryRows={summary?.by_agent ?? []}
        agentLabels={agentLabels}
        currency={currency}
      />

      {showTechnicalDetails && (
        <div className="grid gap-3 xl:grid-cols-2">
          <BreakdownTable
            title="Providers"
            icon={Server}
            rows={summary?.by_provider ?? []}
            currency={currency}
          />
          <BreakdownTable
            title="Models"
            icon={ShieldAlert}
            rows={summary?.by_model ?? []}
            currency={currency}
          />
        </div>
      )}
    </div>
  );
}

// MetricCard removed - replaced by compact inline metric strip above

function UsageProvenancePanel({
  evidence,
  observeResult,
  onImprove,
}: {
  evidence: UsageEvidence;
  observeResult: LocalObserveRefreshResponse | null;
  onImprove: () => void;
}) {
  const exact = observeResult?.exact_usage_events ?? evidence.exact;
  const estimated = observeResult?.estimated_usage_events ?? evidence.estimated;
  const qualities = observeResult?.capture_quality?.length
    ? observeResult.capture_quality
    : evidence.captureQuality;
  const limitations = observeResult?.limitations ?? [];
  const nextSteps = observeResult?.next_steps ?? [];

  return (
    <section className="glass rounded-lg p-5">
      <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <ShieldCheck className="h-4 w-4 text-emerald-500" />
            <h3 className="font-semibold">How reliable are these numbers?</h3>
          </div>
          <p className="mt-1 max-w-3xl text-sm text-muted-foreground">
            Pollek uses exact provider, wrapper, proxy, or local log usage when
            it is available. If an AI app only leaves browser or surface
            metadata, Pollek labels the number as an estimate instead of
            pretending it is exact.
          </p>
        </div>
        <button
          type="button"
          onClick={onImprove}
          className="inline-flex h-9 items-center justify-center rounded-md border bg-background px-3 text-sm font-medium hover:bg-muted"
        >
          Improve exact tracking
        </button>
      </div>
      <div className="mt-4 grid gap-3 md:grid-cols-4">
        <div className="rounded-lg border p-3">
          <div className="text-xs text-muted-foreground">Exact events</div>
          <div className="mt-1 text-xl font-semibold tabular-nums">
            {number(exact)}
          </div>
        </div>
        <div className="rounded-lg border p-3">
          <div className="text-xs text-muted-foreground">Estimated events</div>
          <div className="mt-1 text-xl font-semibold tabular-nums">
            {number(estimated)}
          </div>
        </div>
        <div className="rounded-lg border p-3 md:col-span-2">
          <div className="text-xs text-muted-foreground">Capture quality</div>
          <div className="mt-2 flex flex-wrap gap-1.5">
            {qualities.length ? (
              qualities.map((quality) => (
                <span
                  key={quality}
                  className="rounded-md border bg-background px-2 py-0.5 text-[11px]"
                >
                  {quality.replace(/_/g, " ")}
                </span>
              ))
            ) : (
              <span className="text-sm text-muted-foreground">
                Waiting for AI app, browser, wrapper, proxy, or local log
                telemetry.
              </span>
            )}
          </div>
        </div>
      </div>
      {(limitations.length > 0 || nextSteps.length > 0) && (
        <div className="mt-4 grid gap-3 lg:grid-cols-2">
          {limitations.length > 0 && (
            <div className="rounded-lg border p-3 text-sm text-muted-foreground">
              <div className="mb-1 font-medium text-foreground">
                Current limits
              </div>
              {limitations.slice(0, 3).join(" ")}
            </div>
          )}
          {nextSteps.length > 0 && (
            <div className="rounded-lg border p-3 text-sm text-muted-foreground">
              <div className="mb-1 font-medium text-foreground">Next setup</div>
              {nextSteps
                .slice(0, 2)
                .map((step) => step.title)
                .join(", ")}
            </div>
          )}
        </div>
      )}
    </section>
  );
}

function UsageEventLedger({
  events,
  selected,
  selectedId,
  onSelect,
  agentLabels,
  currency,
  filter,
  onFilter,
  search,
  onSearch,
  onSetup,
}: {
  events: AiUsageEvent[];
  selected: AiUsageEvent | null;
  selectedId: string | null;
  onSelect: (eventId: string) => void;
  agentLabels: Map<string, AgentLabel>;
  currency: string;
  filter: UsageEventFilter;
  onFilter: (filter: UsageEventFilter) => void;
  search: string;
  onSearch: (query: string) => void;
  onSetup: () => void;
}) {
  const filters: Array<{ id: UsageEventFilter; label: string }> = [
    { id: "all", label: "All" },
    { id: "exact", label: "Exact" },
    { id: "estimated", label: "Estimated" },
    { id: "pending", label: "Local only" },
  ];

  return (
    <section className="glass rounded-lg p-5">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        <div>
          <div className="flex items-center gap-2">
            <FileText className="h-4 w-4 text-primary" />
            <h3 className="font-semibold">Usage event ledger</h3>
          </div>
          <p className="mt-1 max-w-3xl text-sm text-muted-foreground">
            Inspect individual model calls, tool usage, estimates, and sync
            state. This page stores usage metadata, not prompts or responses.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <div className="inline-flex h-9 overflow-hidden rounded-md border bg-background">
            {filters.map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => onFilter(item.id)}
                className={`px-3 text-sm hover:bg-muted ${
                  filter === item.id ? "bg-muted text-foreground" : ""
                }`}
              >
                {item.label}
              </button>
            ))}
          </div>
          <label className="flex h-9 min-w-[220px] items-center gap-2 rounded-md border bg-background px-3 text-sm">
            <Search className="h-4 w-4 text-muted-foreground" />
            <input
              aria-label="Search usage events"
              value={search}
              onChange={(event) => onSearch(event.target.value)}
              placeholder="Search provider, model, app..."
              className="min-w-0 flex-1 bg-transparent outline-none placeholder:text-muted-foreground"
            />
          </label>
        </div>
      </div>

      {events.length === 0 ? (
        <UsageEventEmptyState onSetup={onSetup} />
      ) : (
        <div className="mt-4 grid min-h-[420px] gap-4 xl:grid-cols-[340px_minmax(0,1fr)_320px]">
          <div className="space-y-2 overflow-y-auto pr-1 xl:max-h-[560px]">
            {events.map((event, index) => {
              const estimated = eventIsEstimated(event);
              const active = event.event_id === selectedId;
              return (
                <button
                  key={`${event.event_id}-${index}`}
                  type="button"
                  onClick={() => onSelect(event.event_id)}
                  className={`w-full rounded-lg border p-3 text-left transition hover:border-primary/40 hover:bg-primary/5 ${
                    active ? "border-primary/60 bg-primary/10" : "bg-card/60"
                  }`}
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <div className="truncate text-sm font-semibold">
                        {usageEventTitle(event)}
                      </div>
                      <p className="mt-1 truncate text-xs text-muted-foreground">
                        {agentName(event.agent_id, agentLabels)} /{" "}
                        {event.surface || "local"}
                      </p>
                    </div>
                    <span
                      className={`shrink-0 rounded-full px-2 py-0.5 text-[11px] ${
                        estimated
                          ? "bg-amber-500/10 text-amber-700"
                          : "bg-emerald-500/10 text-emerald-700"
                      }`}
                    >
                      {estimated ? "Estimated" : "Exact"}
                    </span>
                  </div>
                  <div className="mt-3 flex flex-wrap gap-2 text-xs text-muted-foreground">
                    <span>
                      {number(event.tokens?.total_tokens ?? 0)} tokens
                    </span>
                    <span>{money(event.cost?.total_cost ?? 0, currency)}</span>
                    <span>{syncLabel(event.cloud_sync_status)}</span>
                  </div>
                </button>
              );
            })}
          </div>

          <div className="rounded-lg border bg-card/60">
            {selected ? (
              <UsageEventDetail
                event={selected}
                agentLabels={agentLabels}
                currency={currency}
              />
            ) : (
              <div className="p-5 text-sm text-muted-foreground">
                Select a usage event to inspect details.
              </div>
            )}
          </div>

          <aside className="space-y-3">
            <section className="rounded-lg border bg-card/60 p-4">
              <h4 className="flex items-center gap-2 text-sm font-semibold">
                <Info className="h-4 w-4 text-primary" />
                What to look for
              </h4>
              <ul className="mt-3 space-y-2 text-xs leading-5 text-muted-foreground">
                <li>
                  Exact means provider, wrapper, or proxy usage was attached.
                </li>
                <li>
                  Estimated means Pollek had metadata but not exact provider
                  usage. Treat it as directional.
                </li>
                <li>
                  Local only means the local ledger works without Pollek Cloud.
                  Cloud sync is optional.
                </li>
              </ul>
            </section>
            <section className="rounded-lg border bg-card/60 p-4">
              <h4 className="flex items-center gap-2 text-sm font-semibold">
                <ListFilter className="h-4 w-4 text-primary" />
                Better exact data
              </h4>
              <p className="mt-2 text-xs leading-5 text-muted-foreground">
                Connect a provider usage source, wrapper, proxy, or plugin when
                you need exact tokens for browser-only AI apps.
              </p>
              <button
                type="button"
                onClick={onSetup}
                className="mt-3 inline-flex h-9 items-center rounded-md border bg-background px-3 text-sm hover:bg-muted"
              >
                Open setup
              </button>
            </section>
          </aside>
        </div>
      )}
    </section>
  );
}

function UsageEventDetail({
  event,
  agentLabels,
  currency,
}: {
  event: AiUsageEvent;
  agentLabels: Map<string, AgentLabel>;
  currency: string;
}) {
  const estimated = eventIsEstimated(event);
  const metadata = event.metadata as Record<string, unknown>;

  return (
    <div className="divide-y">
      <div className="p-5">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            <p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
              Usage event
            </p>
            <h4 className="mt-1 break-words text-lg font-semibold">
              {usageEventTitle(event)}
            </h4>
            <p className="mt-1 text-sm text-muted-foreground">
              {new Date(event.occurred_at).toLocaleString()}
            </p>
          </div>
          <span
            className={`rounded-full px-2.5 py-1 text-xs font-medium ${
              estimated
                ? "bg-amber-500/10 text-amber-700"
                : "bg-emerald-500/10 text-emerald-700"
            }`}
          >
            {estimated ? "Estimated usage" : "Exact usage"}
          </span>
        </div>
      </div>

      <div className="grid gap-3 p-5 md:grid-cols-2">
        <UsageField
          label="AI app"
          value={agentName(event.agent_id, agentLabels)}
        />
        <UsageField label="Provider" value={event.provider || "Not recorded"} />
        <UsageField label="Model" value={event.model || "Not recorded"} />
        <UsageField label="Surface" value={event.surface || "Local"} />
        <UsageField label="Result" value={event.status || "Recorded"} />
        <UsageField
          label="Cloud sync"
          value={syncLabel(event.cloud_sync_status)}
        />
      </div>

      <div className="grid gap-3 p-5 md:grid-cols-3">
        <UsageMetric
          label="Total tokens"
          value={number(event.tokens?.total_tokens ?? 0)}
        />
        <UsageMetric
          label="Input / output"
          value={`${number(event.tokens?.input_tokens ?? 0)} / ${number(
            event.tokens?.output_tokens ?? 0,
          )}`}
        />
        <UsageMetric
          label="Cost"
          value={money(
            event.cost?.total_cost ?? 0,
            event.cost?.currency || currency,
          )}
        />
      </div>

      <div className="space-y-3 p-5">
        <div>
          <h5 className="text-sm font-semibold">Why this label?</h5>
          <p className="mt-1 text-sm leading-6 text-muted-foreground">
            {usageEstimateReason(event)}
          </p>
        </div>
        <div className="rounded-lg border bg-background/60 p-3 text-xs leading-5 text-muted-foreground">
          <div className="font-medium text-foreground">Privacy note</div>
          Usage events keep metadata such as provider, model, token classes,
          cost, timing, sync state, and ids. Pollek does not show raw prompts or
          completions in this ledger.
        </div>
      </div>

      <div className="grid gap-3 p-5 md:grid-cols-2">
        <UsageField
          label="Token source"
          value={String(event.tokens?.source ?? "unknown").replace(/_/g, " ")}
        />
        <UsageField
          label="Cost source"
          value={String(event.cost?.cost_source ?? "unknown").replace(
            /_/g,
            " ",
          )}
        />
        <UsageField
          label="Provider request"
          value={event.provider_request_id || "Not recorded"}
        />
        <UsageField
          label="Idempotency key"
          value={event.idempotency_key}
          mono
        />
        <UsageField label="Trace" value={event.trace_id} mono />
        <UsageField
          label="Capture quality"
          value={
            typeof metadata.capture_quality === "string"
              ? metadata.capture_quality.replace(/_/g, " ")
              : "Not recorded"
          }
        />
      </div>
    </div>
  );
}

function UsageField({
  label,
  value,
  mono = false,
}: {
  label: string;
  value?: string;
  mono?: boolean;
}) {
  return (
    <div className="min-w-0 rounded-lg border bg-background/60 p-3">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div
        className={`mt-1 break-words text-sm font-medium ${
          mono ? "font-mono text-xs" : ""
        }`}
      >
        {value || "-"}
      </div>
    </div>
  );
}

function UsageMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border bg-background/60 p-3">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-1 text-lg font-semibold tabular-nums">{value}</div>
    </div>
  );
}

function UsageEventEmptyState({ onSetup }: { onSetup: () => void }) {
  return (
    <div className="mt-4 rounded-lg border border-dashed bg-background/40 p-5">
      <h4 className="text-sm font-semibold">No usage events match this view</h4>
      <p className="mt-2 max-w-3xl text-sm leading-6 text-muted-foreground">
        For ChatGPT, Claude, Codex, DeepSeek, Manus AI, and Antigravity, exact
        tokens usually require provider telemetry, a wrapper/proxy, local logs,
        or a browser/plugin connector. Browser-only observation may only produce
        estimated usage, and no usage appears until an AI app emits activity.
      </p>
      <button
        type="button"
        onClick={onSetup}
        className="mt-4 inline-flex h-9 items-center rounded-md border bg-background px-3 text-sm hover:bg-muted"
      >
        Check setup
      </button>
    </div>
  );
}

function AgentFirstAttributionSection({
  rows,
  summaryRows,
  agentLabels,
  currency,
}: {
  rows: AgentAttributionRow[];
  summaryRows: NonNullable<AiUsageSummary["by_agent"]>;
  agentLabels: Map<string, AgentLabel>;
  currency: string;
}) {
  return (
    <section className="glass rounded-lg p-5">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <div className="flex items-center gap-2">
            <Bot className="h-4 w-4 text-primary" />
            <h3 className="font-semibold">Agent-first cost & tokens</h3>
          </div>
          <p className="mt-1 max-w-3xl text-sm leading-6 text-muted-foreground">
            Pollek separates usage by AI app, runtime, or agent first. Provider,
            model, and shared-credit totals are secondary rollups so ChatGPT in
            a browser, Codex in a terminal, Claude Code, Antigravity, and other
            tools do not collapse into one model bill too early.
          </p>
        </div>
        <div className="rounded-lg border bg-background/60 px-3 py-2 text-xs leading-5 text-muted-foreground">
          Same model, different app: separate first. Same provider credit pool:
          reconcile second.
        </div>
      </div>

      {rows.length ? (
        <div className="mt-4 grid gap-3 xl:grid-cols-2">
          {rows.map((row) => (
            <article
              key={row.agentKey}
              className="rounded-lg border bg-card/60 p-4"
            >
              <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                <div className="min-w-0">
                  <div className="flex flex-wrap items-center gap-2">
                    <h4 className="break-words text-base font-semibold">
                      {row.agentName}
                    </h4>
                    <span className="rounded-full border bg-background px-2 py-0.5 text-[11px] text-muted-foreground">
                      {row.agentType || "AI app"}
                    </span>
                  </div>
                  <p className="mt-1 break-words text-xs text-muted-foreground">
                    {row.agentKey}
                  </p>
                </div>
                <RegisterControlBar agentId={row.agentKey} tenantId="local" />
              </div>

              <div className="mt-4 grid gap-2 sm:grid-cols-4">
                <UsageMetric label="Calls" value={number(row.calls)} />
                <UsageMetric label="Tokens" value={number(row.tokens)} />
                <UsageMetric label="Cost" value={money(row.cost, currency)} />
                <UsageMetric
                  label="Exact / estimated"
                  value={`${number(row.exact)} / ${number(row.estimated)}`}
                />
              </div>

              <div className="mt-3 flex flex-wrap gap-1.5">
                {row.surfaces.length ? (
                  row.surfaces.map((surface) => (
                    <span
                      key={surface}
                      className="rounded-md border bg-background px-2 py-0.5 text-[11px] text-muted-foreground"
                    >
                      {surface.replace(/_/g, " ")}
                    </span>
                  ))
                ) : (
                  <span className="text-xs text-muted-foreground">
                    No surface metadata yet
                  </span>
                )}
              </div>

              {row.sharedPoolCount > 0 && (
                <div className="mt-3 flex gap-2 rounded-lg border border-amber-500/25 bg-amber-500/10 p-3 text-xs leading-5 text-amber-800 dark:text-amber-200">
                  <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
                  <span>
                    {row.sharedPoolCount} provider/model pool(s) are shared by
                    more than one app. Pollek keeps this app separate here, but
                    the provider invoice or credit balance may still be pooled.
                  </span>
                </div>
              )}

              <div className="mt-4 space-y-2">
                <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                  Provider/model pools
                </div>
                {row.pools.map((pool) => (
                  <div
                    key={pool.poolKey}
                    className="rounded-lg border bg-background/60 p-3"
                  >
                    <div className="flex flex-col gap-2 md:flex-row md:items-start md:justify-between">
                      <div className="min-w-0">
                        <div className="break-words text-sm font-medium">
                          {poolDisplayName(pool)}
                        </div>
                        <p className="mt-1 text-xs leading-5 text-muted-foreground">
                          {providerBillingHint(pool.provider)}
                        </p>
                      </div>
                      <div className="flex shrink-0 flex-wrap gap-2 text-xs text-muted-foreground">
                        <span>{number(pool.calls)} calls</span>
                        <span>{number(pool.tokens)} tokens</span>
                        <span>{money(pool.cost, currency)}</span>
                      </div>
                    </div>
                    <div className="mt-2 flex flex-wrap gap-2 text-[11px] text-muted-foreground">
                      <span>{number(pool.exact)} exact</span>
                      <span>{number(pool.estimated)} estimated</span>
                      <span>{pool.poolKey.replace(/:/g, " / ")}</span>
                    </div>
                  </div>
                ))}
              </div>
            </article>
          ))}
        </div>
      ) : summaryRows.length ? (
        <div className="mt-4 rounded-lg border border-amber-500/25 bg-amber-500/10 p-4">
          <div className="flex gap-2">
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-amber-700" />
            <div>
              <h4 className="text-sm font-semibold">
                Event-level provider/model split is not available yet
              </h4>
              <p className="mt-1 text-sm leading-6 text-muted-foreground">
                Pollek has agent summary totals, but no per-event provider/model
                ledger in this range. Connect a provider usage source, wrapper,
                proxy, or local usage log to make this section exact.
              </p>
            </div>
          </div>
          <div className="mt-3 grid gap-2 md:grid-cols-2 xl:grid-cols-3">
            {summaryRows.map((row) => (
              <div
                key={row.key}
                className="rounded-lg border bg-background/70 p-3 text-sm"
              >
                <div className="font-medium">
                  {agentName(row.key, agentLabels)}
                </div>
                <div className="mt-2 flex flex-wrap gap-2 text-xs text-muted-foreground">
                  <span>{number(row.total_tokens)} tokens</span>
                  <span>{money(row.total_cost, currency)}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      ) : (
        <EmptyState />
      )}
    </section>
  );
}

function TopRow({
  label,
  value,
  cost,
  currency,
}: {
  label: string;
  value: string;
  cost?: number;
  currency: string;
}) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-lg border p-3">
      <div className="min-w-0">
        <div className="text-xs text-muted-foreground">{label}</div>
        <div className="truncate font-medium">{value}</div>
      </div>
      <div className="tabular-nums text-muted-foreground">
        {money(cost ?? 0, currency)}
      </div>
    </div>
  );
}

function BreakdownTable({
  title,
  icon: Icon,
  rows,
  currency,
}: {
  title: string;
  icon: typeof Server;
  rows: NonNullable<AiUsageSummary["by_provider"]>;
  currency: string;
}) {
  return (
    <section className="glass rounded-lg p-5">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="font-semibold">{title}</h3>
        <Icon className="h-4 w-4 text-muted-foreground" />
      </div>
      <div className="space-y-2">
        {rows.length ? (
          rows.map((row, index) => (
            <div
              key={`${row.key}-${index}`}
              className="grid grid-cols-[1fr_auto_auto] items-center gap-3 rounded-lg border p-3 text-sm"
            >
              <span className="min-w-0 truncate font-medium">{row.label}</span>
              <span className="tabular-nums text-muted-foreground">
                {number(row.total_tokens)}
              </span>
              <span className="tabular-nums text-muted-foreground">
                {money(row.total_cost, currency)}
              </span>
            </div>
          ))
        ) : (
          <EmptyState compact />
        )}
      </div>
    </section>
  );
}

function EmptyState({ compact = false }: { compact?: boolean }) {
  return (
    <div
      className={`flex items-center justify-center rounded-lg border border-dashed text-sm text-muted-foreground ${
        compact ? "h-24" : "h-40"
      }`}
    >
      No usage events yet.
    </div>
  );
}

function agentName(
  agentId: string | undefined,
  labels: Map<string, AgentLabel>,
) {
  if (!agentId) return "-";
  return labels.get(agentId)?.name || agentId;
}
