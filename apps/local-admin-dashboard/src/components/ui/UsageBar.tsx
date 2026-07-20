import type { ReactNode } from "react";
import { cn } from "../../lib/utils";

export interface UsageSegment {
  /** Stable key for React + colour lookup. */
  id: string;
  /** Plain, friendly label shown in the legend (e.g. "Input", "Output"). */
  label: string;
  /** Raw magnitude for this segment. */
  value: number;
  /** Tailwind background class for the filled segment. */
  className: string;
  /** Optional preformatted value (defaults to a locale integer). */
  formatted?: string;
}

export interface UsageBarProps {
  segments: UsageSegment[];
  /**
   * Optional shared maximum so several bars in a list stay proportional to
   * each other instead of each filling its own row. When omitted the bar fills
   * its track based on its own segment total.
   */
  max?: number;
  /** Show the value legend under the bar. Defaults to true. */
  showLegend?: boolean;
  /** Height of the track. Defaults to a comfortable 10px. */
  height?: "sm" | "md" | "lg";
  /** Accessible description of what the bar represents. */
  ariaLabel?: string;
  className?: string;
  /** Optional trailing node rendered on the legend row (e.g. a total). */
  legendEnd?: ReactNode;
}

const HEIGHTS: Record<NonNullable<UsageBarProps["height"]>, string> = {
  sm: "h-1.5",
  md: "h-2.5",
  lg: "h-3.5",
};

function formatValue(value: number) {
  return new Intl.NumberFormat().format(Math.round(value || 0));
}

/**
 * A single proportional usage bar with labelled segments. It is the shared
 * visual primitive behind every "how much did this use" surface — token
 * input/output splits, cost, and credit — so the whole product reads the same
 * way. Keep the bar itself jargon-free; anything technical belongs in a
 * TechnicalDetails panel next to it, not inside the bar.
 */
export function UsageBar({
  segments,
  max,
  showLegend = true,
  height = "md",
  ariaLabel,
  className,
  legendEnd,
}: UsageBarProps) {
  const total = segments.reduce((sum, segment) => sum + Math.max(segment.value, 0), 0);
  // The track is filled relative to the shared max (for side-by-side rows) or
  // to this bar's own total when no shared scale is provided.
  const scale = max && max > 0 ? max : total;
  const filledFraction = scale > 0 ? Math.min(total / scale, 1) : 0;

  return (
    <div className={cn("w-full", className)}>
      <div
        role="img"
        aria-label={
          ariaLabel ??
          segments.map((s) => `${s.label}: ${s.formatted ?? formatValue(s.value)}`).join(", ")
        }
        className={cn(
          "flex w-full overflow-hidden rounded-full bg-muted",
          HEIGHTS[height],
        )}
      >
        {total > 0 ? (
          <div className="flex h-full" style={{ width: `${filledFraction * 100}%` }}>
            {segments.map((segment) => {
              const width = total > 0 ? (Math.max(segment.value, 0) / total) * 100 : 0;
              if (width <= 0) return null;
              return (
                <div
                  key={segment.id}
                  className={cn("h-full", segment.className)}
                  style={{ width: `${width}%` }}
                  title={`${segment.label}: ${segment.formatted ?? formatValue(segment.value)}`}
                />
              );
            })}
          </div>
        ) : null}
      </div>

      {showLegend && (
        <div className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
          {segments.map((segment) => (
            <span key={segment.id} className="inline-flex items-center gap-1.5">
              <span className={cn("h-2 w-2 shrink-0 rounded-full", segment.className)} />
              <span>{segment.label}</span>
              <span className="font-medium tabular-nums text-foreground">
                {segment.formatted ?? formatValue(segment.value)}
              </span>
            </span>
          ))}
          {legendEnd ? <span className="ml-auto">{legendEnd}</span> : null}
        </div>
      )}
    </div>
  );
}

/** Shared segment colours so token bars look identical everywhere. */
export const TOKEN_SEGMENT_COLORS = {
  input: "bg-sky-500",
  output: "bg-violet-500",
  cached: "bg-emerald-500",
  reasoning: "bg-amber-500",
} as const;
