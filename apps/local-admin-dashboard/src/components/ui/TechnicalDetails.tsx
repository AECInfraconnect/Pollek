import React, { useId, useState } from "react";
import { ChevronDown, SlidersHorizontal } from "lucide-react";
import { cn } from "../../lib/utils";

export interface TechnicalDetailsProps {
  /** Short, plain trigger label. Defaults to "Technical details". */
  label?: string;
  /** Optional one-line hint shown under the label when collapsed. */
  hint?: string;
  /** Optional small count/badge (e.g. number of hidden items). */
  count?: number | string;
  children: React.ReactNode;
  defaultOpen?: boolean;
  className?: string;
  contentClassName?: string;
  id?: string;
}

/**
 * A single, consistent progressive-disclosure surface for technical / advanced
 * content. The clean summary above stays free of jargon; anything technical —
 * raw fields, coverage matrices, capture-quality notes, scan ids — lives behind
 * one calm "Technical details" toggle. Reuse this everywhere instead of
 * scattering bespoke collapsibles so the whole product discloses depth the same
 * way.
 */
export function TechnicalDetails({
  label = "Technical details",
  hint,
  count,
  children,
  defaultOpen = false,
  className,
  contentClassName,
  id,
}: TechnicalDetailsProps) {
  const [open, setOpen] = useState(defaultOpen);
  const generatedId = useId();
  const contentId = id ?? `techdetails-${generatedId}`;
  const buttonId = `${contentId}-button`;

  return (
    <div
      className={cn(
        "rounded-xl border border-border/70 bg-muted/20",
        className,
      )}
      data-state={open ? "open" : "closed"}
    >
      <button
        id={buttonId}
        type="button"
        aria-expanded={open}
        aria-controls={contentId}
        onClick={() => setOpen((value) => !value)}
        className="group flex w-full items-center gap-3 rounded-xl px-4 py-3 text-left transition-colors hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-1"
      >
        <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground">
          <SlidersHorizontal aria-hidden="true" className="h-3.5 w-3.5" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="flex items-center gap-2 text-sm font-medium">
            {label}
            {count !== undefined && count !== "" && (
              <span className="rounded-full bg-muted px-1.5 py-0.5 text-[11px] font-medium text-muted-foreground">
                {count}
              </span>
            )}
          </span>
          {hint && (
            <span className="mt-0.5 block truncate text-xs text-muted-foreground">
              {hint}
            </span>
          )}
        </span>
        <ChevronDown
          aria-hidden="true"
          className={cn(
            "h-4 w-4 shrink-0 text-muted-foreground transition-transform duration-200",
            open && "rotate-180",
          )}
        />
      </button>
      <div
        id={contentId}
        role="region"
        aria-labelledby={buttonId}
        hidden={!open}
        className={cn("border-t border-border/70 px-4 py-4", contentClassName)}
      >
        {children}
      </div>
    </div>
  );
}
