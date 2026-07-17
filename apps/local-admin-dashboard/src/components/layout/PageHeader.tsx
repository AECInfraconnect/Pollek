import React from "react";
import type { ComponentType } from "react";
import { cn } from "../../lib/utils";

export interface PageHeaderProps {
  /** Plain, human page title. Keep it short and jargon-free. */
  title: string;
  /** One calm sentence describing what this page is for, in plain language. */
  subtitle?: string;
  /** Optional leading icon. */
  icon?: ComponentType<{ className?: string }>;
  /** Primary/secondary actions, right-aligned. */
  actions?: React.ReactNode;
  className?: string;
}

/**
 * The one page header every screen should use. It gives the whole product a
 * consistent, premium top: a clear title, a single plain-language sentence, and
 * room for actions — never a wall of technical sub-labels. Technical depth
 * belongs lower on the page behind a `TechnicalDetails` panel, not in the
 * header.
 */
export function PageHeader({
  title,
  subtitle,
  icon: Icon,
  actions,
  className,
}: PageHeaderProps) {
  return (
    <div
      className={cn(
        "flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between",
        className,
      )}
    >
      <div className="flex items-start gap-3">
        {Icon && (
          <span className="mt-0.5 flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary">
            <Icon className="h-5 w-5" />
          </span>
        )}
        <div className="min-w-0">
          <h2 className="text-2xl font-bold tracking-tight text-foreground">
            {title}
          </h2>
          {subtitle && (
            <p className="mt-1 max-w-2xl text-sm leading-6 text-muted-foreground">
              {subtitle}
            </p>
          )}
        </div>
      </div>
      {actions && (
        <div className="flex flex-wrap items-center gap-2">{actions}</div>
      )}
    </div>
  );
}
