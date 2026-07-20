import type { ComponentType } from "react";
import {
  Activity,
  CheckCircle2,
  CircleDashed,
  HardDriveDownload,
  PauseCircle,
  Trash2,
  ShieldCheck,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type {
  AgentLifecycle,
  LifecycleBadgeSpec,
  LifecycleTone,
} from "../../lib/agentLifecycle";

// Tone -> Tailwind token set. Kept local to the badge so the lifecycle model
// stays presentation-free.
const TONE_TOKENS: Record<
  LifecycleTone,
  { text: string; bg: string; ring: string; dot: string }
> = {
  live: {
    text: "text-emerald-300",
    bg: "bg-emerald-500/10",
    ring: "ring-emerald-500/30",
    dot: "bg-emerald-400",
  },
  present: {
    text: "text-sky-300",
    bg: "bg-sky-500/10",
    ring: "ring-sky-500/30",
    dot: "bg-sky-400",
  },
  governed: {
    text: "text-violet-300",
    bg: "bg-violet-500/10",
    ring: "ring-violet-500/30",
    dot: "bg-violet-400",
  },
  review: {
    text: "text-amber-300",
    bg: "bg-amber-500/10",
    ring: "ring-amber-500/30",
    dot: "bg-amber-400",
  },
  muted: {
    text: "text-zinc-400",
    bg: "bg-zinc-500/10",
    ring: "ring-zinc-500/20",
    dot: "bg-zinc-500",
  },
  gone: {
    text: "text-rose-300",
    bg: "bg-rose-500/10",
    ring: "ring-rose-500/30",
    dot: "bg-rose-400",
  },
};

const BADGE_ICONS: Record<string, ComponentType<{ className?: string }>> = {
  running: Activity,
  installed: HardDriveDownload,
  dormant: PauseCircle,
  uninstalled: Trash2,
  unknown: CircleDashed,
  registered: ShieldCheck,
  pending: CircleDashed,
  new: CircleDashed,
  ignored: CircleDashed,
  merged: CheckCircle2,
  retired: Trash2,
};

/** A pulsing dot used to signal a live, running agent. */
export function LiveDot({ className }: { className?: string }) {
  return (
    <span className={cn("relative flex h-2 w-2", className)}>
      <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-75" />
      <span className="relative inline-flex h-2 w-2 rounded-full bg-emerald-400" />
    </span>
  );
}

export function AgentLifecycleBadge({
  badge,
  size = "sm",
  showIcon = true,
  className,
}: {
  badge: LifecycleBadgeSpec;
  size?: "sm" | "md";
  showIcon?: boolean;
  className?: string;
}) {
  const tone = TONE_TOKENS[badge.tone];
  const Icon = BADGE_ICONS[badge.key];
  return (
    <span
      title={badge.description}
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full font-medium ring-1 ring-inset transition-colors",
        size === "sm" ? "px-2 py-0.5 text-[11px]" : "px-2.5 py-1 text-xs",
        tone.text,
        tone.bg,
        tone.ring,
        className,
      )}
    >
      {badge.live ? (
        <LiveDot />
      ) : showIcon && Icon ? (
        <Icon className={cn(size === "sm" ? "h-3 w-3" : "h-3.5 w-3.5")} />
      ) : (
        <span className={cn("h-1.5 w-1.5 rounded-full", tone.dot)} />
      )}
      {badge.label}
    </span>
  );
}

/**
 * Compact stacked view: presence + governance, side by side. Used on cards and
 * detail headers so an operator sees both "is it running" and "is it governed".
 */
export function AgentLifecycleBadges({
  lifecycle,
  size = "sm",
  className,
}: {
  lifecycle: AgentLifecycle;
  size?: "sm" | "md";
  className?: string;
}) {
  const showGovernance =
    lifecycle.governanceBadge.key !== "new" || lifecycle.presence !== "running";
  return (
    <div className={cn("flex flex-wrap items-center gap-1.5", className)}>
      <AgentLifecycleBadge badge={lifecycle.presenceBadge} size={size} />
      {showGovernance && (
        <AgentLifecycleBadge badge={lifecycle.governanceBadge} size={size} />
      )}
    </div>
  );
}
