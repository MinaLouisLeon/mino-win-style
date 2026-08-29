/**
 * The heads-up display.
 *
 * The whole page is click-through — Rust set `WS_EX_TRANSPARENT` on the window
 * — so nothing here is interactive and nothing here takes focus. It is scenery.
 * That is also why it makes no sound: a window that can never be clicked can
 * never earn the user gesture a browser wants before it will play audio, so the
 * voice and the blips live in the settings window instead and the two are
 * started together by the same Rust event. See `lib/sound.ts`.
 *
 * Three phases, and every one of them is a real state rather than a class name:
 * `boot` runs the power-up once, `live` is the ambient display, and `down`
 * plays the power-off. Rust hides the window when `down` has had time to
 * finish; the two durations are commented on both sides.
 */

import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";

import { JARVIS_DEFAULTS, type JarvisConfig, type Telemetry } from "../lib/jarvis";
import { useI18n } from "../i18n";
import { hudApi, onEvent, trace } from "./api";
import { Arc } from "./Arc";
import { bytes, clockTime, duration, percent, rate } from "./format";

type Phase = "boot" | "live" | "down";

/** How long the power-up runs. The lines below are dealt out across it. */
const BOOT_MS = 2_600;
/** Kept in step with `SHUTDOWN_MS` in `src-tauri/src/jarvis.rs`, which is how
 *  long Rust leaves the window up after asking for the power-down. */
const DOWN_MS = 1_400;
/** How often the readouts are re-read. A second is as fast as a number a person
 *  is glancing at can usefully change. */
const POLL_MS = 1_000;

/** Where a use bar stops being information and starts being a warning. */
const FULL = 85;
/** Where a battery does. */
const EMPTY = 20;

/** The power-up checklist. Each is a translation key under `hud.boot.`. */
const BOOT_LINES = ["core", "shell", "registry", "telemetry", "online"] as const;

export function Hud() {
  const { t, lang } = useI18n();

  const [config, setConfig] = useState<JarvisConfig>(JARVIS_DEFAULTS);
  const [phase, setPhase] = useState<Phase>("boot");
  const [telemetry, setTelemetry] = useState<Telemetry | null>(null);
  const [now, setNow] = useState(() => new Date());
  /** How many boot lines have been dealt out so far. */
  const [revealed, setRevealed] = useState(0);

  // Timers that have to be cancelled when a shutdown interrupts a boot, or a
  // boot interrupts a shutdown. Without this a mode switched off and straight
  // back on again ends up with two boots running over each other.
  const timers = useRef<number[]>([]);
  const clearTimers = () => {
    timers.current.forEach(window.clearTimeout);
    timers.current = [];
  };

  /** Runs the power-up, then settles into the ambient display. */
  const runBoot = useCallback(() => {
    clearTimers();
    setRevealed(0);
    setPhase("boot");
    // The lines land across the first two thirds, leaving the last third for
    // the reactor to reach speed before the HUD takes over.
    const step = (BOOT_MS * 0.62) / BOOT_LINES.length;
    BOOT_LINES.forEach((_, index) => {
      timers.current.push(window.setTimeout(() => setRevealed(index + 1), step * (index + 1)));
    });
    timers.current.push(window.setTimeout(() => setPhase("live"), BOOT_MS));
  }, []);

  // What the mode is now, and what it becomes. `jarvis-boot` carries the config
  // and arrives just before the window is shown, so the first frame anyone sees
  // is the first frame of the power-up.
  useEffect(() => {
    let live = true;
    const stops: (() => void)[] = [];

    hudApi
      .config()
      .then((loaded) => live && setConfig(loaded))
      .catch((err) => trace(`config failed: ${err}`));

    // Only the config is taken from this; the boot itself is driven by
    // `enabled` turning true below, so there is one path into the sequence
    // rather than two that can race.
    void onEvent<JarvisConfig>("jarvis-boot", (loaded) => setConfig(loaded)).then((stop) =>
      live ? stops.push(stop) : stop(),
    );

    void onEvent("jarvis-shutdown", () => {
      clearTimers();
      setPhase("down");
    }).then((stop) => (live ? stops.push(stop) : stop()));

    // Sound and telemetry can be changed while the HUD is up.
    void onEvent<JarvisConfig>("jarvis-mode", (loaded) => setConfig(loaded)).then((stop) =>
      live ? stops.push(stop) : stop(),
    );

    return () => {
      live = false;
      clearTimers();
      stops.forEach((stop) => stop());
    };
  }, []);

  // The power-up runs when the mode turns on, and only then.
  //
  // The window exists from startup whether or not JARVIS mode is on — it has to,
  // for the reason `jarvis::create` gives — so the page is loaded and running
  // long before anybody asks to see it. Booting on mount instead of on this
  // transition left a hidden window animating and polling the machine once a
  // second for a mode that was switched off.
  useEffect(() => {
    if (config.enabled) runBoot();
  }, [config.enabled, runBoot]);

  // The clock. Ticking every second rather than on a frame timer: the display
  // shows whole seconds, so anything faster is work nobody can see.
  useEffect(() => {
    if (!config.enabled) return;
    const id = window.setInterval(() => setNow(new Date()), 1_000);
    return () => window.clearInterval(id);
  }, [config.enabled]);

  // The readouts, only while there is someone to read them. A HUD in the middle
  // of powering down, or with telemetry switched off, polls nothing.
  useEffect(() => {
    if (!config.enabled || phase !== "live" || !config.telemetry) return;

    let live = true;
    const tick = () => {
      hudApi
        .telemetry()
        .then((reading) => live && setTelemetry(reading))
        .catch((err) => trace(`telemetry failed: ${err}`));
    };
    tick();
    const id = window.setInterval(tick, POLL_MS);
    return () => {
      live = false;
      window.clearInterval(id);
    };
  }, [config.enabled, phase, config.telemetry]);

  // Nothing at all while the mode is off, so a hidden window costs a hidden
  // window and not a running animation. The power-down is the exception: it has
  // to keep drawing after `enabled` goes false, until Rust takes the window.
  if (!config.enabled && phase !== "down") return null;

  const greeting = t(`hud.greeting.${greetingKey(now)}`);
  const address = config.address ? t("hud.address", { name: config.address }) : "";

  return (
    <div
      className="hud"
      data-phase={phase}
      lang={lang}
      // The two sequences are timed here, not in the stylesheet, because Rust
      // also needs to know how long the power-down takes — it has to leave the
      // window up until it finishes. One number, three places, no drift.
      style={{ "--boot-ms": `${BOOT_MS}ms`, "--down-ms": `${DOWN_MS}ms` } as CSSProperties}
    >
      {/* Backdrop: a faint grid and a slow sweep, both purely decorative and
          both behind everything that carries information. */}
      <div className="hud__grid" aria-hidden="true" />
      <div className="hud__sweep" aria-hidden="true" />
      <div className="hud__vignette" aria-hidden="true" />

      <Frame />

      <div className="hud__reactor">
        {/* Still at the start of the boot and up to speed by the end of it,
            which is what makes the power-up read as a spin-up. */}
        <Arc size={340} speed={phase === "boot" ? 0.08 : 1} />
      </div>

      <header className="hud__head">
        <div className="hud__mark">
          {/* A Latin acronym ending in a full stop: in Arabic the bidi
              algorithm moves that trailing stop to the front and it reads
              ".J.A.R.V.I.S". Same rule as the build numbers and hex colours in
              the settings window. */}
          <strong dir="ltr">J.A.R.V.I.S.</strong>
          <span>{t("hud.subtitle")}</span>
        </div>
        <div className="hud__clock" dir="ltr">
          <strong>{clockTime(now)}</strong>
          <span>{dateLine(now, lang)}</span>
        </div>
      </header>

      {phase === "boot" ? (
        <ol className="hud__boot" aria-hidden="true">
          {BOOT_LINES.slice(0, revealed).map((line) => (
            <li key={line}>
              <span>{t(`hud.boot.${line}`)}</span>
              <b>{t("hud.boot.ok")}</b>
            </li>
          ))}
        </ol>
      ) : (
        config.telemetry && <Readouts telemetry={telemetry} t={t} />
      )}

      <footer className="hud__foot">
        <p className="hud__status">
          <span className="hud__caret" aria-hidden="true">
            &rsaquo;
          </span>
          {greeting}
          {address}. {t("hud.nominal")}
        </p>
        {telemetry && (
          <p className="hud__uptime" dir="ltr">
            {t("hud.uptime")} {duration(telemetry.uptime_seconds)}
          </p>
        )}
      </footer>
    </div>
  );
}

/** The four corner brackets and the hairlines between them. */
function Frame() {
  return (
    <div className="hud__frame" aria-hidden="true">
      <i className="hud__corner hud__corner--tl" />
      <i className="hud__corner hud__corner--tr" />
      <i className="hud__corner hud__corner--bl" />
      <i className="hud__corner hud__corner--br" />
    </div>
  );
}

/**
 * The live column. `null` while the first reading is still in flight, which is
 * about a second on a cold start — the bars render at zero rather than the
 * column popping into existence, so nothing jumps.
 */
function Readouts({
  telemetry,
  t,
}: {
  telemetry: Telemetry | null;
  t: (key: string) => string;
}) {
  const memory = telemetry ? percent(telemetry.memory_used_bytes, telemetry.memory_total_bytes) : 0;
  const disk = telemetry ? percent(telemetry.disk_used_bytes, telemetry.disk_total_bytes) : 0;

  const cpu = telemetry?.cpu_percent ?? 0;
  const battery = telemetry?.battery;

  return (
    <section className="hud__readouts">
      <Bar label={t("hud.cpu")} value={cpu} alert={cpu >= FULL} />
      <Bar
        label={t("hud.memory")}
        value={memory}
        alert={memory >= FULL}
        note={telemetry ? `${bytes(telemetry.memory_used_bytes)} / ${bytes(telemetry.memory_total_bytes)}` : ""}
      />
      <Bar
        label={t("hud.disk")}
        value={disk}
        alert={disk >= FULL}
        note={telemetry ? `${bytes(telemetry.disk_used_bytes)} / ${bytes(telemetry.disk_total_bytes)}` : ""}
      />

      <div className="hud__net" dir="ltr">
        <span className="hud__net-label">{t("hud.network")}</span>
        <span>&darr; {rate(telemetry?.net_down_bps ?? 0)}</span>
        <span>&uarr; {rate(telemetry?.net_up_bps ?? 0)}</span>
      </div>

      {battery && (
        <Bar
          label={t("hud.power")}
          value={battery.percent}
          // A battery is the one row where a full bar is good news and an empty
          // one is the warning — and where being plugged in means neither.
          alert={battery.percent <= EMPTY && !battery.charging}
          note={battery.charging ? t("hud.charging") : ""}
        />
      )}
    </section>
  );
}

/**
 * One labelled bar.
 *
 * `alert` is passed in rather than worked out from the value, because what
 * counts as bad depends on the row: a processor at 90% is working hard and a
 * battery at 90% is fine.
 *
 * The fill is a `transform: scaleX` rather than a width so it animates on the
 * compositor: six of these updating every second on top of a transparent
 * full-screen window is exactly the case where laying out again is felt.
 */
function Bar({
  label,
  value,
  alert,
  note,
}: {
  label: string;
  value: number;
  alert?: boolean;
  note?: string;
}) {
  const clamped = Math.min(100, Math.max(0, value));
  return (
    <div className="bar" data-high={alert || undefined}>
      <div className="bar__head">
        <span className="bar__label">{label}</span>
        <span className="bar__value" dir="ltr">
          {clamped.toFixed(0)}%
        </span>
      </div>
      <div className="bar__track">
        <div className="bar__fill" style={{ transform: `scaleX(${clamped / 100})` }} />
      </div>
      {note && (
        <span className="bar__note" dir="ltr">
          {note}
        </span>
      )}
    </div>
  );
}

/** Which greeting the hour calls for. Mirrors `greetingKey` in `lib/sound.ts`,
 *  which picks the words that are spoken; both read the same clock. */
function greetingKey(now: Date): "morning" | "afternoon" | "evening" {
  const hour = now.getHours();
  if (hour < 12) return "morning";
  if (hour < 18) return "afternoon";
  return "evening";
}

/** `SAT 29 AUG`, in the display language. `Intl` already knows both, and knows
 *  that Arabic wants a different order — which is the whole reason not to
 *  assemble this from parts by hand. */
function dateLine(now: Date, lang: string): string {
  try {
    return new Intl.DateTimeFormat(lang === "ar" ? "ar-EG" : "en-GB", {
      weekday: "short",
      day: "numeric",
      month: "short",
    })
      .format(now)
      .toUpperCase();
  } catch {
    return now.toDateString().toUpperCase();
  }
}
