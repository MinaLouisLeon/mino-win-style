/**
 * The JARVIS switch, and the three preferences behind it.
 *
 * The main toggle does two different kinds of thing, and the panel is written
 * to keep the difference visible rather than to hide it. Turning it on starts
 * the HUD and re-skins our own windows — ours to do, instant, nothing written.
 * The *Look* that goes with it changes Windows itself, so it is offered through
 * the same confirmation screen as any other change and can be declined; the
 * note under the switch says so, because a toggle that quietly rewrote the
 * registry would be exactly the thing this project exists not to be.
 */

import { useEffect, useState } from "react";

import type { JarvisConfig } from "../lib/api";
import { useI18n } from "../i18n";

interface Props {
  config: JarvisConfig | null;
  onEnabledChange: (enabled: boolean) => void;
  onOptionsChange: (options: { sound?: boolean; telemetry?: boolean; address?: string }) => void;
}

export function JarvisPanel({ config, onEnabledChange, onOptionsChange }: Props) {
  const { t } = useI18n();
  const on = config?.enabled ?? false;

  return (
    <section className="panel">
      <h2>{t("jarvis.title")}</h2>
      <p className="muted">{t("jarvis.body")}</p>

      <label className="dock-toggle">
        <input
          type="checkbox"
          checked={on}
          disabled={!config}
          onChange={(event) => onEnabledChange(event.target.checked)}
        />
        <span>{t("jarvis.show")}</span>
      </label>

      <p className="muted small">{t("jarvis.lookNote")}</p>

      {/* The preferences only exist once the mode does. Hidden rather than
          disabled: four dimmed controls under an off switch is noise. */}
      {on && config && (
        <div className="jarvis-options">
          <label className="dock-toggle">
            <input
              type="checkbox"
              checked={config.sound}
              onChange={(event) => onOptionsChange({ sound: event.target.checked })}
            />
            <span>{t("jarvis.sound")}</span>
          </label>
          <p className="muted small">{t("jarvis.soundNote")}</p>

          <label className="dock-toggle">
            <input
              type="checkbox"
              checked={config.telemetry}
              onChange={(event) => onOptionsChange({ telemetry: event.target.checked })}
            />
            <span>{t("jarvis.telemetry")}</span>
          </label>
          <p className="muted small">{t("jarvis.telemetryNote")}</p>

          <AddressField
            value={config.address}
            onCommit={(address) => onOptionsChange({ address })}
          />
          <p className="muted small">{t("jarvis.addressNote")}</p>
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
