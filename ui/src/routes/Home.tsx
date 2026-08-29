import type {
  Category as Cat,
  DockConfig,
  JarvisConfig,
  JournalEntry,
  OsBuild,
  TweakState,
} from "../lib/api";
import { JarvisPanel } from "../components/JarvisPanel";
import { useI18n } from "../i18n";

interface Props {
  os: OsBuild | null;
  tweaks: TweakState[];
  entries: JournalEntry[];
  journalDir: string;
  dock: DockConfig | null;
  onDockChange: (enabled: boolean) => void;
  jarvis: JarvisConfig | null;
  onJarvisChange: (enabled: boolean) => void;
  onJarvisOptions: (options: { sound?: boolean; telemetry?: boolean; address?: string }) => void;
  onRevertAll: () => void;
  onOpenCategory: (category: Cat) => void;
}

const CATEGORIES: Cat[] = ["appearance", "desktop", "taskbar", "start", "explorer"];

export function Home({
  os,
  tweaks,
  entries,
  journalDir,
  dock,
  onDockChange,
  jarvis,
  onJarvisChange,
  onJarvisOptions,
  onRevertAll,
  onOpenCategory,
}: Props) {
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

      <JarvisPanel
        config={jarvis}
        onEnabledChange={onJarvisChange}
        onOptionsChange={onJarvisOptions}
      />

      <section className="panel">
        <h2>{t("dock.title")}</h2>
        <p className="muted">{t("dock.body")}</p>
        <label className="dock-toggle">
          <input
            type="checkbox"
            checked={dock?.enabled ?? false}
            disabled={!dock}
            onChange={(e) => onDockChange(e.target.checked)}
          />
          <span>{t("dock.show")}</span>
        </label>
        <p className="muted small">{t("dock.note")}</p>
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
