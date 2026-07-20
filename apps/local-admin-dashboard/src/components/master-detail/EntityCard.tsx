import { useId, useMemo, useState } from "react";
import type { MouseEvent, ReactNode } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import { cn } from "@/lib/utils";
import { formatDisplayValue, renderDisplayValue } from "@/lib/displayValue";
import { statusToken, type UiStatus } from "../../lib/status";

export function EntityCard({
  title,
  subtitle,
  summary,
  icon: Icon,
  status,
  statusLabel,
  headerBadges,
  meta = [],
  actions = [],
  visual,
  selected,
  expandable = true,
  defaultExpanded = false,
  collapsedMetaCount = 4,
  className,
}: {
  title: ReactNode;
  subtitle?: ReactNode;
  summary?: ReactNode;
  icon: any;
  status: UiStatus;
  statusLabel: string;
  headerBadges?: ReactNode;
  meta?: { label: string; value: ReactNode }[];
  actions?: {
    label: string;
    icon?: any;
    primary?: boolean;
    danger?: boolean;
    disabled?: boolean;
    onClick: (event: MouseEvent<HTMLButtonElement>) => void;
  }[];
  visual?: ReactNode;
  selected: boolean;
  expandable?: boolean;
  defaultExpanded?: boolean;
  collapsedMetaCount?: number;
  className?: string;
}) {
  const s = statusToken(status);
  const [expanded, setExpanded] = useState(defaultExpanded);
  const detailsId = useId();
  const summaryText = summary === undefined ? "" : formatDisplayValue(summary);
  const hasLongSummary = summaryText.length > 140;
  const hasExtraMeta = meta.length > collapsedMetaCount;
  const canExpand = expandable && (hasLongSummary || hasExtraMeta);
  const visibleMeta = useMemo(
    () => (expanded || !canExpand ? meta : meta.slice(0, collapsedMetaCount)),
    [canExpand, collapsedMetaCount, expanded, meta],
  );

  return (
    <div
      className={cn(
        "rounded-xl border border-border bg-card/60 p-4 backdrop-blur-sm transition-all duration-200",
        "hover:border-primary/40 hover:bg-card hover:shadow-sm",
        selected &&
          "ring-1 ring-primary/50 border-primary/50 bg-card shadow-md",
        className,
      )}
    >
      <div className="flex items-start gap-3">
        {visual ?? (
          <div className={cn("mt-0.5 rounded-lg p-2", s.bg)}>
            <Icon className={cn("h-4 w-4", s.text)} />
          </div>
        )}
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-2">
            <span className="truncate font-medium">
              {renderDisplayValue(title)}
            </span>
            <span
              className={cn(
                "flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[11px] font-medium transition-colors",
                s.bg,
                s.text,
              )}
            >
              <span className={cn("h-1.5 w-1.5 rounded-full", s.dot)} />
              {renderDisplayValue(statusLabel)}
            </span>
          </div>
          {subtitle && (
            <div className="truncate text-xs text-muted-foreground mt-0.5">
              {renderDisplayValue(subtitle)}
            </div>
          )}
          {headerBadges && <div className="mt-2">{headerBadges}</div>}
          <div id={detailsId}>
            {summary && (
              <p
                className={cn(
                  "mt-2 text-xs leading-5 text-muted-foreground",
                  !expanded && "line-clamp-2",
                )}
              >
                {renderDisplayValue(summary)}
              </p>
            )}
            {visibleMeta.length > 0 && (
              <div className="mt-3 flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-muted-foreground">
                {visibleMeta.map((m, idx) => (
                  <span key={idx} className="flex items-center gap-1">
                    {m.label}:{" "}
                    <span className="text-foreground/80 font-medium">
                      {renderDisplayValue(m.value)}
                    </span>
                  </span>
                ))}
                {!expanded && hasExtraMeta && (
                  <span className="text-muted-foreground">
                    +{meta.length - visibleMeta.length} more
                  </span>
                )}
              </div>
            )}
          </div>
          {canExpand && (
            <button
              type="button"
              aria-expanded={expanded}
              aria-controls={detailsId}
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
          {actions.length > 0 && (
            <div className="mt-3 flex flex-wrap gap-2">
              {actions.map((action, idx) => {
                const ActionIcon = action.icon;
                return (
                  <button
                    key={idx}
                    type="button"
                    disabled={action.disabled}
                    onKeyDown={(event) => event.stopPropagation()}
                    onClick={(event) => {
                      event.stopPropagation();
                      action.onClick(event);
                    }}
                    className={cn(
                      "inline-flex h-8 items-center justify-center whitespace-nowrap gap-1.5 rounded-md px-3 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
                      action.primary
                        ? "bg-primary text-primary-foreground hover:bg-primary/90"
                        : action.danger
                          ? "border border-red-500/20 bg-red-500/10 text-red-600 hover:bg-red-500/20"
                          : "border border-input bg-background hover:bg-accent hover:text-accent-foreground",
                      action.disabled && "cursor-not-allowed opacity-50",
                    )}
                  >
                    {ActionIcon && <ActionIcon className="h-3.5 w-3.5" />}
                    {action.label}
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
