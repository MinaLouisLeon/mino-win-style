import type { TweakState, Value } from "../lib/api";
import { useI18n, useTweakText } from "../i18n";

interface Props {
  state: TweakState;
  /** The unapplied value, if the user has touched this row. */
  pending: Value | undefined;
  onChange: (id: string, value: Value) => void;
}

export function TweakRow({ state, pending, onChange }: Props) {
  const { t, tOr } = useI18n();
  const text = useTweakText();

  const unsupported = state.support.level === "unsupported";
  const broken = state.error !== null;
  const disabled = unsupported || broken;
  const value = pending ?? state.value;
  const changed = pending !== undefined && pending !== state.value;
  const desc = text.desc(state.id);

  return (
    <div className={`row${changed ? " row--changed" : ""}${disabled ? " row--off" : ""}`}>
      <div className="row__label">
        <div className="row__name">
          {text.name(state.id)}
          {state.tier === "b" && (
            <span className="badge badge--warn" title={t("tier.b")}>
              B
            </span>
          )}
          {changed && <span className="badge badge--accent">•</span>}
        </div>
        {desc && <p className="row__desc">{desc}</p>}
        {state.support.level === "partial" && (
          <p className="row__note">
            {tOr(`support.note.${state.support.note.key}`, state.support.note.en)}
          </p>
        )}
        {/* Narrowed inline, not via the `unsupported` alias: a boolean does not
            tell TypeScript which arm of the union it is looking at. */}
        {state.support.level === "unsupported" && (
          <p className="row__note row__note--off">
            {t("support.unsupported")} —{" "}
            {tOr(`support.note.${state.support.note.key}`, state.support.note.en)}
          </p>
        )}
        {broken && (
          <p className="row__note row__note--error">
            {t("error.read")}: {state.error}
          </p>
        )}
      </div>

      <div className="row__control">
        {state.kind.kind === "bool" && (
          <Switch
            checked={value === true}
            disabled={disabled}
            label={text.name(state.id)}
            onChange={(next) => onChange(state.id, next)}
          />
        )}

        {state.kind.kind === "color" && (
          <div className="colour">
            <input
              type="color"
              aria-label={text.name(state.id)}
              disabled={disabled}
              value={typeof value === "string" ? value : "#0F62C0"}
              onChange={(e) => onChange(state.id, e.target.value.toUpperCase())}
            />
            <code>{typeof value === "string" ? value : "—"}</code>
          </div>
        )}

        {state.kind.kind === "choice" && (
          <select
            aria-label={text.name(state.id)}
            disabled={disabled}
            value={typeof value === "string" ? value : ""}
            onChange={(e) => onChange(state.id, e.target.value)}
          >
            {state.kind.choices.map((choice) => (
              <option key={choice} value={choice}>
                {text.choice(state.id, choice)}
              </option>
            ))}
          </select>
        )}
      </div>
    </div>
  );
}

function Switch({
  checked,
  disabled,
  label,
  onChange,
}: {
  checked: boolean;
  disabled: boolean;
  label: string;
  onChange: (next: boolean) => void;
}) {
  return (
    <label className="switch">
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        aria-label={label}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className="switch__track" aria-hidden="true">
        <span className="switch__thumb" />
      </span>
    </label>
  );
}
