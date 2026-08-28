import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";

import { PlanDialog } from "./components/PlanDialog";
import { Home } from "./routes/Home";
import { Category } from "./routes/Category";
import { History } from "./routes/History";
import { Looks } from "./routes/Looks";
import { useI18n } from "./i18n";
import {
  api,
  inTauri,
  type Category as Cat,
  type JournalEntry,
  type OsBuild,
  type PackSummary,
  type Plan,
  type TweakState,
  type Value,
} from "./lib/api";

type View = "home" | "looks" | Cat | "history";

const CATEGORIES: Cat[] = ["appearance", "desktop", "taskbar", "start", "explorer"];

export default function App() {
  const { t, lang, setLang } = useI18n();

  const [view, setView] = useState<View>("home");
  const [os, setOs] = useState<OsBuild | null>(null);
  const [tweaks, setTweaks] = useState<TweakState[]>([]);
  const [entries, setEntries] = useState<JournalEntry[]>([]);
  const [journalDir, setJournalDir] = useState("");
  const [packs, setPacks] = useState<PackSummary[]>([]);
  /** Set while a Look is waiting in the confirmation dialog. */
  const [pendingPack, setPendingPack] = useState<string | null>(null);

  const [pending, setPending] = useState<Record<string, Value>>({});
  const [plan, setPlan] = useState<Plan | null>(null);
  const [busy, setBusy] = useState(false);
  const [restartAsk, setRestartAsk] = useState(false);
  const [confirmRevertAll, setConfirmRevertAll] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const reload = useCallback(async () => {
    const [osInfo, list, history, dir, looks] = await Promise.all([
      api.osInfo(),
      api.listTweaks(),
      api.history(),
      api.journalDir(),
      api.listPacks(),
    ]);
    setOs(osInfo);
    setTweaks(list);
    setEntries(history);
    setJournalDir(dir);
    setPacks(looks);
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
