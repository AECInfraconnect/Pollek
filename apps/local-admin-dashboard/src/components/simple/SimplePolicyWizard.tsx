import { useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { CheckCircle2, ChevronRight, Eye, ShieldCheck } from "lucide-react";
import { toast } from "sonner";
import { useMode } from "../../context/ModeContext";
import { isAdvanceMode } from "../../lib/modes";
import { defaultClient as client } from "../../services/api";
import type {
  ControlLevel,
  PolicyFeasibilityResult,
  PolicySuggestion,
} from "../../services/types";
import { ControlLevelSelector } from "./ControlLevelSelector";
import { FeasibilityPreview } from "./FeasibilityPreview";

type Step = 1 | 2 | 3 | 4;

const STEP_LABELS = [
  "Choose AI",
  "Choose activity",
  "Choose behavior",
  "Review setup",
];

const FALLBACK_SUGGESTIONS: PolicySuggestion[] = [
  {
    id: "pol_watch_activity",
    title_en: "Watch all activity",
    title_th: "ดูทุกกิจกรรม",
    domains: ["network", "file_system", "process"],
    recommended_level: "observe",
  },
  {
    id: "pol_prompt_guard",
    title_en: "Guard prompts and private data",
    title_th: "ป้องกันพรอมป์และข้อมูลส่วนตัว",
    domains: ["prompt_content", "mcp_tool_call"],
    recommended_level: "ask",
  },
  {
    id: "pol_ask_before_write",
    title_en: "Ask before changing files",
    title_th: "ถามก่อนแก้ไขไฟล์",
    domains: ["file_system"],
    recommended_level: "ask",
  },
];

function StepDots({ step }: { step: Step }) {
  return (
    <div className="flex flex-wrap gap-2">
      {STEP_LABELS.map((label, index) => {
        const active = step >= index + 1;
        return (
          <div
            key={label}
            className={`inline-flex items-center gap-2 rounded-full border px-3 py-1 text-xs font-medium ${
              active
                ? "border-primary/30 bg-primary/10 text-primary"
                : "border-border bg-background text-muted-foreground"
            }`}
          >
            <span>{index + 1}</span>
            <span>{label}</span>
          </div>
        );
      })}
    </div>
  );
}

function Section({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <section className="space-y-4">
      <div>
        <h3 className="text-lg font-semibold">{title}</h3>
        <p className="mt-1 text-sm leading-6 text-muted-foreground">
          {description}
        </p>
      </div>
      {children}
    </section>
  );
}

function NextButton({
  disabled,
  onClick,
  children,
}: {
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className="inline-flex h-10 items-center gap-2 rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
    >
      {children}
      <ChevronRight className="h-4 w-4" />
    </button>
  );
}

function suggestionTitle(suggestion: PolicySuggestion) {
  const lang =
    typeof localStorage !== "undefined"
      ? localStorage.getItem("i18nextLng")
      : "en";
  return lang === "th" ? suggestion.title_th : suggestion.title_en;
}

export function SimplePolicyWizard({
  agents = [],
  initialTarget,
  onComplete,
  onCancel,
}: {
  agents?: { id: string; label: string }[];
  initialTarget?: string;
  onComplete?: () => void;
  onCancel?: () => void;
}) {
  const { mode } = useMode();
  const [step, setStep] = useState<Step>(1);
  const [picked, setPicked] = useState<string[]>([]);
  const [suggestions, setSuggestions] =
    useState<PolicySuggestion[]>(FALLBACK_SUGGESTIONS);
  const [policy, setPolicy] = useState<PolicySuggestion | null>(null);
  const [level, setLevel] = useState<ControlLevel>("observe");
  const [feasibility, setFeasibility] =
    useState<PolicyFeasibilityResult | null>(null);
  const [plan, setPlan] = useState<any | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const pickedLabels = useMemo(
    () =>
      agents
        .filter((agent) => picked.includes(agent.id))
        .map((agent) => agent.label),
    [agents, picked],
  );

  useEffect(() => {
    if (initialTarget && picked.length === 0) {
      setPicked([initialTarget]);
      void loadSuggestions([initialTarget]).then(() => setStep(2));
    }
  }, [initialTarget, picked.length]);

  async function loadSuggestions(targets: string[]) {
    setBusy(true);
    try {
      const next = await client.getPolicySuggestions(targets);
      setSuggestions(next.length > 0 ? next : FALLBACK_SUGGESTIONS);
    } catch {
      setSuggestions(FALLBACK_SUGGESTIONS);
    } finally {
      setBusy(false);
    }
  }

  async function toActivityChoice() {
    await loadSuggestions(picked);
    setStep(2);
  }

  async function toReview() {
    if (!policy) return;
    setBusy(true);
    try {
      const feas = await client.previewFeasibility(policy, level);
      setFeasibility(feas);
      if (isAdvanceMode(mode)) {
        const session = await client.createDeploySession({
          policy,
          agents: picked,
          requested_level: level,
        });
        setSessionId(session.id);
        setPlan(await client.confirmDeploySession(session.id));
      }
    } catch {
      setFeasibility({
        policy_id: policy.id,
        requested_level: level,
        achievable_level: level,
        verdict: "observe_only",
        per_domain: [],
        gaps: [],
        friendly_en:
          "Pollek can start by watching this activity. Blocking may need extra setup on this computer or inside the AI app.",
        friendly_th:
          "Pollek เริ่มจากการดูเหตุการณ์นี้ได้ การบล็อกอาจต้องตั้งค่าเพิ่มบนเครื่องหรือใน AI app",
      } as PolicyFeasibilityResult);
    } finally {
      setBusy(false);
      setStep(4);
    }
  }

  async function activateRule() {
    if (!policy) return;
    setBusy(true);
    try {
      let sid = sessionId;
      if (!sid) {
        const session = await client.createDeploySession({
          policy,
          agents: picked,
          requested_level: level,
        });
        await client.confirmDeploySession(session.id);
        sid = session.id;
      }
      await client.applyDeploySession(sid);
      if (onComplete) {
        onComplete();
        return;
      }
    } catch (error) {
      console.error("Failed to activate rule", error);
      toast.error(
        "Could not activate this rule. Check that the local service is running, then try again.",
      );
      return;
    } finally {
      setBusy(false);
    }
    window.location.assign("/activity");
  }

  return (
    <div className="relative mx-auto max-w-3xl space-y-6 py-4">
      {onCancel && (
        <button
          type="button"
          onClick={onCancel}
          className="absolute right-0 top-0 rounded-md border bg-background px-3 py-1.5 text-sm text-muted-foreground hover:bg-muted"
        >
          Cancel
        </button>
      )}

      <div className="space-y-3">
        <StepDots step={step} />
        <div className="rounded-lg border bg-card/60 p-4">
          <div className="flex items-start gap-3">
            <div className="rounded-lg bg-primary/10 p-2 text-primary">
              <ShieldCheck className="h-4 w-4" />
            </div>
            <div>
              <h2 className="text-base font-semibold">
                Create an AI activity rule
              </h2>
              <p className="mt-1 text-sm leading-6 text-muted-foreground">
                Start from what you want to see or stop. Pollek will tell you
                whether this computer can watch, ask first, or block it now.
              </p>
            </div>
          </div>
        </div>
      </div>

      {step === 1 && !initialTarget && (
        <Section
          title="Choose the AI app"
          description="Pick one or more AI apps. You can start with watch-only and tighten controls later."
        >
          <div className="grid gap-2">
            {agents.length > 0 ? (
              agents.map((agent) => {
                const active = picked.includes(agent.id);
                return (
                  <button
                    key={agent.id}
                    type="button"
                    onClick={() =>
                      setPicked((current) =>
                        current.includes(agent.id)
                          ? current.filter((id) => id !== agent.id)
                          : [...current, agent.id],
                      )
                    }
                    className={`flex items-center justify-between rounded-lg border p-3 text-left text-sm ${
                      active
                        ? "border-primary bg-primary/10"
                        : "border-border bg-background hover:bg-muted"
                    }`}
                  >
                    <span className="font-medium">{agent.label}</span>
                    {active && <CheckCircle2 className="h-4 w-4 text-primary" />}
                  </button>
                );
              })
            ) : (
              <div className="rounded-lg border border-dashed p-6 text-sm text-muted-foreground">
                No AI apps are loaded yet. Run Find AI Apps first, then come
                back to create a rule.
              </div>
            )}
          </div>
          <NextButton
            disabled={!picked.length || busy}
            onClick={toActivityChoice}
          >
            Choose activity
          </NextButton>
        </Section>
      )}

      {step === 1 && initialTarget && (
        <div className="rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
          Loading rule suggestions...
        </div>
      )}

      {step === 2 && (
        <Section
          title="Choose what to watch or control"
          description="These are plain-language rule starters. Advanced policy details stay available after review."
        >
          <div className="grid gap-2">
            {suggestions.map((suggestion) => {
              const active = policy?.id === suggestion.id;
              return (
                <button
                  key={suggestion.id}
                  type="button"
                  onClick={() => {
                    setPolicy(suggestion);
                    setLevel(suggestion.recommended_level);
                  }}
                  className={`rounded-lg border p-4 text-left ${
                    active
                      ? "border-primary bg-primary/10"
                      : "border-border bg-background hover:bg-muted"
                  }`}
                >
                  <div className="flex items-start justify-between gap-3">
                    <div>
                      <div className="text-sm font-semibold">
                        {suggestionTitle(suggestion)}
                      </div>
                      <div className="mt-1 text-xs text-muted-foreground">
                        Covers {suggestion.domains.join(", ") || "AI activity"}
                      </div>
                    </div>
                    {active && <CheckCircle2 className="h-4 w-4 text-primary" />}
                  </div>
                </button>
              );
            })}
          </div>
          <NextButton disabled={!policy} onClick={() => setStep(3)}>
            Choose behavior
          </NextButton>
        </Section>
      )}

      {step === 3 && (
        <Section
          title="Choose what Pollek should do"
          description="Watching is always the safest starting point. Ask first and block depend on the local OS setup and the AI app path."
        >
          <ControlLevelSelector value={level} onChange={setLevel} />
          <NextButton disabled={busy} onClick={toReview}>
            Review setup
          </NextButton>
        </Section>
      )}

      {step === 4 && feasibility && (
        <Section
          title="Review what can really happen"
          description={`Selected AI app${pickedLabels.length === 1 ? "" : "s"}: ${
            pickedLabels.join(", ") || picked.join(", ")
          }`}
        >
          <FeasibilityPreview result={feasibility as any} />

          {isAdvanceMode(mode) && plan && (
            <details className="rounded-lg border bg-background p-4 text-sm">
              <summary className="cursor-pointer font-semibold">
                Advanced control method plan
              </summary>
              <ul className="mt-3 space-y-2 text-xs text-muted-foreground">
                {(plan.bindings ?? []).map((binding: any, index: number) => (
                  <li key={`${binding.domain}-${index}`}>
                    {binding.domain}: {binding.method_id} ({binding.effective_level})
                  </li>
                ))}
                {(plan.fallbacks ?? []).map((fallback: string, index: number) => (
                  <li key={`fallback-${index}`}>Fallback: {fallback}</li>
                ))}
              </ul>
            </details>
          )}

          <div className="rounded-lg border border-blue-500/20 bg-blue-500/10 p-4 text-sm text-blue-700">
            <div className="flex items-start gap-2">
              <Eye className="mt-0.5 h-4 w-4" />
              <p>
                Even when Pollek cannot block something directly on this OS, it
                will still show the activity and explain what to change inside
                the AI app settings or local setup.
              </p>
            </div>
          </div>

          <button
            type="button"
            onClick={activateRule}
            className="inline-flex h-10 w-full items-center justify-center gap-2 rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground hover:bg-primary/90"
          >
            Save rule and watch activity
          </button>
        </Section>
      )}
    </div>
  );
}
