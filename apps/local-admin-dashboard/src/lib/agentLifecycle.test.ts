import { describe, expect, it } from "vitest";

import {
  buildLifecycleContext,
  deriveAgentLifecycle,
  matchesLifecycleFilter,
  summarizeLifecycles,
  type LifecycleContext,
} from "./agentLifecycle";
import type {
  DiscoveredAgentCandidateV2,
  DiscoveryScanJob,
} from "../services/types";

const NOW = new Date("2026-07-20T12:00:00Z").getTime();

function candidate(
  over: Partial<DiscoveredAgentCandidateV2> & {
    sources?: string[];
    seenAt?: string;
    scanId?: string;
  } = {},
): DiscoveredAgentCandidateV2 {
  const { sources = [], seenAt, scanId, ...rest } = over;
  const seen = seenAt ?? new Date(NOW).toISOString();
  return {
    schema_version: "3",
    candidate_id: rest.candidate_id ?? "c1",
    tenant_id: "t",
    device_id: "d",
    status: rest.status ?? "discovered",
    canonical_service_id: "svc",
    surface_group_id: "grp",
    authority_boundary: "local_device",
    entity_role: "local_agent_host",
    duplicate_policy: "standalone",
    observe_scope: "",
    enforce_scope: "",
    related_surfaces: [],
    display_name: rest.display_name ?? "Agent",
    inferred_agent_type: "cli_agent",
    confidence: 0.9,
    risk_score: 10,
    first_seen: seen,
    last_seen: seen,
    scan_ids: scanId ? [scanId] : [],
    last_scan_id: scanId,
    evidence: sources.map((source, i) => ({
      evidence_id: `e${i}`,
      source,
      confidence: 0.9,
      observed_at: seen,
      privacy_class: "public_metadata",
      redacted: false,
      data: {},
    })),
    discovered_configs: [],
    discovered_endpoints: [],
    discovered_mcp_servers: [],
    suggested_registration: {},
    suggested_observation_profile: {},
    suggested_control_bindings: [],
    telemetry_plan: {},
    labels: {},
    ...rest,
  } as DiscoveredAgentCandidateV2;
}

const freshCtx: LifecycleContext = {
  latestScanId: "scan-1",
  latestScanAt: NOW - 60 * 1000, // 1 min ago
  now: NOW,
};

describe("deriveAgentLifecycle", () => {
  it("marks a live process in the latest fresh scan as running + live", () => {
    const lc = deriveAgentLifecycle(
      candidate({ sources: ["process_scan"], scanId: "scan-1" }),
      freshCtx,
    );
    expect(lc.presence).toBe("running");
    expect(lc.isLive).toBe(true);
    expect(lc.primaryBadge.key).toBe("running");
    expect(lc.presenceBadge.live).toBe(true);
  });

  it("running but stale scan is not shown as live", () => {
    const lc = deriveAgentLifecycle(
      candidate({ sources: ["process_scan"], scanId: "scan-1" }),
      { latestScanId: "scan-1", latestScanAt: NOW - 60 * 60 * 1000, now: NOW },
    );
    expect(lc.presence).toBe("running");
    expect(lc.isLive).toBe(false);
    expect(lc.presenceBadge.live).toBe(false);
  });

  it("install-only evidence in the latest scan is installed, not running", () => {
    const lc = deriveAgentLifecycle(
      candidate({ sources: ["installed_app_scan"], scanId: "scan-1" }),
      freshCtx,
    );
    expect(lc.presence).toBe("installed");
    expect(lc.isLive).toBe(false);
  });

  it("absent from newest scan but recent => dormant/stopped", () => {
    const lc = deriveAgentLifecycle(
      candidate({
        sources: ["process_scan"],
        scanId: "scan-0",
        seenAt: new Date(NOW - 2 * 60 * 60 * 1000).toISOString(),
      }),
      freshCtx,
    );
    expect(lc.presence).toBe("dormant");
    expect(lc.primaryBadge.label).toBe("Stopped");
  });

  it("absent from newest scan and stale => uninstalled/removed", () => {
    const lc = deriveAgentLifecycle(
      candidate({
        sources: ["installed_app_scan"],
        scanId: "scan-0",
        seenAt: new Date(NOW - 5 * 24 * 60 * 60 * 1000).toISOString(),
      }),
      freshCtx,
    );
    expect(lc.presence).toBe("uninstalled");
  });

  it("retired status is always uninstalled", () => {
    const lc = deriveAgentLifecycle(
      candidate({
        status: "retired",
        sources: ["process_scan"],
        scanId: "scan-1",
      }),
      freshCtx,
    );
    expect(lc.presence).toBe("uninstalled");
  });

  it("registered running agent keeps a running primary but a governed badge", () => {
    const lc = deriveAgentLifecycle(
      candidate({
        status: "registered",
        sources: ["process_scan"],
        scanId: "scan-1",
      }),
      freshCtx,
    );
    expect(lc.presence).toBe("running");
    expect(lc.governance).toBe("registered");
    expect(lc.governanceBadge.key).toBe("registered");
  });

  it("registered but not running surfaces registered as the primary badge", () => {
    const lc = deriveAgentLifecycle(
      candidate({
        status: "registered",
        sources: ["installed_app_scan"],
        scanId: "scan-1",
      }),
      freshCtx,
    );
    expect(lc.presence).toBe("installed");
    expect(lc.primaryBadge.key).toBe("registered");
  });

  it("pending_approval maps to needs-review governance", () => {
    const lc = deriveAgentLifecycle(
      candidate({ status: "pending_approval", scanId: "scan-1" }),
      freshCtx,
    );
    expect(lc.governance).toBe("pending");
    expect(lc.governanceBadge.label).toBe("Needs review");
  });
});

describe("buildLifecycleContext", () => {
  it("uses the first scan as latest and reads its finish time", () => {
    const scans: DiscoveryScanJob[] = [
      {
        scan_id: "scan-1",
        tenant_id: "t",
        status: "completed",
        finished_at: new Date(NOW - 30 * 1000).toISOString(),
        sources: [],
        candidates_found: 3,
      } as DiscoveryScanJob,
    ];
    const ctx = buildLifecycleContext(scans, null, NOW);
    expect(ctx.latestScanId).toBe("scan-1");
    expect(ctx.latestScanAt).toBe(NOW - 30 * 1000);
  });
});

describe("summarizeLifecycles + filters", () => {
  it("aggregates counts across presence and governance", () => {
    const cands = [
      candidate({
        candidate_id: "a",
        sources: ["process_scan"],
        scanId: "scan-1",
      }),
      candidate({
        candidate_id: "b",
        sources: ["installed_app_scan"],
        scanId: "scan-1",
        status: "registered",
      }),
      candidate({
        candidate_id: "c",
        sources: ["installed_app_scan"],
        scanId: "scan-0",
        seenAt: new Date(NOW - 10 * 24 * 60 * 60 * 1000).toISOString(),
      }),
    ];
    const counts = summarizeLifecycles(cands, freshCtx);
    expect(counts.total).toBe(3);
    expect(counts.running).toBe(1);
    expect(counts.installed).toBe(1);
    expect(counts.uninstalled).toBe(1);
    expect(counts.registered).toBe(1);
  });

  it("filter chips select the right subset", () => {
    const running = deriveAgentLifecycle(
      candidate({ sources: ["process_scan"], scanId: "scan-1" }),
      freshCtx,
    );
    expect(matchesLifecycleFilter(running, "running")).toBe(true);
    expect(matchesLifecycleFilter(running, "installed")).toBe(true);
    expect(matchesLifecycleFilter(running, "removed")).toBe(false);
    expect(matchesLifecycleFilter(running, "all")).toBe(true);
  });
});
