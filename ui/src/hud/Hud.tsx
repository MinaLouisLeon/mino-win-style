/**
 * The overlay window's host.
 *
 * One window, one Look at a time. This owns everything that is the same
 * whichever Look is worn — which overlay is on screen, the power-up and
 * power-down, the clock, and the reading of the machine — and each Look's
 * overlay owns what it draws. See `hud/overlay.ts` for the contract between
 * them.
 *
 * The whole page is click-through: Rust set `WS_EX_TRANSPARENT` on the window,
 * so nothing here is interactive and nothing here takes focus. It is scenery.
 */

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type ComponentType,
} from "react";

import { SHELL_DEFAULTS, type LookId, type ShellConfig, type Telemetry } from "../lib/shell-look";
import { useI18n } from "../i18n";
import { hudApi, onEvent, trace } from "./api";
import { BOOT_MS, DOWN_MS, type OverlayProps, type Phase } from "./overlay";
import { JarvisOverlay } from "./overlays/JarvisOverlay";

/** How often the readouts are re-read. A second is as fast as a number a person
 *  is glancing at can usefully change. */
const POLL_MS = 1_000;

/**
 * The overlay each Look draws, if it draws one.
 *
 * Partial on purpose: a Look with no overlay — Rust never shows this window for
 * one — simply has no entry, and the host renders nothing rather than needing
 * to know about surfaces.
 */
const OVERLAYS: Partial<Record<LookId, ComponentType<OverlayProps>>> = {
  jarvis: JarvisOverlay,
};

export function Hud() {
  const { lang } = useI18n();

  const [config, setConfig] = useState<ShellConfig>(SHELL_DEFAULTS);
  const [phase, setPhase] = useState<Phase>("boot");
  const [telemetry, setTelemetry] = useState<Telemetry | null>(null);
  const [now, setNow] = useState(() => new Date());
  /**
   * Which Look is on screen — not the same thing as which one is worn. It
   * outlives `config.active` going null, because the power-down has to keep
   * drawing the Look it is powering down.
   */
  const [drawn, setDrawn] = useState<LookId | null>(null);

  // Timers that have to be cancelled when a shutdown interrupts a boot, or a
  // boot interrupts a shutdown. Without this a Look switched off and straight
  // back on again ends up with two boots running over each other.
  const timers = useRef<number[]>([]);
  const clearTimers = () => {
    timers.current.forEach(window.clearTimeout);
    timers.current = [];
  };

  /** Runs the power-up, then settles into the ambient display. */
  const runBoot = useCallback(() => {
    clearTimers();
    setPhase("boot");
    timers.current.push(window.setTimeout(() => setPhase("live"), BOOT_MS));
  }, []);

  // Which Look is worn, and what it becomes. `shell-boot` carries the config
  // and arrives just before the window is shown, so the first frame anyone sees
  // is the first frame of the power-up.
  useEffect(() => {
    let live = true;
    const stops: (() => void)[] = [];

    hudApi
      .config()
      .then((loaded) => live && setConfig(loaded))
      .catch((err) => trace(`config failed: ${err}`));

    // Only the config is taken from this; the boot itself is driven by `active`
    // changing below, so there is one path into the sequence rather than two
    // that can race.
    void onEvent<ShellConfig>("shell-boot", (loaded) => setConfig(loaded)).then((stop) =>
      live ? stops.push(stop) : stop(),
    );

    void onEvent("shell-shutdown", () => {
      clearTimers();
      setPhase("down");
    }).then((stop) => (live ? stops.push(stop) : stop()));

    // Sound and telemetry can be changed while the overlay is up.
    void onEvent<ShellConfig>("shell-look", (loaded) => setConfig(loaded)).then((stop) =>
      live ? stops.push(stop) : stop(),
    );

    return () => {
      live = false;
      clearTimers();
      stops.forEach((stop) => stop());
    };
  }, []);

  // The power-up runs when a Look with an overlay is put on, and only then.
  //
  // The window exists from startup whether or not one is worn — it has to, for
  // the reason `shell_look::create` gives — so the page is loaded and running
  // long before anybody asks to see it. Booting on mount instead of on this
  // transition left a hidden window animating and polling the machine once a
  // second for a Look that was switched off.
  useEffect(() => {
    const next = config.active;
    if (!next || !OVERLAYS[next]) return;
    setDrawn(next);
    runBoot();
  }, [config.active, runBoot]);

  // The clock. Ticking every second rather than on a frame timer: the display
  // shows whole seconds, so anything faster is work nobody can see.
  useEffect(() => {
    if (!config.active) return;
    const id = window.setInterval(() => setNow(new Date()), 1_000);
    return () => window.clearInterval(id);
  }, [config.active]);

  // The readouts, only while there is someone to read them. An overlay in the
  // middle of powering down, or with telemetry switched off, polls nothing.
  useEffect(() => {
    if (!config.active || phase !== "live" || !config.telemetry) return;

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
  }, [config.active, phase, config.telemetry]);

  // Nothing at all until a Look is worn, so a hidden window costs a hidden
  // window and not a running animation. `drawn` rather than `config.active` is
  // what keeps the power-down on screen: by then nothing is worn, and the
  // overlay still has an animation to finish.
  const Overlay = drawn ? OVERLAYS[drawn] : undefined;
  if (!Overlay) return null;

  return (
    <div
      className="hud"
      data-phase={phase}
      data-look={drawn}
      lang={lang}
      // The two sequences are timed here, not in the stylesheet, because Rust
      // also needs to know how long the power-down takes — it has to leave the
      // window up until it finishes. One number, three places, no drift.
      style={{ "--boot-ms": `${BOOT_MS}ms`, "--down-ms": `${DOWN_MS}ms` } as CSSProperties}
    >
      <Overlay
        config={config}
        phase={phase}
        telemetry={telemetry}
        now={now}
        bootMs={BOOT_MS}
      />
    </div>
  );
}
