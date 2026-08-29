/**
 * The Look picker, and the preferences behind the one being worn.
 *
 * A picker rather than a row of switches, because there is exactly one Look at
 * a time and a set of toggles that can all be on is a lie about the thing
 * underneath.
 *
 * Choosing one does two different kinds of thing, and the panel is written to
 * keep the difference visible rather than to hide it. The overlay and the skin
 * are ours — instant, nothing written. The *Look* that goes with it changes
 * Windows itself, so it is offered through the same confirmation screen as any
 * other change and can be declined; the note under the picker says so, because
 * a switch that quietly rewrote the registry would be exactly the thing this
 * project exists not to be.
 */

import { useEffect, useState } from "react";

import type { LookId, LookInfo, ShellConfig } from "../lib/api";
import { useI18n } from "../i18n";

interface Props {
  config: ShellConfig | null;
  /** The registry, from Rust. The UI keeps no list of its own. */
  looks: LookInfo[];
  onLookChange: (id: LookId | null) => void;
  onOptionsChange: (options: { sound?: boolean; telemetry?: boolean; address?: string }) => void;
}

export function LookPanel({ config, looks, onLookChange, onOptionsChange }: Props) {
  const { t } = useI18n();

  const active = config?.active ?? null;
  const worn = looks.find((look) => look.id === active) ?? null;

  // The readouts belong to whatever is drawn on the desktop, so they are only
  // worth offering when something is. The voice and the name it uses are
  // JARVIS's own — nothing else speaks.
  const hasOverlay = worn?.surfaces.includes("overlay") ?? false;
  const speaks = active === "jarvis";

  return (
    <section className="panel">
      <h2>{t("look.title")}</h2>
      <p className="muted">{t("look.body")}</p>

      <div className="looks-picker" role="radiogroup" aria-label={t("look.title")}>
        <label className="dock-toggle">
          <input
            type="radio"
            name="shell-look"
            checked={active === null}
            disabled={!config}
            onChange={() => onLookChange(null)}
          />
          <span>{t("look.none")}</span>
        </label>

        {looks.map((look) => (
          <label className="dock-toggle" key={look.id}>
            <input
              type="radio"
              name="shell-look"
              checked={active === look.id}
              disabled={!config}
              onChange={() => onLookChange(look.id)}
            />
            <span>{t(`look.name.${look.id}`)}</span>
          </label>
        ))}
      </div>

      {worn && <p className="muted small">{t(`look.desc.${worn.id}`)}</p>}
      <p className="muted small">{t("look.lookNote")}</p>

      {/* The preferences only exist once something is drawing. Hidden rather
          than disabled: dimmed controls under an unpicked Look are noise. */}
      {config && hasOverlay && (
        <div className="jarvis-options">
          {speaks && (
            <>
              <label className="dock-toggle">
                <input
                  type="checkbox"
                  checked={config.sound}
                  onChange={(event) => onOptionsChange({ sound: event.target.checked })}
                />
                <span>{t("jarvis.sound")}</span>
              </label>
              <p className="muted small">{t("jarvis.soundNote")}</p>
            </>
          )}

          <label className="dock-toggle">
            <input
              type="checkbox"
              checked={config.telemetry}
              onChange={(event) => onOptionsChange({ telemetry: event.target.checked })}
            />
            <span>{t("jarvis.telemetry")}</span>
          </label>
          <p className="muted small">{t("jarvis.telemetryNote")}</p>

          {speaks && (
            <>
              <AddressField
                value={config.address}
                onCommit={(address) => onOptionsChange({ address })}
              />
              <p className="muted small">{t("jarvis.addressNote")}</p>
            </>
          )}
        </div>
      )}
    </section>
  );
}

/**
 * The name the greeting uses.
 *
 * Typing is local; the value is committed when the field is left or Enter is
 * pressed. Sending every keystroke through would be a file written to disk and
 * an event broadcast to three windows per letter — and would speak a
 * half-finished name if the sound happened to fire.
 */
function AddressField({
  value,
  onCommit,
}: {
  value: string;
  onCommit: (value: string) => void;
}) {
  const { t } = useI18n();
  const [draft, setDraft] = useState(value);

  // Follows the config when it changes elsewhere — another window, or Rust
  // trimming what was typed — without fighting the user mid-word.
  useEffect(() => setDraft(value), [value]);

  const commit = () => {
    if (draft.trim() !== value) onCommit(draft);
  };

  return (
    <label className="field">
      <span>{t("jarvis.address")}</span>
      <input
        type="text"
        className="input"
        value={draft}
        maxLength={40}
        placeholder={t("jarvis.addressPlaceholder")}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") event.currentTarget.blur();
        }}
      />
    </label>
  );
}
