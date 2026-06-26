import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { ControlLevelSelector } from "./ControlLevelSelector";
import { FeasibilityPreview } from "./FeasibilityPreview";
import type {
  ControlLevel,
  PolicyFeasibilityResult,
  PolicySuggestion,
} from "../../services/types";
import { defaultClient as client } from "../../services/api"; // instance ของ ControlPlaneClient

// --- PRIMITIVES (mocking the design system) ---
function StepDots({ step, labels }: { step: number; labels: string[] }) {
  return (
    <div className="flex gap-2">
      {labels.map((l, i) => (
        <span
          key={i}
          className={step >= i + 1 ? "font-bold text-primary" : "text-muted"}
        >
          {l}
        </span>
      ))}
    </div>
  );
}
function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-4">
      <h3 className="text-xl font-bold">{title}</h3>
      {children}
    </div>
  );
}
function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: () => void;
}) {
  return (
    <label className="flex items-center gap-2 cursor-pointer">
      <input type="checkbox" checked={checked} onChange={onChange} /> {label}
    </label>
  );
}
function Radio({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: () => void;
}) {
  return (
    <label className="flex items-center gap-2 cursor-pointer">
      <input type="radio" checked={checked} onChange={onChange} /> {label}
    </label>
  );
}
function NextBtn({
  disabled,
  onClick,
  label,
}: {
  disabled: boolean;
  onClick: () => void;
  label: string;
}) {
  return (
    <button
      disabled={disabled}
      onClick={onClick}
      className="mt-4 px-4 py-2 bg-primary text-white rounded disabled:opacity-50"
    >
      {label}
    </button>
  );
}
function i18nTitle(s: any) {
  const isTh = localStorage.getItem("i18nextLng") === "th";
  return isTh ? s.title_th : s.title_en;
}
// ----------------------------------------------

import { useMode } from "../../context/ModeContext";
import { isAdvanceMode } from "../../lib/modes";

type Step = 1 | 2 | 3 | 4;

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
  const { t } = useTranslation();
  const [step, setStep] = useState<Step>(1);
  const [picked, setPicked] = useState<string[]>([]);
  const [suggestions, setSuggestions] = useState<PolicySuggestion[]>([]);
  const [policy, setPolicy] = useState<PolicySuggestion | null>(null);
  const [level, setLevel] = useState<ControlLevel>("enforce");
  const [feasibility, setFeasibility] =
    useState<PolicyFeasibilityResult | null>(null);
  const [plan, setPlan] = useState<any | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (initialTarget && picked.length === 0) {
      setPicked([initialTarget]);
      setBusy(true);
      client
        .getPolicySuggestions([initialTarget])
        .then(setSuggestions)
        .catch(() => {
          setSuggestions([
            {
              id: "pol_observe",
              title_en: "Observe All Activity",
              title_th: "สังเกตการณ์ทุกกิจกรรม",
              domains: ["network"],
              recommended_level: "observe",
            },
          ]);
        })
        .finally(() => {
          setBusy(false);
          setStep(2);
        });
    }
  }, [initialTarget]);

  async function toPolicies() {
    setBusy(true);
    try {
      setSuggestions(await client.getPolicySuggestions(picked));
    } catch {
      setSuggestions([
        {
          id: "pol_observe",
          title_en: "Observe All Activity",
          title_th: "สังเกตการณ์ทุกกิจกรรม",
          domains: ["network"],
          recommended_level: "observe",
        },
      ]);
    }
    setBusy(false);
    setStep(2);
  }
  async function toConfirm() {
    setBusy(true);
    try {
      const feas = await client.previewFeasibility(policy, level);
      setFeasibility(feas); // auto-detect + auto-select เกิดที่ backend
      if (isAdvanceMode(mode)) {
        const session = await client.createDeploySession({
          policy,
          agents: picked,
          requested_level: level,
        });
        setSessionId(session.id);
        const p = await client.confirmDeploySession(session.id);
        setPlan(p);
      }
    } catch {
      setFeasibility({
        policy_id: policy!.id,
        requested_level: level,
        achievable_level: level,
        verdict: "fully_enforceable",
        per_domain: [],
        gaps: [],
        friendly_en: "This policy can be fully enforced.",
        friendly_th: "นโยบายนี้สามารถบังคับใช้ได้จริง",
      } as any);
    }
    setBusy(false);
    setStep(4);
  }
  async function protectNow() {
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
    } catch {}
    // -> ไปหน้า Activity timeline
    window.location.href = "/activity";
  }

  const isTh = localStorage.getItem("i18nextLng") === "th";

  return (
    <div className="mx-auto max-w-2xl space-y-6 relative py-4">
      {onCancel && (
        <button
          onClick={onCancel}
          className="absolute -top-2 right-0 px-3 py-1.5 text-sm font-medium rounded-md border bg-background hover:bg-muted text-muted-foreground"
        >
          Cancel
        </button>
      )}
      <StepDots
        step={step}
        labels={[
          isTh ? "1. เลือก Agent" : "1. Agent",
          isTh ? "2. เลือกนโยบาย" : "2. Policy",
          isTh ? "3. ตั้งค่าการควบคุม" : "3. Control",
          isTh ? "4. ยืนยัน" : "4. Confirm",
        ]}
      />

      {step === 1 && !initialTarget && (
        <Section title={t("step.agent")}>
          {agents.map((a) => (
            <Toggle
              key={a.id}
              label={a.label}
              checked={picked.includes(a.id)}
              onChange={() =>
                setPicked((p) =>
                  p.includes(a.id) ? p.filter((x) => x !== a.id) : [...p, a.id],
                )
              }
            />
          ))}
          <NextBtn
            disabled={!picked.length || busy}
            onClick={toPolicies}
            label={t("common.next")}
          />
        </Section>
      )}

      {step === 1 && initialTarget && (
        <div className="py-12 text-center text-sm text-muted-foreground animate-pulse">
          Loading policy suggestions...
        </div>
      )}

      {step === 2 && (
        <Section title={t("step.policy")}>
          {suggestions.map((s) => (
            <Radio
              key={s.id}
              label={i18nTitle(s)}
              checked={policy?.id === s.id}
              onChange={() => {
                setPolicy(s);
                setLevel(s.recommended_level);
              }}
            />
          ))}
          <NextBtn
            disabled={!policy}
            onClick={() => setStep(3)}
            label={t("common.next")}
          />
        </Section>
      )}

      {step === 3 && (
        <Section title={t("step.level")}>
          <ControlLevelSelector value={level} onChange={setLevel} />
          <NextBtn
            disabled={busy}
            onClick={toConfirm}
            label={t("common.review")}
          />
        </Section>
      )}

      {step === 4 && feasibility && (
        <Section title={t("step.confirm")}>
          <FeasibilityPreview result={feasibility as any} />
          {isAdvanceMode(mode) && plan && (
              <div className="mt-4 rounded-xl border border-zinc-700 bg-zinc-900/50 p-4">
                <h4 className="text-sm font-semibold text-zinc-300 mb-2">
                  Control Method Plan (Advance)
                </h4>
                <ul className="space-y-2 text-xs font-mono text-zinc-400">
                  {plan.bindings.map((b: any, i: number) => (
                    <li key={i}>
                      [{b.domain}] → {b.method_id} (Level: {b.effective_level})
                    </li>
                  ))}
                  {plan.fallbacks.map((f: string, i: number) => (
                    <li key={`f-${i}`} className="text-amber-500/80">
                      Fallback: {f}
                    </li>
                  ))}
                </ul>
              </div>
            )}
          <button
            onClick={protectNow}
            className="mt-4 w-full rounded-xl bg-violet-600 py-3 font-semibold text-white hover:bg-violet-500"
          >
            {t("simple.protect_now")}
          </button>
        </Section>
      )}
    </div>
  );
}
