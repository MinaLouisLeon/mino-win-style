import { useEffect, useRef } from "react";

import type { Plan } from "../lib/api";
import { useI18n, useTweakText } from "../i18n";

interface Props {
  plan: Plan | null;
  busy: boolean;
  onApply: () => void;
  onCancel: () => void;
}

/**
 * The confirmation step. It shows the literal registry writes, not a summary —
 * this is the screen that has to earn the user's trust before anything is
 * touched, and hiding the detail would defeat the point.
 */
export function PlanDialog({ plan, busy, onApply, onCancel }: Props) {
  const { t, tOr } = useI18n();
  const text = useTweakText();
  const applyRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (plan) applyRef.current?.focus();
  }, [plan]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [busy, onCancel]);

  /** A value as the user would say it, not as the registry stores it. */
  const show = (tweak: string, value: boolean | string) => {
    if (typeof value === "boolean") return t(value ? "common.on" : "common.off");
    return value.startsWith("#") ? value : text.choice(tweak, value);
  };

  if (!plan) return null;

  const nothing = plan.items.length === 0;

  return (
    <div className="scrim" role="presentation" onClick={() => !busy && onCancel()}>
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-label={t("plan.title")}
        onClick={(e) => e.stopPropagation()}
      >
        <h2>{t("plan.title")}</h2>

        {nothing && <p className="muted">{t("plan.none")}</p>}

        <ul className="plan">
          {plan.items.map((item) => (
            <li key={item.tweak}>
              <div className="plan__head">
                <span>{text.name(item.tweak)}</span>
                <span className="plan__values">
                  <code>{show(item.tweak, item.from)}</code>
                  <span aria-hidden="true">→</span>
                  <code className="plan__to">{show(item.tweak, item.to)}</code>
                </span>
              </div>
              <details>
                <summary>
                  {t("plan.registry")} ({item.changes.length})
                </summary>
                <ul className="plan__changes" dir="ltr">
                  {item.changes.map((change, i) => (
                    <li key={i}>
                      {change.op === "value"
                        ? `${change.loc.hive}\\${change.loc.path}\\\\${change.loc.name || "(Default)"}`
                        : `${change.hive}\\${change.path} — ${change.to_present ? "create" : "remove"}`}
                    </li>
                  ))}
                </ul>
              </details>
            </li>
          ))}
        </ul>

        {plan.skipped.length > 0 && (
          <div className="callout callout--warn">
            <strong>{t("plan.skipped")}</strong>
            <ul>
              {plan.skipped.map((s) => (
                <li key={s.tweak}>
                  {text.name(s.tweak)} —{" "}
                  {s.reason_key ? tOr(`support.note.${s.reason_key}`, s.reason) : s.reason}
                </li>
              ))}
            </ul>
          </div>
        )}

        {plan.needs_shell_restart && <p className="callout">{t("plan.restart")}</p>}
        {plan.needs_elevation && <p className="callout callout--warn">{t("plan.elevation")}</p>}

        <div className="dialog__actions">
          <button type="button" className="btn" onClick={onCancel} disabled={busy}>
            {t("plan.cancel")}
          </button>
          <button
            type="button"
            className="btn btn--primary"
            ref={applyRef}
            onClick={onApply}
            disabled={busy || nothing || plan.needs_elevation}
          >
            {busy ? t("plan.applying") : t("plan.apply")}
          </button>
        </div>
      </div>
    </div>
  );
}
