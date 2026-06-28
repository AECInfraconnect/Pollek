import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import {
  AlertTriangle,
  ArrowRight,
  CheckCircle2,
  Download,
  Eye,
  Globe,
  Info,
  LockKeyhole,
  Loader2,
  Radio,
  Send,
  ShieldAlert,
  ShieldCheck,
  Store,
  Wrench,
  XCircle,
} from "lucide-react";
import { toast } from "sonner";

import { useMode } from "../../context/ModeContext";
import { isAdvanceMode } from "../../lib/modes";
import { TelemetryApi } from "../../services/api";
import type { GuardEvent, GuardIncidentEnvelope } from "../../types/guard";

type FeedStatus = "loading" | "ready" | "unavailable";
type StreamStatus = "connecting" | "live" | "disconnected";

const BROWSER_CONNECTOR_OPTIONS = [
  {
    id: "chromium",
    label: "Chrome / Chromium",
    install: "Load unpacked from apps/prompt-guard-browser-extension/dist/chromium",
    packagePath:
      "apps/prompt-guard-browser-extension/packages/pollek-prompt-guard-chromium.zip",
  },
  {
    id: "edge",
    label: "Microsoft Edge",
    install: "Load unpacked from apps/prompt-guard-browser-extension/dist/edge",
    packagePath:
      "apps/prompt-guard-browser-extension/packages/pollek-prompt-guard-edge.zip",
  },
  {
    id: "safari",
    label: "Safari",
    install: "Convert dist/safari-webextension with Xcode, then enable in Safari Settings",
    packagePath:
      "apps/prompt-guard-browser-extension/packages/pollek-prompt-guard-safari-webextension.zip",
  },
];

const PROMPT_CAPTURE_ADAPTERS = [
  {
    id: "cli_hook",
    label: "CLI hook",
    bestFor: "Codex CLI, Claude Code, shell-driven agents",
    dataPath:
      "Wrap the command so prompt/output text is checked before it reaches the agent or after output is produced.",
    status: "Supported event source",
  },
  {
    id: "sdk_adapter",
    label: "SDK adapter",
    bestFor: "Custom Python, TypeScript, or app-embedded AI agents",
    dataPath:
      "Call the local Prompt Guard API from the application code before provider calls and after model responses.",
    status: "Supported event source",
  },
  {
    id: "mcp_proxy",
    label: "MCP proxy",
    bestFor: "Agents that use MCP tools or tool servers",
    dataPath:
      "Route tool prompts, tool outputs, and sensitive responses through a local MCP proxy that emits metadata-only guard events.",
    status: "Supported event source",
  },
  {
    id: "local_proxy",
    label: "Local proxy or wrapper",
    bestFor: "Desktop apps without extension or SDK support",
    dataPath:
      "Place a user-approved proxy or launch wrapper in front of the app. OS observe alone can see process/file/network metadata, not encrypted prompt bodies.",
    status: "Needs per-app setup",
  },
];

const SEVERITY_STYLE: Record<string, string> = {
  critical: "border-red-500/80 bg-red-500/10 text-red-950 dark:text-red-100",
  warn: "border-amber-500/80 bg-amber-500/10 text-amber-950 dark:text-amber-100",
  info: "border-sky-500/70 bg-sky-500/10 text-sky-950 dark:text-sky-100",
};

const ACTION_STYLE: Record<string, string> = {
  allow: "border-sky-500/25 bg-sky-500/10 text-sky-800 dark:text-sky-200",
  redact:
    "border-emerald-500/25 bg-emerald-500/10 text-emerald-800 dark:text-emerald-200",
  deny: "border-red-500/25 bg-red-500/10 text-red-800 dark:text-red-200",
};

const CATEGORY_LABELS: Record<string, string> = {
  llm01_prompt_injection: "Prompt injection attempt",
  llm02_sensitive_information_disclosure: "Sensitive information disclosure",
  llm07_system_prompt_leakage: "System prompt leak",
  prompt_injection: "Prompt injection attempt",
  indirect_prompt_injection: "Hidden instruction risk",
  pii: "Private personal data",
  secret: "Secret or API key",
  credential: "Credential",
  data_exfiltration: "Data exfiltration risk",
  system_prompt_leak: "System prompt leak",
  output_leak: "Sensitive output risk",
  unsafe_output: "Unsafe output",
};

const CATEGORY_MESSAGES: Record<string, string> = {
  llm01_prompt_injection:
    "Pollek saw instructions that looked like a prompt injection attempt. The incident was recorded without showing the raw prompt.",
  llm02_sensitive_information_disclosure:
    "Pollek saw sensitive data in the prompt or response path and recorded the protection result without storing the sensitive value.",
  llm07_system_prompt_leakage:
    "Pollek saw output that looked like system prompt or hidden instruction leakage and recorded the incident for review.",
  prompt_injection:
    "Pollek saw instructions that looked like a prompt injection attempt. The incident was recorded without showing the raw prompt.",
  secret:
    "Pollek saw a secret or API key pattern and recorded whether it was watched, redacted, or blocked.",
  system_prompt_leak:
    "Pollek saw output that looked like system prompt or hidden instruction leakage and recorded the incident for review.",
};

const CATEGORY_ACTIONS: Record<string, string[]> = {
  llm01_prompt_injection: [
    "Review the document, webpage, or tool output the AI app was using.",
    "Route this AI app through Prompt Guard before allowing similar prompts.",
    "Tighten the AI app's own tool and prompt-safety settings if local blocking is not available.",
  ],
  llm02_sensitive_information_disclosure: [
    "Confirm whether the AI app needs this data.",
    "Use a narrower file, folder, website, or connector permission.",
    "Keep redaction enabled for this AI app path.",
  ],
  llm07_system_prompt_leakage: [
    "Do not reuse or forward the leaked output.",
    "Rotate any canary token or secret if the output looked real.",
    "Review previous tool outputs and responses in this session.",
  ],
  prompt_injection: [
    "Review the document, webpage, or tool output the AI app was using.",
    "Route this AI app through Prompt Guard before allowing similar prompts.",
  ],
};

const FINDING_LABELS: Record<string, string> = {
  api_key: "API key or secret",
  bearer_token: "Bearer token",
  credential: "Credential",
  email: "Email address",
  phone: "Phone number",
  pii: "Private personal data",
  prompt_injection: "Prompt injection signal",
  secret: "Secret",
  system_prompt: "System prompt text",
  thai_id: "Thai ID number",
};

function labelize(value?: string | null) {
  if (!value) return "Unknown";
  return value
    .replace(/[_:.-]+/g, " ")
    .split(" ")
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function looksCorrupted(value?: string | null) {
  if (!value) return false;
  return /โ€|�|Â|à|เน|เธ|เธฃ|เธซ|เธญ|เธง/.test(value);
}

function eventKey(ev: GuardEvent) {
  return ev.event_id || `${ev.ts}-${ev.action}-${ev.categories.join("-")}`;
}

function formatDateTime(value?: string) {
  if (!value) return "Time not recorded";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

function primaryCategory(ev: GuardEvent) {
  return ev.categories[0] ?? "safety";
}

export function guardCategoryLabel(category?: string | null) {
  if (!category) return "Prompt or data safety";
  return CATEGORY_LABELS[category] ?? labelize(category);
}

export function guardFindingLabel(kind?: string | null) {
  if (!kind) return "Finding";
  return FINDING_LABELS[kind] ?? labelize(kind);
}

export function guardActionLabel(action?: string | null) {
  if (action === "deny") return "Blocked";
  if (action === "redact") return "Sensitive data protected";
  return "Watched only";
}

function guardActionHeadline(ev: GuardEvent) {
  const category = guardCategoryLabel(primaryCategory(ev)).toLowerCase();
  if (ev.action === "deny") return `Pollek blocked ${category}`;
  if (ev.action === "redact") return `Pollek protected ${category}`;
  return `Pollek watched ${category}`;
}

function guardUserMessage(ev: GuardEvent) {
  const category = primaryCategory(ev);
  if (CATEGORY_MESSAGES[category]) return CATEGORY_MESSAGES[category];
  const message = ev.remediation.user_message?.trim();
  if (message && !looksCorrupted(message)) return message;
  if (ev.action === "deny") {
    return "Pollek blocked this prompt or output safety event and recorded the reason without storing sensitive content.";
  }
  if (ev.action === "redact") {
    return "Pollek redacted sensitive details and recorded what kind of data was protected.";
  }
  return "Pollek watched this prompt or data-safety event and recorded it for the timeline.";
}

function guardRecommendedActions(ev: GuardEvent) {
  const cleaned = ev.remediation.recommended_actions
    .map(normalizeRecommendedAction)
    .filter((action) => action && !looksCorrupted(action));
  if (cleaned.length) return cleaned;
  return (
    CATEGORY_ACTIONS[primaryCategory(ev)] ?? [
      "Review the affected AI app and decide whether it should use Prompt Guard.",
      "Check setup if you expected this event to be redacted or blocked.",
      "Use the AI app's own safety or tool settings when local blocking is not available.",
    ]
  );
}

function directionLabel(direction?: string | null) {
  if (direction === "request") return "Before the AI app received the prompt";
  if (direction === "response") return "After the AI app produced output";
  return "During AI activity";
}

function severityLabel(severity?: string | null) {
  if (severity === "critical") return "Needs attention";
  if (severity === "warn") return "Review soon";
  if (severity === "info") return "Informational";
  return labelize(severity ?? "info");
}

function guardAnalysisModeLabel(mode?: string | null) {
  if (mode === "local_only") return "Local only";
  if (mode === "enterprise_cloud") return "Enterprise Cloud";
  return labelize(mode ?? "Not reported");
}

function guardSourceLabel(ev: GuardEvent) {
  if (ev.source === "dashboard_manual_check") return "Dashboard local check";
  if (ev.source === "browser_extension") return "Browser extension";
  if (ev.source === "cli_hook") return "CLI hook";
  if (ev.source === "sdk_adapter") return "SDK adapter";
  if (ev.source === "mcp_proxy") return "MCP proxy";
  if (ev.source === "content_guard_local_engine") {
    return "Local Content Guard engine";
  }
  if (ev.source) return labelize(ev.source);
  if (ev.agent_id?.includes("plugin") || ev.agent_id?.includes("mcp")) {
    return "MCP proxy or plugin telemetry";
  }
  if (ev.agent_id?.includes("browser")) return "Browser extension or proxy";
  return "Local Prompt Guard telemetry";
}

function normalizeRecommendedAction(action: string) {
  const trimmed = action.trim();
  if (!trimmed) return "";
  if (!/[_.:-]/.test(trimmed)) return trimmed;
  return labelize(trimmed);
}

export function normalizeGuardEvent(
  raw: GuardEvent | GuardIncidentEnvelope,
): GuardEvent | null {
  const envelope = raw as GuardIncidentEnvelope;
  const payload = envelope.payload;
  const nested =
    payload && "guard_event" in payload ? payload.guard_event : undefined;
  const direct =
    payload && "remediation" in payload ? (payload as GuardEvent) : undefined;
  const guardEvent = nested ?? direct;

  if (guardEvent) {
    const payloadFindings =
      payload && "findings" in payload ? payload.findings : undefined;
    const payloadRedaction =
      payload && "redaction" in payload ? payload.redaction : undefined;

    return {
      ...guardEvent,
      event_id:
        guardEvent.event_id ||
        envelope.event_id ||
        `guard_${guardEvent.ts || envelope.timestamp || guardEvent.action}`,
      ts: guardEvent.ts || envelope.timestamp || "",
      tenant_id: guardEvent.tenant_id ?? envelope.tenant_id ?? null,
      agent_id: guardEvent.agent_id ?? envelope.agent_id ?? null,
      categories: guardEvent.categories ?? [],
      injection_score: guardEvent.injection_score ?? 0,
      findings_summary: guardEvent.findings_summary?.length
        ? guardEvent.findings_summary
        : (payloadFindings ?? envelope.findings ?? []),
      redaction_applied:
        Boolean(guardEvent.redaction_applied) ||
        Boolean(payloadRedaction?.applied) ||
        Boolean(envelope.redaction_applied) ||
        Boolean(envelope.redaction?.applied),
      remediation: guardEvent.remediation ?? {
        user_message:
          "Pollek observed a safety event, but the source did not include user guidance.",
        recommended_actions: [],
        can_override: false,
      },
      severity: guardEvent.severity ?? "info",
      action: guardEvent.action ?? "allow",
      direction: guardEvent.direction ?? "request",
      source: guardEvent.source ?? envelope.event_type ?? null,
    };
  }

  const event = raw as GuardEvent;
  if (event.event_id && event.remediation) {
    return {
      ...event,
      categories: event.categories ?? [],
      findings_summary: event.findings_summary ?? [],
    };
  }

  return null;
}

export function mergeGuardEvents(
  current: GuardEvent[],
  incoming: GuardEvent[],
  limit = 50,
) {
  const map = new Map<string, GuardEvent>();
  [...current, ...incoming].forEach((event) => {
    map.set(eventKey(event), event);
  });
  return Array.from(map.values())
    .sort((a, b) => {
      const left = new Date(a.ts).getTime();
      const right = new Date(b.ts).getTime();
      return (
        (Number.isNaN(right) ? 0 : right) - (Number.isNaN(left) ? 0 : left)
      );
    })
    .slice(0, limit);
}

function StatusChip({ ev }: { ev: GuardEvent }) {
  return (
    <span
      className={`inline-flex items-center rounded-full border px-2 py-1 text-xs font-medium ${
        ACTION_STYLE[ev.action] ?? ACTION_STYLE.allow
      }`}
    >
      {ev.action === "deny" ? (
        <XCircle className="mr-1 h-3.5 w-3.5" />
      ) : ev.action === "redact" ? (
        <ShieldCheck className="mr-1 h-3.5 w-3.5" />
      ) : (
        <Eye className="mr-1 h-3.5 w-3.5" />
      )}
      {guardActionLabel(ev.action)}
    </span>
  );
}

export function GuardIncidentCard({ ev }: { ev: GuardEvent }) {
  const { mode } = useMode();
  const showTechnicalDetails = isAdvanceMode(mode);
  const severityStyle =
    SEVERITY_STYLE[ev.severity] ?? "border-border bg-card/70 text-foreground";
  const categories = ev.categories.length
    ? ev.categories.map(guardCategoryLabel)
    : ["Prompt or data safety"];
  const actions = guardRecommendedActions(ev);

  return (
    <article className={`rounded-lg border border-l-4 p-4 ${severityStyle}`}>
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <ShieldAlert className="h-4 w-4 shrink-0" />
            <h3 className="text-base font-semibold leading-6">
              {guardActionHeadline(ev)}
            </h3>
          </div>
          <p className="mt-1 text-sm text-muted-foreground">
            {directionLabel(ev.direction)} - {ev.agent_id || "Local AI app"} -{" "}
            {formatDateTime(ev.ts)}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <StatusChip ev={ev} />
          <span className="inline-flex items-center rounded-full border border-current/20 px-2 py-1 text-xs font-medium">
            {severityLabel(ev.severity)}
          </span>
          <span className="inline-flex items-center rounded-full border border-current/20 px-2 py-1 text-xs font-medium">
            {guardSourceLabel(ev)}
          </span>
        </div>
      </div>

      <div className="mt-4 rounded-lg border bg-background/70 p-4">
        <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          What happened
        </div>
        <p className="mt-2 text-sm leading-6">{guardUserMessage(ev)}</p>
      </div>

      <div className="mt-4 grid gap-3 md:grid-cols-2">
        <div className="rounded-lg border bg-background/60 p-4">
          <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Signals Pollek saw
          </div>
          <div className="mt-3 flex flex-wrap gap-2">
            {categories.map((category) => (
              <span
                key={category}
                className="rounded-full border bg-card px-2 py-1 text-xs"
              >
                {category}
              </span>
            ))}
            {ev.redaction_applied && (
              <span className="rounded-full border border-emerald-500/25 bg-emerald-500/10 px-2 py-1 text-xs text-emerald-800 dark:text-emerald-200">
                Redaction applied
              </span>
            )}
          </div>
        </div>

        <div className="rounded-lg border bg-background/60 p-4">
          <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Sensitive details
          </div>
          {ev.findings_summary.length > 0 ? (
            <div className="mt-3 space-y-2">
              {ev.findings_summary.map((finding) => (
                <div
                  key={finding.kind}
                  className="flex items-center justify-between gap-3 rounded-md border bg-card px-3 py-2 text-sm"
                >
                  <span>{guardFindingLabel(finding.kind)}</span>
                  <span className="font-medium">{finding.count}</span>
                </div>
              ))}
            </div>
          ) : (
            <p className="mt-3 text-sm text-muted-foreground">
              No sensitive item counts were included with this event.
            </p>
          )}
        </div>
      </div>

      {actions.length > 0 && (
        <div className="mt-4 rounded-lg border bg-background/60 p-4">
          <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Recommended next steps
          </div>
          <ul className="mt-3 space-y-2 text-sm">
            {actions.map((action) => (
              <li key={action} className="flex gap-2">
                <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-emerald-600" />
                <span>{action}</span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {ev.remediation.can_override && (
        <div className="mt-4 rounded-lg border border-amber-500/25 bg-amber-500/10 p-3 text-sm text-amber-950 dark:text-amber-100">
          Approval override is not connected in this local path yet. Use Prompt
          Guard setup or the AI app's own safety settings to change what is
          allowed.
        </div>
      )}

      {showTechnicalDetails && (
        <div className="mt-4 rounded-lg border bg-background/50 p-4">
          <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Technical details
          </div>
          <dl className="mt-3 grid gap-2 text-sm md:grid-cols-2">
            <div className="flex justify-between gap-3">
              <dt className="text-muted-foreground">Event ID</dt>
              <dd className="max-w-[14rem] truncate font-medium">
                {ev.event_id}
              </dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt className="text-muted-foreground">Raw action</dt>
              <dd className="font-medium">{ev.action}</dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt className="text-muted-foreground">Direction</dt>
              <dd className="font-medium">{ev.direction}</dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt className="text-muted-foreground">Injection score</dt>
              <dd className="font-medium">{ev.injection_score}</dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt className="text-muted-foreground">Analysis mode</dt>
              <dd className="font-medium">
                {guardAnalysisModeLabel(ev.analysis_pipeline?.mode)}
              </dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt className="text-muted-foreground">Third-party NER</dt>
              <dd className="font-medium">
                {ev.analysis_pipeline?.enterprise_cloud_ner_enabled
                  ? ev.analysis_pipeline.third_party_provider || "Enabled"
                  : ev.analysis_pipeline?.enterprise_cloud_ner_supported
                    ? "Enterprise-supported, off locally"
                    : "Not configured"}
              </dd>
            </div>
            <div className="md:col-span-2">
              <dt className="text-muted-foreground">Raw categories</dt>
              <dd className="mt-1 break-words font-medium">
                {ev.categories.join(", ") || "none"}
              </dd>
            </div>
            {ev.analysis_pipeline?.steps?.length ? (
              <div className="md:col-span-2">
                <dt className="text-muted-foreground">Analysis steps</dt>
                <dd className="mt-1 break-words font-medium">
                  {ev.analysis_pipeline.steps.join(", ")}
                </dd>
              </div>
            ) : null}
          </dl>
        </div>
      )}
    </article>
  );
}

function PromptGuardCheckPanel({
  onEvent,
}: {
  onEvent: (event: GuardEvent) => void;
}) {
  const [text, setText] = useState("");
  const [direction, setDirection] = useState<"request" | "response">(
    "request",
  );
  const [persistMetadata, setPersistMetadata] = useState(true);
  const [checking, setChecking] = useState(false);
  const [lastResult, setLastResult] = useState<GuardEvent | null>(null);

  const canSubmit = text.trim().length > 0 && !checking;

  const runCheck = async () => {
    const value = text.trim();
    if (!value) return;
    setChecking(true);
    try {
      const response = await TelemetryApi.checkPromptGuard({
        text: value,
        direction,
        source: "dashboard_manual_check",
        surface: "local_dashboard",
        persist: persistMetadata,
      });
      const normalized = normalizeGuardEvent(response.guard_event);
      if (!normalized) {
        throw new Error("Prompt Guard returned an unreadable event.");
      }
      setLastResult(normalized);
      if (response.persisted) {
        onEvent(normalized);
      }
      setText("");

      if (response.storage_error) {
        toast.error("Prompt Guard checked, but history was not saved", {
          description: response.storage_error,
        });
      } else if (normalized.action === "deny") {
        toast.error("Prompt Guard recommends blocking this text", {
          description: "Raw text was checked locally and was not stored.",
        });
      } else if (normalized.action === "redact") {
        toast.info("Prompt Guard found a safety signal", {
          description: "Raw text was checked locally and was not stored.",
        });
      } else {
        toast.success("Prompt Guard checked this text locally", {
          description: "No configured prompt-injection signal was found.",
        });
      }
    } catch (error) {
      toast.error("Prompt Guard check failed", {
        description:
          error instanceof Error
            ? error.message
            : "The local safety service did not return a valid response.",
      });
    } finally {
      setChecking(false);
    }
  };

  return (
    <section className="rounded-lg border bg-card/70 p-4">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <h3 className="text-sm font-semibold">
            Check text with local Prompt Guard
          </h3>
          <p className="mt-1 max-w-3xl text-sm leading-6 text-muted-foreground">
            Paste a prompt or model output to run the real local guard engine.
            Raw text is sent only to this device's local safety service for the
            check and is not stored in history.
          </p>
        </div>
        <span className="inline-flex w-fit items-center rounded-full border border-emerald-500/25 bg-emerald-500/10 px-2 py-1 text-xs font-medium text-emerald-800 dark:text-emerald-200">
          Local engine
        </span>
      </div>

      <div className="mt-4 grid gap-4 lg:grid-cols-[minmax(0,1fr)_18rem]">
        <div className="space-y-3">
          <textarea
            aria-label="Text to check with Prompt Guard"
            value={text}
            onChange={(event) => setText(event.target.value)}
            placeholder="Paste a prompt, tool output, or model response to check locally..."
            className="min-h-32 w-full resize-y rounded-lg border bg-background px-3 py-2 text-sm leading-6 outline-none ring-primary/20 transition placeholder:text-muted-foreground/70 focus:ring-2"
          />
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={() => setDirection("request")}
              className={`h-9 rounded-md border px-3 text-sm font-medium transition ${
                direction === "request"
                  ? "border-primary bg-primary text-primary-foreground"
                  : "hover:bg-muted"
              }`}
            >
              Prompt before AI app
            </button>
            <button
              type="button"
              onClick={() => setDirection("response")}
              className={`h-9 rounded-md border px-3 text-sm font-medium transition ${
                direction === "response"
                  ? "border-primary bg-primary text-primary-foreground"
                  : "hover:bg-muted"
              }`}
            >
              AI output
            </button>
            <label className="inline-flex items-center gap-2 text-sm text-muted-foreground">
              <input
                type="checkbox"
                checked={persistMetadata}
                onChange={(event) => setPersistMetadata(event.target.checked)}
                className="h-4 w-4 rounded border-border"
              />
              Keep metadata in timeline
            </label>
          </div>
        </div>

        <div className="flex flex-col gap-3 rounded-lg border bg-background/70 p-3">
          <button
            type="button"
            onClick={runCheck}
            disabled={!canSubmit}
            className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-primary px-3 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {checking ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Send className="h-4 w-4" />
            )}
            Check with Prompt Guard
          </button>
          <div className="rounded-md border border-dashed p-3 text-xs leading-5 text-muted-foreground">
            Timeline history stores categories, rule IDs, action, severity,
            source, counts, and text length. It does not store the raw prompt or
            raw response.
          </div>
          {lastResult && (
            <div className="rounded-md border bg-card p-3 text-sm">
              <div className="flex items-center justify-between gap-2">
                <span className="font-medium">Latest check</span>
                <StatusChip ev={lastResult} />
              </div>
              <p className="mt-2 text-xs leading-5 text-muted-foreground">
                {guardUserMessage(lastResult)}
              </p>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

function BrowserConnectorInstallPanel() {
  return (
    <section className="rounded-lg border bg-card/70 p-4">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <h3 className="text-sm font-semibold">
            Connect real browser AI apps
          </h3>
          <p className="mt-1 max-w-3xl text-sm leading-6 text-muted-foreground">
            Use the browser connector to observe prompt submissions from
            ChatGPT, Claude, DeepSeek, Gemini, and Manus web sessions. Chrome
            and Edge use the same WebExtension core. Safari must be packaged as
            a Safari Web Extension app with Xcode.
          </p>
        </div>
        <span className="inline-flex w-fit items-center rounded-full border border-sky-500/25 bg-sky-500/10 px-2 py-1 text-xs font-medium text-sky-800 dark:text-sky-200">
          User-approved install
        </span>
      </div>

      <div className="mt-4 grid gap-3 lg:grid-cols-3">
        {BROWSER_CONNECTOR_OPTIONS.map((option) => (
          <div
            key={option.id}
            className="rounded-lg border bg-background/70 p-4"
          >
            <div className="flex items-center gap-2 text-sm font-semibold">
              {option.id === "safari" ? (
                <Store className="h-4 w-4 text-primary" />
              ) : (
                <Globe className="h-4 w-4 text-primary" />
              )}
              {option.label}
            </div>
            <p className="mt-2 text-sm leading-6 text-muted-foreground">
              {option.install}
            </p>
            <div className="mt-3 rounded-md border border-dashed p-2 text-xs leading-5 text-muted-foreground">
              Package: {option.packagePath}
            </div>
          </div>
        ))}
      </div>

      <div className="mt-4 rounded-lg border border-amber-500/20 bg-amber-500/10 p-3 text-sm leading-6 text-amber-950 dark:text-amber-100">
        Browsers do not allow silent extension installation from a local
        dashboard. Pollek can build the package and show the correct install
        path, but the user or enterprise browser policy must approve
        installation.
      </div>

      <div className="mt-4 flex flex-wrap gap-2">
        <Link
          to="/setup?category=safety"
          className="inline-flex h-9 items-center gap-2 rounded-md bg-primary px-3 text-sm font-medium text-primary-foreground hover:bg-primary/90"
        >
          <Download className="h-4 w-4" />
          Open setup guidance
        </Link>
        <Link
          to="/activity?category=safety"
          className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
        >
          <ArrowRight className="h-4 w-4" />
          Watch connector activity
        </Link>
      </div>
    </section>
  );
}

function PromptCaptureAdapterPanel() {
  return (
    <section className="rounded-lg border bg-card/70 p-4">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <h3 className="text-sm font-semibold">
            Capture desktop-agent prompts through a real data path
          </h3>
          <p className="mt-1 max-w-3xl text-sm leading-6 text-muted-foreground">
            Desktop AI apps do not expose raw prompt bodies to a normal OS
            observer. To observe or protect prompts, the agent must use a
            wrapper, local proxy, SDK adapter, CLI hook, MCP proxy, or browser
            connector that is actually in the prompt/output path.
          </p>
        </div>
        <span className="inline-flex w-fit items-center rounded-full border border-amber-500/25 bg-amber-500/10 px-2 py-1 text-xs font-medium text-amber-800 dark:text-amber-200">
          Setup required per app
        </span>
      </div>

      <div className="mt-4 grid gap-3 lg:grid-cols-4">
        {PROMPT_CAPTURE_ADAPTERS.map((adapter) => (
          <div
            key={adapter.id}
            className="rounded-lg border bg-background/70 p-4"
          >
            <div className="text-sm font-semibold">{adapter.label}</div>
            <p className="mt-1 text-xs font-medium text-primary">
              {adapter.status}
            </p>
            <p className="mt-2 text-sm leading-6 text-muted-foreground">
              {adapter.bestFor}
            </p>
            <div className="mt-3 rounded-md border border-dashed p-2 text-xs leading-5 text-muted-foreground">
              {adapter.dataPath}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function EnterpriseNerExtensionPointPanel() {
  return (
    <section className="rounded-lg border bg-card/70 p-4">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <h3 className="text-sm font-semibold">
            Enterprise Cloud NER is a consented extension point
          </h3>
          <p className="mt-1 max-w-3xl text-sm leading-6 text-muted-foreground">
            Local mode uses the deterministic on-device guard engine. Enterprise
            Cloud can later route selected metadata or redacted text to an
            approved third-party NER provider, but only after tenant consent,
            provider disclosure, routing policy, and audit events are configured.
          </p>
        </div>
        <span className="inline-flex w-fit items-center rounded-full border border-sky-500/25 bg-sky-500/10 px-2 py-1 text-xs font-medium text-sky-800 dark:text-sky-200">
          Designed, not enabled locally
        </span>
      </div>
    </section>
  );
}

function EmptyState({
  feedStatus,
  streamStatus,
}: {
  feedStatus: FeedStatus;
  streamStatus: StreamStatus;
}) {
  const loading = feedStatus === "loading";
  const unavailable = feedStatus === "unavailable";

  return (
    <div className="rounded-lg border border-dashed border-border/70 bg-card/40 p-8 text-center">
      <ShieldCheck className="mx-auto h-8 w-8 text-muted-foreground" />
      <h3 className="mt-3 text-sm font-semibold">
        {loading
          ? "Loading Prompt Guard history"
          : unavailable || streamStatus === "disconnected"
            ? "Prompt Guard history is not connected"
            : "No Prompt Guard incidents yet"}
      </h3>
      <p className="mx-auto mt-2 max-w-2xl text-sm leading-6 text-muted-foreground">
        {loading
          ? "Pollek is checking stored incidents before listening for new ones."
          : unavailable || streamStatus === "disconnected"
            ? "Restart or update the local safety service, then make sure the AI app is using a guarded prompt/output path."
            : "This usually means nothing risky was observed, or the AI app is not routed through Prompt Guard yet. Use the local check above to verify the guard engine now."}
      </p>
      {!loading && (
        <div className="mt-4 flex flex-wrap justify-center gap-2">
          <Link
            to="/protect?intent=enable_prompt_guard"
            className="inline-flex h-9 items-center gap-2 rounded-md bg-primary px-3 text-sm font-medium text-primary-foreground hover:bg-primary/90"
          >
            <ShieldCheck className="h-4 w-4" />
            Enable Prompt Guard
          </Link>
          <Link
            to="/setup?category=safety"
            className="inline-flex h-9 items-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
          >
            <Wrench className="h-4 w-4" />
            Check setup
          </Link>
        </div>
      )}
    </div>
  );
}

function StreamBadge({ status }: { status: StreamStatus }) {
  const copy =
    status === "live"
      ? "Live"
      : status === "connecting"
        ? "Connecting"
        : "Disconnected";
  const style =
    status === "live"
      ? "border-emerald-500/25 bg-emerald-500/10 text-emerald-700 dark:text-emerald-200"
      : status === "connecting"
        ? "border-sky-500/25 bg-sky-500/10 text-sky-700 dark:text-sky-200"
        : "border-amber-500/25 bg-amber-500/10 text-amber-800 dark:text-amber-200";

  return (
    <span
      className={`inline-flex items-center rounded-full border px-2 py-1 text-xs font-medium ${style}`}
    >
      <Radio className="mr-1 h-3.5 w-3.5" />
      {copy}
    </span>
  );
}

export function GuardIncidentFeed() {
  const [events, setEvents] = useState<GuardEvent[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [feedStatus, setFeedStatus] = useState<FeedStatus>("loading");
  const [streamStatus, setStreamStatus] = useState<StreamStatus>("connecting");

  const addCheckedEvent = (event: GuardEvent) => {
    setFeedStatus("ready");
    setEvents((current) => mergeGuardEvents(current, [event]));
    setSelectedId(eventKey(event));
  };

  useEffect(() => {
    let cancelled = false;

    TelemetryApi.listGuardEvents().then((page) => {
      if (cancelled) return;
      const normalized = page.items
        .map(normalizeGuardEvent)
        .filter((event): event is GuardEvent => Boolean(event));
      setEvents((current) => {
        const merged = mergeGuardEvents(current, normalized);
        return merged;
      });
      setSelectedId(
        (current) =>
          current ?? (normalized[0] ? eventKey(normalized[0]) : null),
      );
      setFeedStatus(page.unavailable ? "unavailable" : "ready");
    });

    const source = new EventSource(TelemetryApi.streamUrl("guard-events"));
    source.onopen = () => setStreamStatus("live");
    source.onmessage = (event) => {
      try {
        const parsed = normalizeGuardEvent(JSON.parse(event.data));
        if (!parsed) return;
        setFeedStatus("ready");
        setStreamStatus("live");
        setEvents((current) => {
          if (current.some((item) => eventKey(item) === eventKey(parsed))) {
            return current;
          }
          toast.info(guardActionHeadline(parsed), {
            description: guardActionLabel(parsed.action),
          });
          setSelectedId(eventKey(parsed));
          return mergeGuardEvents(current, [parsed]);
        });
      } catch {
        setStreamStatus("disconnected");
      }
    };
    source.onerror = () => setStreamStatus("disconnected");

    return () => {
      cancelled = true;
      source.close();
    };
  }, []);

  const selected = useMemo(() => {
    if (!events.length) return null;
    return events.find((event) => eventKey(event) === selectedId) ?? events[0];
  }, [events, selectedId]);

  if (events.length === 0) {
    return (
      <div className="space-y-4">
        <PromptGuardCheckPanel onEvent={addCheckedEvent} />
        <BrowserConnectorInstallPanel />
        <PromptCaptureAdapterPanel />
        <EnterpriseNerExtensionPointPanel />
        <EmptyState feedStatus={feedStatus} streamStatus={streamStatus} />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <PromptGuardCheckPanel onEvent={addCheckedEvent} />
      <BrowserConnectorInstallPanel />
      <PromptCaptureAdapterPanel />
      <EnterpriseNerExtensionPointPanel />
      <div className="grid gap-4 xl:grid-cols-[minmax(260px,0.75fr)_minmax(0,1.2fr)_minmax(240px,0.7fr)]">
        <section className="rounded-lg border bg-card/60">
          <div className="border-b p-4">
            <div className="flex items-center justify-between gap-3">
              <div>
                <h3 className="text-sm font-semibold">Incident timeline</h3>
                <p className="mt-1 text-xs text-muted-foreground">
                  Stored history plus live Prompt Guard events.
                </p>
              </div>
              <StreamBadge status={streamStatus} />
            </div>
          </div>
          <div className="max-h-[42rem] space-y-2 overflow-auto p-3">
            {events.map((event) => {
              const active = selected && eventKey(selected) === eventKey(event);
              return (
                <button
                  key={eventKey(event)}
                  type="button"
                  onClick={() => setSelectedId(eventKey(event))}
                  className={`w-full cursor-pointer rounded-lg border p-3 text-left transition ${
                    active
                      ? "border-primary bg-primary/10"
                      : "bg-background/70 hover:bg-muted/60"
                  }`}
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0">
                      <div className="truncate text-sm font-semibold">
                        {guardActionHeadline(event)}
                      </div>
                      <div className="mt-1 truncate text-xs text-muted-foreground">
                        {event.agent_id || "Local AI app"} -{" "}
                        {formatDateTime(event.ts)}
                      </div>
                    </div>
                    <StatusChip ev={event} />
                  </div>
                  <p className="mt-2 line-clamp-2 text-xs leading-5 text-muted-foreground">
                    {guardUserMessage(event)}
                  </p>
                </button>
              );
            })}
          </div>
        </section>

        <section className="min-w-0">
          {selected ? <GuardIncidentCard ev={selected} /> : null}
        </section>

        <aside className="space-y-3">
          <section className="rounded-lg border bg-card/60 p-4">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <Info className="h-4 w-4 text-primary" />
              What this means
            </div>
            <p className="mt-2 text-sm leading-6 text-muted-foreground">
              Pollek records prompt and output safety events so you can see what
              happened, which AI app was involved, and whether data was only
              watched, redacted, or blocked.
            </p>
          </section>

          <section className="rounded-lg border bg-card/60 p-4">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <LockKeyhole className="h-4 w-4 text-primary" />
              Control paths
            </div>
            <p className="mt-2 text-sm leading-6 text-muted-foreground">
              Blocking and redaction require the AI app to use a guarded path.
              Observation can still help you adjust the AI app's own permissions
              when local blocking is not available.
            </p>
            <div className="mt-3 flex flex-col gap-2">
              <Link
                to="/protect?intent=enable_prompt_guard"
                className="inline-flex h-9 items-center justify-center gap-2 rounded-md bg-primary px-3 text-sm font-medium text-primary-foreground hover:bg-primary/90"
              >
                <ShieldCheck className="h-4 w-4" />
                Enable Prompt Guard
              </Link>
              <Link
                to="/setup?category=safety"
                className="inline-flex h-9 items-center justify-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
              >
                <Wrench className="h-4 w-4" />
                Check setup
              </Link>
              <Link
                to="/activity?category=safety"
                className="inline-flex h-9 items-center justify-center gap-2 rounded-md border px-3 text-sm hover:bg-muted"
              >
                <ArrowRight className="h-4 w-4" />
                Open activity
              </Link>
            </div>
          </section>

          <section className="rounded-lg border bg-card/60 p-4">
            <div className="flex items-center gap-2 text-sm font-semibold">
              <ShieldCheck className="h-4 w-4 text-primary" />
              Real app capture
            </div>
            <div className="mt-3 space-y-3 text-sm leading-6 text-muted-foreground">
              <p>
                Browser connector: ChatGPT, Claude, DeepSeek, Gemini, and
                Manus web prompts.
              </p>
              <p>
                Hooks and wrappers: Codex, Claude Code, SDK agents, and MCP
                tools can send checks into this same timeline.
              </p>
              <p>
                OS observe: processes, files, folders, commands, and network
                metadata. It does not expose encrypted prompt bodies by itself.
              </p>
            </div>
          </section>

          <section className="rounded-lg border border-amber-500/20 bg-amber-500/10 p-4 text-sm text-amber-950 dark:text-amber-100">
            <div className="flex items-center gap-2 font-semibold">
              <AlertTriangle className="h-4 w-4" />
              Setup reminder
            </div>
            <p className="mt-2 leading-6">
              If this page stays empty while you test an AI app, connect that
              app through Prompt Guard, a browser extension, a CLI hook, an SDK
              wrapper, or the app's supported safety settings.
            </p>
          </section>
        </aside>
      </div>
    </div>
  );
}
