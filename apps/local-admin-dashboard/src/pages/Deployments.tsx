import { useState } from "react";
import {
  CheckCircle2,
  Clock3,
  RotateCcw,
  ServerCog,
  ShieldCheck,
  SlidersHorizontal,
} from "lucide-react";
import { EntityCard } from "../components/master-detail/EntityCard";
import { MasterDetailLayout } from "../components/master-detail/MasterDetailLayout";
import { DetailPane } from "../components/master-detail/DetailPane";
import type { UiStatus } from "../lib/status";

interface DeploymentRecord {
  id: string;
  target: string;
  status: "active" | "needs_approval";
  level: "Enforce" | "Approval";
  policy: string;
  device: string;
  bundle: string;
  date: string;
}

const deployments: DeploymentRecord[] = [
  {
    id: "dep-1",
    target: "claude_desktop",
    status: "active",
    level: "Enforce",
    policy: "Protect workspace source files",
    device: "Local device",
    bundle: "bundle-local-1",
    date: new Date().toISOString(),
  },
  {
    id: "dep-2",
    target: "cursor",
    status: "needs_approval",
    level: "Approval",
    policy: "Ask before terminal access",
    device: "Local device",
    bundle: "draft-session",
    date: new Date().toISOString(),
  },
];

function deploymentStatus(record: DeploymentRecord): {
  status: UiStatus;
  label: string;
} {
  if (record.status === "active") {
    return { status: "ok", label: "Active" };
  }
  return { status: "degraded", label: "Needs approval" };
}

function DeploymentDetail({ record }: { record: DeploymentRecord }) {
  const status = deploymentStatus(record);
  return (
    <div className="grid gap-4 xl:grid-cols-[280px_minmax(0,1fr)_300px]">
      <aside className="rounded-lg border bg-card/50 p-4">
        <h3 className="mb-3 flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
          <span className="h-1.5 w-1.5 rounded-full bg-primary" />
          Record Summary
        </h3>
        <div className="space-y-3 text-sm">
          <SummaryRow label="Deployment ID" value={record.id} />
          <SummaryRow label="Target AI app" value={record.target} />
          <SummaryRow label="Policy" value={record.policy} />
          <SummaryRow label="Device" value={record.device} />
          <SummaryRow label="Bundle" value={record.bundle} />
          <SummaryRow label="Requested level" value={record.level} />
        </div>
      </aside>

      <DetailPane
        title={`Deployment to ${record.target}`}
        subtitle={`${record.policy} on ${record.device}`}
        status={status.status}
        statusLabel={status.label}
        actions={[
          ...(record.status === "needs_approval"
            ? [
                {
                  label: "Approve",
                  primary: true,
                  icon: CheckCircle2,
                  onClick: () => undefined,
                },
              ]
            : []),
          {
            label: "Rollback",
            icon: RotateCcw,
            onClick: () => undefined,
          },
        ]}
        tabs={[
          {
            id: "overview",
            label: "Overview",
            content: (
              <div className="grid gap-3 md:grid-cols-2">
                <SummaryMetric
                  label="What is happening"
                  value={
                    record.status === "active"
                      ? "Policy is active for this target."
                      : "Policy is waiting for approval before activation."
                  }
                />
                <SummaryMetric
                  label="User impact"
                  value={
                    record.level === "Enforce"
                      ? "Matching activity can be controlled on this device."
                      : "Pollek will ask before changing access."
                  }
                />
                <SummaryMetric label="Target" value={record.target} />
                <SummaryMetric label="Last updated" value={formatDate(record.date)} />
              </div>
            ),
          },
          {
            id: "history",
            label: "History",
            content: (
              <div className="space-y-3">
                <TimelineItem
                  icon={ServerCog}
                  title="Deployment record created"
                  detail={formatDate(record.date)}
                />
                <TimelineItem
                  icon={ShieldCheck}
                  title={
                    record.status === "active"
                      ? "Policy active on local device"
                      : "Approval is required"
                  }
                  detail={record.policy}
                />
              </div>
            ),
          },
          {
            id: "technical",
            label: "Technical Details",
            content: (
              <div className="grid gap-3 text-sm md:grid-cols-2">
                <SummaryRow label="Bundle ID" value={record.bundle} />
                <SummaryRow label="Deployment ID" value={record.id} />
                <SummaryRow label="Target key" value={record.target} />
                <SummaryRow label="Mode" value={record.level} />
              </div>
            ),
          },
        ]}
      />

      <aside className="space-y-3 rounded-lg border bg-card/50 p-4">
        <h3 className="text-sm font-semibold">Related Records</h3>
        <RelatedRecord icon={ShieldCheck} title="Policy" detail={record.policy} />
        <RelatedRecord icon={ServerCog} title="Device" detail={record.device} />
        <RelatedRecord icon={Clock3} title="Last update" detail={formatDate(record.date)} />
      </aside>
    </div>
  );
}

function SummaryRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid gap-1 border-b border-border/40 pb-2 last:border-0">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className="break-words font-medium">{value}</span>
    </div>
  );
}

function SummaryMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border bg-background/40 p-4">
      <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
        {label}
      </div>
      <div className="mt-2 break-words text-sm font-semibold">{value}</div>
    </div>
  );
}

function TimelineItem({
  icon: Icon,
  title,
  detail,
}: {
  icon: typeof ServerCog;
  title: string;
  detail: string;
}) {
  return (
    <div className="flex gap-3 rounded-lg border bg-background/40 p-3">
      <div className="rounded-lg bg-primary/10 p-2 text-primary">
        <Icon className="h-4 w-4" />
      </div>
      <div>
        <div className="text-sm font-medium">{title}</div>
        <div className="text-xs text-muted-foreground">{detail}</div>
      </div>
    </div>
  );
}

function RelatedRecord({
  icon: Icon,
  title,
  detail,
}: {
  icon: typeof ShieldCheck;
  title: string;
  detail: string;
}) {
  return (
    <div className="flex gap-3 rounded-lg border bg-background/40 p-3">
      <Icon className="mt-0.5 h-4 w-4 text-primary" />
      <div className="min-w-0">
        <div className="text-sm font-medium">{title}</div>
        <div className="truncate text-xs text-muted-foreground">{detail}</div>
      </div>
    </div>
  );
}

function formatDate(value: string) {
  return new Date(value).toLocaleString();
}

export function Deployments() {
  const [selectedId, setSelectedId] = useState<string | undefined>();

  return (
    <div className="space-y-4">
      <div>
        <h1 className="text-2xl font-bold">Deployments</h1>
        <p className="text-sm text-muted-foreground">
          Track active and pending policy deployments across local devices and AI
          apps.
        </p>
      </div>

      <MasterDetailLayout
        items={deployments}
        selectedId={selectedId}
        onSelect={(id) => setSelectedId(id || undefined)}
        idSelector={(record) => record.id}
        masterLayout="grid"
        masterListClassName="grid gap-4 md:grid-cols-2 xl:grid-cols-3"
        detailBackLabel="Back to all deployments"
        renderCard={(record, selected) => {
          const status = deploymentStatus(record);
          return (
            <EntityCard
              title={`Deployment to ${record.target}`}
              subtitle={record.policy}
              summary={`Requested level: ${record.level}. Bundle: ${record.bundle}.`}
              icon={SlidersHorizontal}
              status={status.status}
              statusLabel={status.label}
              meta={[
                { label: "Target", value: record.target },
                { label: "Device", value: record.device },
                { label: "Updated", value: formatDate(record.date) },
              ]}
              selected={selected}
            />
          );
        }}
        renderDetail={(record) => <DeploymentDetail record={record} />}
      />
    </div>
  );
}
