import type { PackSummary } from "../lib/api";
import { useI18n } from "../i18n";

interface Props {
  packs: PackSummary[];
  busy: boolean;
  onApply: (pack: PackSummary) => void;
}

/**
 * A Look is a whole desktop in one click. It still goes through the same
 * confirmation screen as a single switch — nothing here is a shortcut past it.
 */
export function Looks({ packs, busy, onApply }: Props) {
  const { t, lang } = useI18n();

  const text = (map: Record<string, string>, fallback = "") =>
    map[lang] ?? map.en ?? fallback;

  return (
    <>
      <header className="page">
        <h1>{t("nav.looks")}</h1>
        <p className="muted">{t("looks.subtitle")}</p>
      </header>

      {packs.length === 0 && <p className="muted">{t("looks.none")}</p>}

      <section className="looks">
        {packs.map((pack) => (
          <article key={pack.id} className="look">
            <div
              className={`look__swatch look__swatch--${pack.id.split(".").pop()}`}
              aria-hidden="true"
            />
            <div className="look__body">
              <h2>{text(pack.name, pack.id)}</h2>
              <p className="muted">{text(pack.description)}</p>
              <p className="look__meta">
                {t("looks.settings", { n: pack.settings })}
                {pack.author && ` · ${pack.author}`}
              </p>
            </div>
            <button
              type="button"
              className="btn btn--primary"
              disabled={busy || !pack.applicable}
              onClick={() => onApply(pack)}
            >
              {pack.applicable ? t("looks.apply") : t("looks.unsupported")}
            </button>
          </article>
        ))}
      </section>

      <p className="muted small">{t("looks.revertNote")}</p>
    </>
  );
}
