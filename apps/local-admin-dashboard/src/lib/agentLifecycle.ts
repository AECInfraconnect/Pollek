// Agent lifecycle model.
//
// Discovery candidates carry a governance `status` (registered / pending / …)
// and a pile of recency + evidence fields. Operators, though, think about their
// machine in physical terms: which AI agents are *running right now*, which are
// merely *installed*, which *used to be here but are gone*, and which are under
// Pollek governance. This module derives that human-facing lifecycle from the
// raw candidate so every surface can render the same, consistent picture.
//
// It is intentionally pure and dependency-free so it can be unit-tested and
// reused across the Discovery page, the agent list, and detail views.

import type {
  DiscoveredAgentCandidateV2,
  DiscoveryScanJob,
} from "../services/types";

/** Physical presence of the agent on this machine, right now. */
export type AgentPresence =
  | "running" // a live process was observed in the most recent scan
  | "installed" // present on disk (install marker / extension) but not running
  | "dormant" // seen in a recent scan but absent from the newest one
  | "uninstalled" // gone: absent from the newest scan and stale, or retired
  | "unknown";

/** Where the agent sits in the Pollek governance workflow. */
export type AgentGovernance =
  | "registered"
  | "pending"
  | "new"
  | "ignored"
  | "merged"
  | "retired";

export type LifecycleTone =
  | "live"
  | "present"
  | "governed"
  | "review"
  | "muted"
  | "gone";

export interface LifecycleBadgeSpec {
  key: string;
  label: string;
  tone: LifecycleTone;
  /** Render a pulsing "live" dot (used for the running-now indicator). */
  live?: boolean;
  description: string;
}

export interface AgentLifecycle {
  presence: AgentPresence;
  governance: AgentGovernance;
  /** True only when running AND the latest scan is fresh enough to trust. */
  isLive: boolean;
  presenceBadge: LifecycleBadgeSpec;
  governanceBadge: LifecycleBadgeSpec;
  /** The single most informative badge for compact rows. */
  primaryBadge: LifecycleBadgeSpec;
  /** e.g. "2 minutes ago" — when this agent was last seen. */
  lastSeenLabel: string;
  /** e.g. "as of 3m ago" — freshness of the scan that produced the presence. */
  asOfLabel: string;
}

export interface LifecycleContext {
  latestScanId?: string;
  /** Epoch ms the latest scan finished (or started), for freshness. */
  latestScanAt?: number;
  now?: number;
}

// Evidence sources that mean "a process was actually observed running", vs.
// sources that only prove the agent is installed on disk.
const LIVE_PROCESS_SOURCES = new Set([
  "process_scan",
  "cli_agent",
  "port_probe",
  "local_model_server",
  "python_framework",
  "container",
  "browser_session",
  "browser_window",
  "network_egress",
  "network_sni",
  "token_usage",
]);

const INSTALL_ONLY_SOURCES = new Set([
  "installed_app_scan",
  "ide_extension",
  "browser_extension",
  "mcp_config",
  "browser_history",
  "user_confirmation",
]);

// A scan newer than this is "live" enough to show a pulsing running indicator.
const LIVE_FRESHNESS_MS = 15 * 60 * 1000; // 15 minutes
// Absent from the newest scan but seen within this window => "dormant" (likely
// just stopped/closed), rather than fully "uninstalled".
const DORMANT_WINDOW_MS = 24 * 60 * 60 * 1000; // 24 hours

function scanIdsForCandidate(candidate: DiscoveredAgentCandidateV2): string[] {
  const ids = new Set<string>();
  if (candidate.last_scan_id) ids.add(candidate.last_scan_id);
  for (const id of candidate.scan_ids ?? []) {
    if (id) ids.add(id);
  }
  for (const evidence of candidate.evidence ?? []) {
    const id = (evidence?.data as { scan_id?: unknown } | undefined)?.scan_id;
    if (typeof id === "string" && id.length > 0) ids.add(id);
  }
  return Array.from(ids);
}

function hasLiveProcessEvidence(
  candidate: DiscoveredAgentCandidateV2,
): boolean {
  return (candidate.evidence ?? []).some((e) =>
    LIVE_PROCESS_SOURCES.has(e.source),
  );
}

function hasInstallEvidence(candidate: DiscoveredAgentCandidateV2): boolean {
  return (candidate.evidence ?? []).some(
    (e) =>
      INSTALL_ONLY_SOURCES.has(e.source) || LIVE_PROCESS_SOURCES.has(e.source),
  );
}

/** Build a lifecycle context from the scan list + any in-flight scan job. */
export function buildLifecycleContext(
  scans: DiscoveryScanJob[],
  scanJob?: DiscoveryScanJob | null,
  now: number = Date.now(),
): LifecycleContext {
  const latest = scans[0] ?? scanJob ?? undefined;
  const at = latest?.finished_at || latest?.started_at;
  return {
    latestScanId: latest?.scan_id,
    latestScanAt: at ? new Date(at).getTime() : undefined,
    now,
  };
}

const PRESENCE_BADGES: Record<AgentPresence, LifecycleBadgeSpec> = {
  running: {
    key: "running",
    label: "Running",
    tone: "live",
    live: true,
    description: "A live process for this agent was seen in the latest scan.",
  },
  installed: {
    key: "installed",
    label: "Installed",
    tone: "present",
    description:
      "Installed on this machine but not running at the time of the latest scan.",
  },
  dormant: {
    key: "dormant",
    label: "Stopped",
    tone: "muted",
    description:
      "Seen in a recent scan but absent from the latest one — likely closed or stopped.",
  },
  uninstalled: {
    key: "uninstalled",
    label: "Removed",
    tone: "gone",
    description:
      "Previously present but no longer detected — the agent appears to have been uninstalled.",
  },
  unknown: {
    key: "unknown",
    label: "Unknown",
    tone: "muted",
    description:
      "Not enough signal to determine whether this agent is present.",
  },
};

const GOVERNANCE_BADGES: Record<AgentGovernance, LifecycleBadgeSpec> = {
  registered: {
    key: "registered",
    label: "Registered",
    tone: "governed",
    description: "Registered with Pollek and governed by policy.",
  },
  pending: {
    key: "pending",
    label: "Needs review",
    tone: "review",
    description: "Awaiting your review before it can be governed.",
  },
  new: {
    key: "new",
    label: "New",
    tone: "review",
    description: "Newly discovered — not yet triaged.",
  },
  ignored: {
    key: "ignored",
    label: "Ignored",
    tone: "muted",
    description: "You dismissed this candidate; it is kept for reference only.",
  },
  merged: {
    key: "merged",
    label: "Merged",
    tone: "muted",
    description: "Merged into another agent as the same underlying identity.",
  },
  retired: {
    key: "retired",
    label: "Retired",
    tone: "gone",
    description: "Retired — no longer active on this machine.",
  },
};

function governanceFromStatus(status: string): AgentGovernance {
  switch (status) {
    case "registered":
      return "registered";
    case "pending_approval":
    case "unconfirmed":
      return "pending";
    case "ignored":
      return "ignored";
    case "merged":
      return "merged";
    case "retired":
      return "retired";
    default:
      return "new";
  }
}

function derivePresence(
  candidate: DiscoveredAgentCandidateV2,
  ctx: LifecycleContext,
): { presence: AgentPresence; isLive: boolean } {
  if (candidate.status === "retired") {
    return { presence: "uninstalled", isLive: false };
  }

  const now = ctx.now ?? Date.now();
  const lastSeenMs = candidate.last_seen
    ? new Date(candidate.last_seen).getTime()
    : NaN;
  const candidateScanIds = scanIdsForCandidate(candidate);
  const inLatestScan =
    ctx.latestScanId != null && candidateScanIds.includes(ctx.latestScanId);
  // If we cannot resolve scan membership at all, fall back to recency alone.
  const scanKnown = ctx.latestScanId != null && candidateScanIds.length > 0;

  const scanFresh =
    ctx.latestScanAt != null && now - ctx.latestScanAt <= LIVE_FRESHNESS_MS;

  if (!scanKnown || inLatestScan) {
    // Present in the newest scan (or scan membership unknown): decide running
    // vs merely installed from the evidence sources.
    if (hasLiveProcessEvidence(candidate)) {
      return { presence: "running", isLive: scanFresh };
    }
    if (hasInstallEvidence(candidate)) {
      return { presence: "installed", isLive: false };
    }
    // No evidence classification but present in scan — treat as installed.
    return { presence: "installed", isLive: false };
  }

  // Absent from the newest scan: dormant if recent, uninstalled if stale.
  if (!Number.isNaN(lastSeenMs) && now - lastSeenMs <= DORMANT_WINDOW_MS) {
    return { presence: "dormant", isLive: false };
  }
  return { presence: "uninstalled", isLive: false };
}

export function formatRelative(
  timestamp: string | undefined,
  now: number = Date.now(),
): string {
  if (!timestamp) return "unknown";
  const then = new Date(timestamp).getTime();
  if (Number.isNaN(then)) return "unknown";
  const delta = Math.max(0, now - then);
  const sec = Math.round(delta / 1000);
  if (sec < 45) return "just now";
  const min = Math.round(sec / 60);
  if (min < 60) return `${min} minute${min === 1 ? "" : "s"} ago`;
  const hr = Math.round(min / 60);
  if (hr < 24) return `${hr} hour${hr === 1 ? "" : "s"} ago`;
  const day = Math.round(hr / 24);
  if (day < 30) return `${day} day${day === 1 ? "" : "s"} ago`;
  const mon = Math.round(day / 30);
  if (mon < 12) return `${mon} month${mon === 1 ? "" : "s"} ago`;
  const yr = Math.round(mon / 12);
  return `${yr} year${yr === 1 ? "" : "s"} ago`;
}

function shortRelative(
  timestamp: number | undefined,
  now: number = Date.now(),
): string {
  if (timestamp == null || Number.isNaN(timestamp)) return "";
  const delta = Math.max(0, now - timestamp);
  const min = Math.round(delta / 60000);
  if (min < 1) return "just now";
  if (min < 60) return `${min}m ago`;
  const hr = Math.round(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const day = Math.round(hr / 24);
  return `${day}d ago`;
}

/**
 * Derive the full lifecycle picture for a discovery candidate. Pure: given the
 * same candidate + context it always returns the same result.
 */
export function deriveAgentLifecycle(
  candidate: DiscoveredAgentCandidateV2,
  ctx: LifecycleContext = {},
): AgentLifecycle {
  const now = ctx.now ?? Date.now();
  const { presence, isLive } = derivePresence(candidate, ctx);
  const governance = governanceFromStatus(candidate.status);

  const presenceBadge: LifecycleBadgeSpec = {
    ...PRESENCE_BADGES[presence],
    live: presence === "running" ? isLive : false,
  };
  const governanceBadge = GOVERNANCE_BADGES[governance];

  // The primary badge is the one an operator most needs at a glance: a live
  // agent's running state, then its governance state, otherwise its presence.
  let primaryBadge: LifecycleBadgeSpec;
  if (presence === "running") {
    primaryBadge = presenceBadge;
  } else if (governance === "registered") {
    primaryBadge = governanceBadge;
  } else {
    primaryBadge = presenceBadge;
  }

  const asOf = shortRelative(ctx.latestScanAt, now);
  return {
    presence,
    governance,
    isLive,
    presenceBadge,
    governanceBadge,
    primaryBadge,
    lastSeenLabel: formatRelative(candidate.last_seen, now),
    asOfLabel: asOf ? `as of ${asOf}` : "",
  };
}

export interface LifecycleCounts {
  running: number;
  installed: number;
  dormant: number;
  uninstalled: number;
  registered: number;
  needsReview: number;
  total: number;
}

/** Aggregate lifecycle counts for a set of candidates (for hero summaries). */
export function summarizeLifecycles(
  candidates: DiscoveredAgentCandidateV2[],
  ctx: LifecycleContext = {},
): LifecycleCounts {
  const counts: LifecycleCounts = {
    running: 0,
    installed: 0,
    dormant: 0,
    uninstalled: 0,
    registered: 0,
    needsReview: 0,
    total: candidates.length,
  };
  for (const candidate of candidates) {
    const lc = deriveAgentLifecycle(candidate, ctx);
    if (lc.presence === "running") counts.running += 1;
    else if (lc.presence === "installed") counts.installed += 1;
    else if (lc.presence === "dormant") counts.dormant += 1;
    else if (lc.presence === "uninstalled") counts.uninstalled += 1;
    if (lc.governance === "registered") counts.registered += 1;
    if (lc.governance === "pending" || lc.governance === "new")
      counts.needsReview += 1;
  }
  return counts;
}

export type LifecycleFilter =
  | "all"
  | "running"
  | "installed"
  | "registered"
  | "needs_review"
  | "removed";

/** Whether a candidate matches a lifecycle filter chip. */
export function matchesLifecycleFilter(
  lc: AgentLifecycle,
  filter: LifecycleFilter,
): boolean {
  switch (filter) {
    case "all":
      return true;
    case "running":
      return lc.presence === "running";
    case "installed":
      return lc.presence === "installed" || lc.presence === "running";
    case "registered":
      return lc.governance === "registered";
    case "needs_review":
      return lc.governance === "pending" || lc.governance === "new";
    case "removed":
      return lc.presence === "uninstalled" || lc.presence === "dormant";
    default:
      return true;
  }
}
