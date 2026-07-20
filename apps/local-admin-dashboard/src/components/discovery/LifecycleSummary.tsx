import type { ComponentType } from "react";
import {
  Activity,
  HardDriveDownload,
  LayoutGrid,
  ShieldCheck,
  Trash2,
  CircleDashed,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type {
  LifecycleCounts,
  LifecycleFilter,
} from "../../lib/agentLifecycle";
import { LiveDot } from "./AgentLifecycleBadge";

interface SegmentSpec {
  filter: LifecycleFilter;
  label: string;
  count: number;
  icon: ComponentType<{ className?: string }>;
  accent: string; // text + value color
  ring: string; // active ring color
  live?: boolean;
}

/**
 * Premium lifecycle summary: a segmented strip of clickable stats that doubles
 * as the primary filter for the agent list. Shows, at a glance, how many agents
 * are running right now, installed, governed, awaiting review, or removed.
 */
export function LifecycleSummary({
  counts,
  active,
  onSelect,
  className,
}: {
  counts: LifecycleCounts;
  active: LifecycleFilter;
  onSelect: (filter: LifecycleFilter) => void;
  className?: string;
}) {
  const segments: SegmentSpec[] = [
    {
      filter: "all",
      label: "All agents",
      count: counts.total,
      icon: LayoutGrid,
      accent: "text-foreground",
      ring: "ring-primary/50",
    },
    {
      filter: "running",
      label: "Running now",
      count: counts.running,
      icon: Activity,
      accent: "text-emerald-300",
      ring: "ring-emerald-500/50",
      live: counts.running > 0,
    },
    {
      filter: "installed",
      label: "Installed",
      count: counts.installed + counts.running,
      icon: HardDriveDownload,
      accent: "text-sky-300",
      ring: "ring-sky-500/50",
    },
    {
      filter: "registered",
      label: "Governed",
      count: counts.registered,
      icon: ShieldCheck,
      accent: "text-violet-300",
      ring: "ring-violet-500/50",
    },
    {
      filter: "needs_review",
      label: "Needs review",
      count: counts.needsReview,
      icon: CircleDashed,
      accent: "text-amber-300",
      ring: "ring-amber-500/50",
    },
    {
      filter: "removed",
      label: "Removed",
      count: counts.uninstalled + counts.dormant,
      icon: Trash2,
      accent: "text-rose-300",
      ring: "ring-rose-500/50",
    },
  ];

  return (
    <div
      className={cn(
        "grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-6",
        className,
      )}
    >
      {segments.map((seg) => {
        const isActive = active === seg.filter;
        const Icon = seg.icon;
        return (
          <button
            key={seg.filter}
            type="button"
            aria-pressed={isActive}
            onClick={() => onSelect(seg.filter)}
            className={cn(
              "group relative flex flex-col gap-1 rounded-xl border bg-card/50 p-3 text-left backdrop-blur-sm transition-all",
              "hover:border-primary/40 hover:bg-card",
              isActive
                ? cn("border-transparent bg-card shadow-sm ring-1", seg.ring)
                : "border-border",
            )}
          >
            <div className="flex items-center gap-1.5 text-[11px] font-medium text-muted-foreground">
              {seg.live ? (
                <LiveDot />
              ) : (
                <Icon className={cn("h-3.5 w-3.5", seg.accent)} />
              )}
              <span className="truncate">{seg.label}</span>
            </div>
            <div
              className={cn("text-2xl font-semibold tabular-nums", seg.accent)}
            >
              {seg.count}
            </div>
          </button>
        );
      })}
    </div>
  );
}
