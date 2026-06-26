import { RefreshCw, Search } from "lucide-react";

export interface TimelineFilters {
  search: string;
  decision: string;
  mode: string;
  entityType: string;
  entityId: string;
}

export function ActivityFilters({
  value,
  loading,
  onChange,
  onRefresh,
}: {
  value: TimelineFilters;
  loading: boolean;
  onChange: (next: TimelineFilters) => void;
  onRefresh: () => void;
}) {
  const patch = (next: Partial<TimelineFilters>) =>
    onChange({ ...value, ...next });

  return (
    <div className="rounded-lg border bg-card/50 p-4">
      <div className="grid gap-3 lg:grid-cols-[1.2fr_0.75fr_0.75fr_0.75fr_1fr_auto]">
        <label className="relative block">
          <span className="sr-only">Search activity</span>
          <Search className="absolute left-3 top-2.5 h-4 w-4 text-muted-foreground" />
          <input
            value={value.search}
            onChange={(event) => patch({ search: event.target.value })}
            placeholder="Search actor, tool, resource, policy, trace..."
            className="h-9 w-full rounded-md border bg-background pl-9 pr-3 text-sm"
          />
        </label>

        <label className="block">
          <span className="sr-only">Decision</span>
          <select
            value={value.decision}
            onChange={(event) => patch({ decision: event.target.value })}
            className="h-9 w-full rounded-md border bg-background px-3 text-sm"
          >
            <option value="">All decisions</option>
            <option value="allow">Allow</option>
            <option value="deny">Deny</option>
            <option value="ok">OK</option>
            <option value="error">Error</option>
            <option value="observe">Observe</option>
          </select>
        </label>

        <label className="block">
          <span className="sr-only">Mode</span>
          <select
            value={value.mode}
            onChange={(event) => patch({ mode: event.target.value })}
            className="h-9 w-full rounded-md border bg-background px-3 text-sm"
          >
            <option value="">All modes</option>
            <option value="observe">Observe</option>
            <option value="enforce">Enforce</option>
            <option value="warn">Warn</option>
            <option value="approval">Approval</option>
          </select>
        </label>

        <label className="block">
          <span className="sr-only">Entity type</span>
          <select
            value={value.entityType}
            onChange={(event) => patch({ entityType: event.target.value })}
            className="h-9 w-full rounded-md border bg-background px-3 text-sm"
          >
            <option value="">Any entity</option>
            <option value="agent">Agent</option>
            <option value="tool">Tool</option>
            <option value="resource">Resource</option>
            <option value="policy">Policy</option>
            <option value="identity">Identity</option>
          </select>
        </label>

        <label className="block">
          <span className="sr-only">Entity ID</span>
          <input
            value={value.entityId}
            onChange={(event) => patch({ entityId: event.target.value })}
            placeholder="Entity ID"
            className="h-9 w-full rounded-md border bg-background px-3 text-sm"
          />
        </label>

        <button
          type="button"
          onClick={onRefresh}
          className="inline-flex h-9 items-center justify-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
        >
          <RefreshCw className={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />
          Refresh
        </button>
      </div>
    </div>
  );
}
