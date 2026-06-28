import { type ReactNode, useMemo, useState } from "react";
import { Activity, Bug, FileText, Network, ShieldCheck } from "lucide-react";
import { cn } from "@/lib/utils";
import { formatDisplayValue, renderDisplayValue } from "@/lib/displayValue";
import type {
  Entity360Response,
  GraphMetric,
} from "../../features/entity-graph/types";
import { EntityPageHeader, type EntityPageHeaderProps } from "./EntityPageHeader";
import { RelatedList, type RelatedListItem } from "./RelatedList";
import { ActivityFeed } from "./ActivityFeed";
import { EntityRelationshipPanel } from "../relationship/EntityRelationshipPanel";
import { ContextualHelp } from "../help/ContextualHelp";

export interface RelatedSection {
  title: string;
  icon: any;
  iconColor?: string;
  items: RelatedListItem[];
  viewAllHref?: string;
}

export interface DetailField {
  label: string;
  value: ReactNode;
  status?: "ok" | "warning" | "danger" | "info" | "unknown";
  source?: string;
  history?: string;
  confidence?: string;
  note?: string;
}

export interface DetailSection {
  title: string;
  description?: string;
  helpTopicId?: string;
  icon?: any;
  fields: DetailField[];
}

interface Entity360PageProps {
  header: EntityPageHeaderProps;
  aboutSection: ReactNode;
  relatedSections: RelatedSection[];
  data?: Entity360Response | null;
  detailSections?: DetailSection[];
  extraTabs?: Array<{
    id: string;
    label: string;
    icon?: any;
    content: ReactNode;
  }>;
}

type TabId = "activity" | "details" | "map" | "debug" | string;

const fieldStatusClass: Record<NonNullable<DetailField["status"]>, string> = {
  ok: "bg-emerald-500/10 text-emerald-500",
  warning: "bg-amber-500/10 text-amber-500",
  danger: "bg-red-500/10 text-red-500",
  info: "bg-blue-500/10 text-blue-500",
  unknown: "bg-muted text-muted-foreground",
};

function metricFields(metrics: GraphMetric[]): DetailField[] {
  return metrics.map((metric) => ({
    label: metric.label,
    value: metric.value,
    source: "entity graph metric",
  }));
}

function fallbackDetailSections(data?: Entity360Response | null): DetailSection[] {
  if (!data) {
    return [
      {
        title: "Record Details",
        description: "Live details load from the local service.",
        fields: [
          {
            label: "Status",
            value: "Waiting for entity data",
            status: "unknown",
            source: "dashboard",
          },
        ],
      },
    ];
  }

  const entity = data.entity;
  return [
    {
      title: "Current Status",
      description: "Canonical entity values from the shared entity graph.",
      icon: ShieldCheck,
      fields: [
        {
          label: "Entity ID",
          value: entity.entity_id,
          source: "entity graph",
        },
        {
          label: "Type",
          value: entity.type,
          source: "entity graph",
        },
        {
          label: "Status",
          value: entity.status || "unknown",
          status: entity.status ? "info" : "unknown",
          source: "entity graph",
        },
        ...(entity.mode
          ? [
              {
                label: "Mode",
                value: entity.mode,
                source: "policy / observation merge",
              },
            ]
          : []),
        ...(entity.risk
          ? [
              {
                label: "Risk",
                value: entity.risk,
                status: "warning" as const,
                source: "risk scorer",
              },
            ]
          : []),
        ...metricFields(entity.metrics),
      ],
    },
    {
      title: "Evidence History",
      description: "When this page was built and what warnings affected it.",
      fields: [
        {
          label: "Generated",
          value: new Date(data.generated_at).toLocaleString(),
          source: "local-control-plane",
        },
        {
          label: "Warnings",
          value: data.warnings.length,
          status: data.warnings.length ? "warning" : "ok",
          source: "entity graph resolver",
          note: data.warnings.map((warning) => warning.message).join("; "),
        },
      ],
    },
  ];
}

function DetailSectionCard({ section }: { section: DetailSection }) {
  const Icon = section.icon ?? FileText;
  const helpTopicId =
    section.helpTopicId ??
    (section.title === "Reference Intel"
      ? "entity.reference_intel"
      : section.title === "Known Capability Checklist"
        ? "entity.known_capabilities"
        : section.title.includes("Deployment")
          ? "policy.deploy"
          : undefined);
  return (
    <section className="rounded-lg border border-border/70 bg-background/40">
      <div className="border-b border-border/50 px-4 py-3">
        <div className="flex items-center gap-2">
          <Icon className="h-4 w-4 text-primary" />
          <h3 className="text-sm font-semibold">{section.title}</h3>
          <ContextualHelp topicId={helpTopicId} />
        </div>
        {section.description && (
          <p className="mt-1 text-xs leading-5 text-muted-foreground">
            {section.description}
          </p>
        )}
      </div>
      <div className="divide-y divide-border/30">
        {section.fields.map((field) => (
          <div
            key={`${section.title}-${field.label}`}
            className="grid gap-2 px-4 py-3 text-sm md:grid-cols-[180px_1fr]"
          >
            <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
              {field.label}
            </div>
            <div className="min-w-0 space-y-1">
              <div className="flex flex-wrap items-center gap-2">
                <div className="break-words font-medium text-foreground">
                  {renderDisplayValue(field.value)}
                </div>
                {field.status && (
                  <span
                    className={cn(
                      "rounded-full px-2 py-0.5 text-[10px] font-semibold uppercase",
                      fieldStatusClass[field.status],
                    )}
                  >
                    {field.status}
                  </span>
                )}
              </div>
              {(field.source || field.history || field.confidence) && (
                <div className="flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-muted-foreground">
                  {field.source && (
                    <span>
                      Source:{" "}
                      <span className="text-foreground/70">
                        {formatDisplayValue(field.source)}
                      </span>
                    </span>
                  )}
                  {field.history && (
                    <span>
                      History:{" "}
                      <span className="text-foreground/70">
                        {formatDisplayValue(field.history)}
                      </span>
                    </span>
                  )}
                  {field.confidence && (
                    <span>
                      Confidence:{" "}
                      <span className="text-foreground/70">
                        {formatDisplayValue(field.confidence)}
                      </span>
                    </span>
                  )}
                </div>
              )}
              {field.note && (
                <p className="text-xs leading-5 text-muted-foreground">
                  {formatDisplayValue(field.note)}
                </p>
              )}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function DetailsPanel({
  detailSections,
  data,
}: {
  detailSections?: DetailSection[];
  data?: Entity360Response | null;
}) {
  const sections = detailSections?.length
    ? detailSections
    : fallbackDetailSections(data);
  return (
    <div className="space-y-3">
      {sections.map((section) => (
        <DetailSectionCard key={section.title} section={section} />
      ))}
    </div>
  );
}

function DebugPanel({ data }: { data?: Entity360Response | null }) {
  if (!data) {
    return (
      <div className="rounded-lg border border-dashed border-border p-6 text-sm text-muted-foreground">
        No debug payload is available yet.
      </div>
    );
  }
  return (
    <pre className="max-h-[520px] overflow-auto rounded-lg border border-border bg-background/70 p-4 text-xs leading-5 text-muted-foreground">
      {JSON.stringify(data, null, 2)}
    </pre>
  );
}

export function Entity360Page({
  header,
  aboutSection,
  relatedSections,
  data,
  detailSections,
  extraTabs = [],
}: Entity360PageProps) {
  const [activeTab, setActiveTab] = useState<TabId>("activity");

  const defaultTabs = useMemo(
    () => [
      { id: "activity" as const, label: "Activity", icon: Activity },
      { id: "details" as const, label: "Details", icon: FileText },
      { id: "map" as const, label: "Map", icon: Network },
      { id: "debug" as const, label: "Debug", icon: Bug },
    ],
    [],
  );

  const allTabs = [
    ...defaultTabs,
    ...extraTabs.map((tab) => ({
      id: tab.id,
      label: tab.label,
      icon: tab.icon ?? FileText,
    })),
  ];

  return (
    <div className="space-y-4">
      <EntityPageHeader {...header} />

      <div className="grid gap-4 lg:grid-cols-[300px_minmax(0,1fr)_320px] md:grid-cols-[280px_minmax(0,1fr)]">
        <aside className="space-y-3">
          <section className="rounded-lg border border-border bg-card/50 p-4">
            <h3 className="mb-3 flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              <div className="h-1.5 w-1.5 rounded-full bg-primary" />
              Record Summary
            </h3>
            {aboutSection}
          </section>
        </aside>

        <section className="min-w-0 rounded-lg border border-border bg-card/50">
          <div className="border-b border-border/50 px-4 pt-3">
            <div className="mb-2 flex items-center justify-between gap-3">
              <div>
                <h2 className="text-sm font-semibold">Detail Workspace</h2>
                <p className="text-xs text-muted-foreground">
                  Status, history, relationships, and raw evidence for this
                  record.
                </p>
              </div>
            </div>
            <nav className="flex gap-0.5 overflow-x-auto" aria-label="Entity detail tabs">
              {allTabs.map((tab) => {
                const TabIcon = tab.icon;
                return (
                  <button
                    key={tab.id}
                    type="button"
                    onClick={() => setActiveTab(tab.id)}
                    className={cn(
                      "flex items-center gap-1.5 whitespace-nowrap border-b-2 px-3 py-2.5 text-xs font-medium transition-colors",
                      activeTab === tab.id
                        ? "border-primary text-primary"
                        : "border-transparent text-muted-foreground hover:text-foreground",
                    )}
                  >
                    <TabIcon className="h-3.5 w-3.5" />
                    {tab.label}
                  </button>
                );
              })}
            </nav>
          </div>

          <div className="p-4">
            {activeTab === "activity" && (
              <ActivityFeed items={data?.activity ?? []} maxVisible={20} />
            )}
            {activeTab === "details" && (
              <DetailsPanel detailSections={detailSections} data={data} />
            )}
            {activeTab === "map" &&
              (data?.graph ? (
                <EntityRelationshipPanel
                  graph={data.graph}
                  selectedNodeId={data.entity.id}
                />
              ) : (
                <div className="rounded-lg border border-dashed border-border p-6 text-sm text-muted-foreground">
                  No relationships have been observed for this entity yet.
                </div>
              ))}
            {activeTab === "debug" && <DebugPanel data={data} />}
            {extraTabs.map(
              (tab) =>
                activeTab === tab.id && <div key={tab.id}>{tab.content}</div>,
            )}
          </div>
        </section>

        <aside className="space-y-3">
          <div className="rounded-lg border border-border bg-card/50">
            <div className="border-b border-border/50 px-4 py-3">
              <h2 className="text-sm font-semibold">Related Records</h2>
              <p className="text-xs text-muted-foreground">
                Direct links from registry, telemetry, policies, and evidence.
              </p>
            </div>
            <div className="space-y-3 p-3">
              {relatedSections.map((section) => (
                <RelatedList
                  key={section.title}
                  title={section.title}
                  icon={section.icon}
                  iconColor={section.iconColor}
                  items={section.items}
                  viewAllHref={section.viewAllHref}
                />
              ))}
              {relatedSections.length === 0 && (
                <div className="rounded-lg border border-dashed border-border p-6 text-center text-xs text-muted-foreground">
                  No related records found yet.
                </div>
              )}
            </div>
          </div>
        </aside>
      </div>
    </div>
  );
}
