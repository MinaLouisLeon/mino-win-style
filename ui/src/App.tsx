import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { PlanDialog } from "./components/PlanDialog";
import { Home } from "./routes/Home";
import { Category } from "./routes/Category";
import { History } from "./routes/History";
import { Looks } from "./routes/Looks";
import { useI18n } from "./i18n";
import { applyJarvisTheme } from "./lib/jarvis";
import * as sound from "./lib/sound";
import {
  api,
  inTauri,
  type Category as Cat,
  type JarvisConfig,
  type JournalEntry,
  type DockConfig,
  type OsBuild,
  type PackSummary,
  type Plan,
  type TweakState,
  type Value,
} from "./lib/api";

type View = "home" | "looks" | Cat | "history";

/** The Look that goes with JARVIS mode, offered — never applied — when the
 *  mode is switched on. Matches `packs/jarvis/manifest.json`. */
const JARVIS_PACK = "com.mino.jarvis";

const CATEGORIES: Cat[] = ["appearance", "desktop", "taskbar", "start", "explorer"];

export default function App() {
  const { t, lang, setLang } = useI18n();

  const [view, setView] = useState<View>("home");
  const [os, setOs] = useState<OsBuild | null>(null);
  const [tweaks, setTweaks] = useState<TweakState[]>([]);
  const [entries, setEntries] = useState<JournalEntry[]>([]);
  const [journalDir, setJournalDir] = useState("");
  const [packs, setPacks] = useState<PackSummary[]>([]);
  const [dock, setDock] = useState<DockConfig | null>(null);
  const [jarvis, setJarvis] = useState<JarvisConfig | null>(null);
  /** Set while a Look is waiting in the confirmation dialog. */
  const [pendingPack, setPendingPack] = useState<string | null>(null);

  const [pending, setPending] = useState<Record<string, Value>>({});
  const [plan, setPlan] = useState<Plan | null>(null);
  const [busy, setBusy] = useState(false);
  const [restartAsk, setRestartAsk] = useState(false);
  const [confirmRevertAll, setConfirmRevertAll] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const reload = useCallback(async () => {
    const [osInfo, list, history, dir, looks, dockConfig, jarvisConfig] = await Promise.all([
      api.osInfo(),
      api.listTweaks(),
      api.history(),
      api.journalDir(),
      api.listPacks(),
      api.dockConfig(),
      api.jarvisConfig(),
    ]);
    setOs(osInfo);
    setTweaks(list);
    setEntries(history);
    setJournalDir(dir);
    setPacks(looks);
    setDock(dockConfig);
    setJarvis(jarvisConfig);
  }, []);

  useEffect(() => {
    reload().catch((err) => setMessage(String(err)));
  }, [reload]);

  // Wear the system's own accent, so the app looks like part of the desktop it
  // is editing rather than a visitor.
  useEffect(() => {
    const accent = tweaks.find((tweak) => tweak.id === "appearance.accent_color");
    if (accent && typeof accent.value === "string") {
      document.documentElement.style.setProperty("--accent", accent.value);
    }
  }, [tweaks]);

  // The skin, and whether the app is allowed to make a noise. Both follow the
  // config wherever it was changed from, including another window.
  useEffect(() => {
    applyJarvisTheme(jarvis?.enabled ?? false);
    sound.setSoundEnabled(Boolean(jarvis?.enabled && jarvis.sound));
  }, [jarvis?.enabled, jarvis?.sound]);

  // Interface blips, added by delegation rather than by putting a handler on
  // every control: there are a few dozen of them, and none should have to know
  // that a sound scheme exists.
  //
  // `mouseover` bubbles from whatever is under the cursor, so moving between a
  // button and the text inside it fires it again — hence the guard on which
  // control was last entered.
  const lastHovered = useRef<Element | null>(null);
  useEffect(() => {
    if (!jarvis?.enabled || !jarvis.sound) return;

    const controlUnder = (event: Event) =>
      (event.target as HTMLElement | null)?.closest?.("button, .switch, .dock-toggle, .input") ??
      null;

    const over = (event: Event) => {
      const control = controlUnder(event);
      if (control && control !== lastHovered.current) sound.hover();
      lastHovered.current = control;
    };
    const down = (event: Event) => {
      if (controlUnder(event)) sound.click();
    };

    document.addEventListener("mouseover", over);
    document.addEventListener("mousedown", down);
    return () => {
      document.removeEventListener("mouseover", over);
      document.removeEventListener("mousedown", down);
      lastHovered.current = null;
    };
  }, [jarvis?.enabled, jarvis?.sound]);

  /** The line the HUD shows, spoken. Built here because this is where the
   *  dictionary is, and where the user gesture that permits speech happened. */
  const spokenGreeting = (config: JarvisConfig) => {
    const greeting = t(`hud.greeting.${sound.greetingKey()}`);
    const address = config.address ? t("hud.address", { name: config.address }) : "";
    return `${greeting}${address}. ${t("hud.nominal")}`;
  };

  /**
   * Turns the mode on or off.
   *
   * The HUD and the skin happen immediately — they are ours and they change
   * nothing on the machine. The Look is only *offered*: `reviewPack` opens the
   * same confirmation screen every other change goes through, and declining it
   * leaves JARVIS mode on with the desktop untouched.
   */
  const setJarvisEnabled = async (enabled: boolean) => {
    try {
      const next = await api.jarvisSetEnabled(enabled);
      setJarvis(next);

      // Set before playing, not left to the effect above: this click is the
      // user gesture the browser wants, and by the next render it is spent.
      sound.setSoundEnabled(next.enabled && next.sound);
      if (next.sound) {
        if (enabled) {
          sound.bootSweep();
          sound.speak(spokenGreeting(next), lang);
        } else {
          sound.powerDown();
          sound.speak(t("jarvis.farewell"), lang);
        }
      }

      if (enabled) {
        const look = packs.find((pack) => pack.id === JARVIS_PACK && pack.applicable);
        if (look) await reviewPack(look);
      }
    } catch (err) {
      setMessage(String(err));
    }
  };

  const setJarvisOptions = async (options: {
    sound?: boolean;
    telemetry?: boolean;
    address?: string;
  }) => {
    try {
      const next = await api.jarvisSetOptions(options);
      setJarvis(next);
      // Turning sound on should prove it did something.
      if (options.sound === true) {
        sound.setSoundEnabled(true);
        sound.on();
      }
    } catch (err) {
      setMessage(String(err));
    }
  };

  const change = useCallback((id: string, value: Value) => {
    setPending((current) => {
      const next = { ...current, [id]: value };
      return next;
    });
  }, []);

  // A row the user set back to where it started is not a pending change.
  const realPending = useMemo(() => {
    const out: Record<string, Value> = {};
    for (const [id, value] of Object.entries(pending)) {
      const state = tweaks.find((tweak) => tweak.id === id);
      if (state && state.value !== value) out[id] = value;
    }
    return out;
  }, [pending, tweaks]);

  const pendingCount = Object.keys(realPending).length;

  const review = async () => {
    try {
      setPendingPack(null);
      setPlan(await api.planChanges(label(), realPending));
    } catch (err) {
      setMessage(String(err));
    }
  };

  /** A Look goes to the same dialog; only the source of the plan differs. */
  const reviewPack = async (pack: PackSummary) => {
    try {
      setPendingPack(pack.dir);
      setPlan(await api.planPack(pack.dir));
    } catch (err) {
      setPendingPack(null);
      setMessage(String(err));
    }
  };

  const label = () => {
    const ids = Object.keys(realPending);
    return ids.length === 1 ? ids[0] : `${ids.length} settings`;
  };

  const apply = async () => {
    setBusy(true);
    try {
      const report = pendingPack
        ? await api.applyPack(pendingPack)
        : await api.applyChanges(label(), realPending);
      setPending({});
      setPendingPack(null);
      setPlan(null);
      await reload();
      if (report.shell_restart_pending) setRestartAsk(true);
    } catch (err) {
      setMessage(String(err));
    } finally {
      setBusy(false);
    }
  };

  const revert = async (id: string) => {
    setBusy(true);
    try {
      await api.revertEntry(id);
      await reload();
    } catch (err) {
      setMessage(String(err));
    } finally {
      setBusy(false);
    }
  };

  const revertAll = async () => {
    setBusy(true);
    try {
      await api.revertAll();
      setConfirmRevertAll(false);
      await reload();
    } catch (err) {
      setMessage(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="app">
      <aside className="nav">
        <div className="brand">
          <span className="brand__mark" aria-hidden="true" />
          <div>
            <strong>{t("app.name")}</strong>
            <small>{t("app.tagline")}</small>
          </div>
        </div>

        <nav>
          <NavButton active={view === "home"} onClick={() => setView("home")}>
            {t("nav.home")}
          </NavButton>
          <NavButton active={view === "looks"} onClick={() => setView("looks")}>
            {t("nav.looks")}
          </NavButton>
          {CATEGORIES.map((category) => (
            <NavButton
              key={category}
              active={view === category}
              onClick={() => setView(category)}
            >
              {t(`nav.${category}`)}
            </NavButton>
          ))}
          <NavButton active={view === "history"} onClick={() => setView("history")}>
            {t("nav.history")}
          </NavButton>
        </nav>

        <button
          type="button"
          className="btn btn--ghost lang"
          onClick={() => setLang(lang === "ar" ? "en" : "ar")}
        >
          {t("lang.switch")}
        </button>
      </aside>

      <main className="main">
        {!inTauri && <p className="callout callout--warn">{t("home.mock")}</p>}
        {message && (
          <p className="callout callout--error" onClick={() => setMessage(null)}>
            {message}
          </p>
        )}

        {view === "home" && (
          <Home
            os={os}
            tweaks={tweaks}
            journalDir={journalDir}
            entries={entries}
            dock={dock}
            onDockChange={async (enabled) => {
              try {
                setDock(await api.dockSetEnabled(enabled));
              } catch (err) {
                setMessage(String(err));
              }
            }}
            jarvis={jarvis}
            onJarvisChange={setJarvisEnabled}
            onJarvisOptions={setJarvisOptions}
            onRevertAll={() => setConfirmRevertAll(true)}
            onOpenCategory={(category) => setView(category)}
          />
        )}

        {view === "looks" && <Looks packs={packs} busy={busy} onApply={reviewPack} />}

        {CATEGORIES.includes(view as Cat) && (
          <Category
            category={view as Cat}
            tweaks={tweaks}
            pending={pending}
            onChange={change}
          />
        )}

        {view === "history" && (
          <History entries={entries} busy={busy} onRevert={revert} />
        )}
      </main>

      {pendingCount > 0 && (
        <div className="bar" role="status">
          <span>
            {pendingCount === 1
              ? t("changes.pending", { n: 1 })
              : t("changes.pendingPlural", { n: pendingCount })}
          </span>
          <div className="bar__actions">
            <button type="button" className="btn" onClick={() => setPending({})}>
              {t("changes.discard")}
            </button>
            <button type="button" className="btn btn--primary" onClick={review}>
              {t("changes.review")}
            </button>
          </div>
        </div>
      )}

      <PlanDialog
        plan={plan}
        busy={busy}
        onApply={apply}
        onCancel={() => {
          setPlan(null);
          setPendingPack(null);
        }}
      />

      {restartAsk && (
        <Ask
          title={t("restart.title")}
          body={t("restart.body")}
          confirm={t("restart.yes")}
          cancel={t("restart.later")}
          busy={busy}
          onConfirm={async () => {
            setRestartAsk(false);
            try {
              await api.restartExplorer();
            } catch (err) {
              setMessage(String(err));
            }
          }}
          onCancel={() => setRestartAsk(false)}
        />
      )}

      {confirmRevertAll && (
        <Ask
          title={t("home.revertAll")}
          body={t("home.revertAllHint")}
          confirm={t("home.revertAll")}
          cancel={t("plan.cancel")}
          busy={busy}
          onConfirm={revertAll}
          onCancel={() => setConfirmRevertAll(false)}
        />
      )}
    </div>
  );
}

function NavButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      className={`nav__item${active ? " nav__item--on" : ""}`}
      aria-current={active ? "page" : undefined}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

function Ask({
  title,
  body,
  confirm,
  cancel,
  busy,
  onConfirm,
  onCancel,
}: {
  title: string;
  body: string;
  confirm: string;
  cancel: string;
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="scrim" role="presentation" onClick={onCancel}>
      <div
        className="dialog dialog--small"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(e) => e.stopPropagation()}
      >
        <h2>{title}</h2>
        <p className="muted">{body}</p>
        <div className="dialog__actions">
          <button type="button" className="btn" onClick={onCancel} disabled={busy}>
            {cancel}
          </button>
          <button type="button" className="btn btn--primary" onClick={onConfirm} disabled={busy}>
            {confirm}
          </button>
        </div>
      </div>
    </div>
  );
}
