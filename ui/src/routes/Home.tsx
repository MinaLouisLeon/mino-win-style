import type { Category as Cat, JournalEntry, OsBuild, TweakState } from "../lib/api";
import { useI18n } from "../i18n";

interface Props {
  os: OsBuild | null;
  tweaks: TweakState[];
  entries: JournalEntry[];
  journalDir: string;
  onRevertAll: () => void;
  onOpenCategory: (category: Cat) => void;
}

const CATEGORIES: Cat[] = ["appearance", "taskbar", "start", "explorer"];

export function Home({ os, tweaks, entries, journalDir, onRevertAll, onOpenCategory }: Props) {
  const { t } = useI18n();

  const usable = tweaks.filter((tweak) => tweak.support.level !== "unsupported");
  const accent = tweaks.find((tweak) => tweak.id === "appearance.accent_color");
  const dark = tweaks.find((tweak) => tweak.id === "appearance.dark_mode");
  const applied = entries.filter((entry) => entry.status === "applied").length;

  return (
    <>
      <header className="page">
        <h1>{t("home.title")}</h1>
        <p className="muted">{t("home.subtitle")}</p>
      </header>

      <section className="cards">
        <article className="card card--wide">
          <span className="card__label">{t("home.os")}</span>
          <strong>
            {os ? `${os.product_name} ${os.display_version}` : "…"}
          </strong>
          <code dir="ltr">{os ? `build ${os.build}.${os.ubr}` : ""}</code>
        </article>

        <article className="card">
          <span className="card__label">{t("tweak.appearance.accent_color.name")}</span>
          <div className="swatch-row">
            <span
              className="swatch"
              style={{
                background: typeof accent?.value === "string" ? accent.value : "#0F62C0",
              }}
              aria-hidden="true"
            />
            <code dir="ltr">
              {typeof accent?.value === "string" ? accent.value : "—"}
            </code>
          </div>
        </article>

        <article className="card">
          <span className="card__label">{t("tweak.appearance.dark_mode.name")}</span>
          <strong>{t(dark?.value === true ? "common.on" : "common.off")}</strong>
        </article>

        <article className="card">
          <span className="card__label">{t("home.available")}</span>
          <strong>{usable.length}</strong>
        </article>
      </section>

      <section className="cards">
        {CATEGORIES.map((category) => {
          const count = usable.filter((tweak) => tweak.category === category).length;
          return (
            <button
              type="button"
              key={category}
              className="card card--action"
              onClick={() => onOpenCategory(category)}
            >
              <span className="card__label">{t(`nav.${category}`)}</span>
              <strong>{count}</strong>
            </button>
          );
        })}
      </section>

      <section className="panel">
        <h2>{t("home.revertAll")}</h2>
        <p className="muted">{t("home.revertAllHint")}</p>
        <p className="muted small" dir="ltr">
          {t("home.journal")}: <code>{journalDir}</code>
        </p>
        <p className="muted small" dir="ltr">
          {t("history.safeRestore")}
        </p>
        <button
          type="button"
          className="btn"
          onClick={onRevertAll}
          disabled={applied === 0}
        >
          {t("home.revertAll")}
        </button>
      </section>
    </>
  );
}
