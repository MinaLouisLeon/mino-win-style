/**
 * The bar.
 *
 * What it shows is bounded by what it can actually do. There is no supported
 * way to read another application's menus from outside its process, so there
 * are none here: the name of what you are working in, the three window commands
 * this app genuinely implements, and a status cluster that reads the same
 * `Sampler` the overlay does. A greyed-out File menu that did nothing would be
 * the one dishonest thing in this app.
 *
 * The bar takes clicks, unlike the overlay, and so it can take focus — which is
 * the reason for `held` below.
 */

import { useCallback, useEffect, useState } from "react";

import { watchShellLook, type LookId, type Telemetry } from "../lib/shell-look";
import { useI18n } from "../i18n";
import { clockTime, rate } from "../hud/format";
import { barApi, onEvent, trace, type AppWindow } from "./api";

/** How often we ask what is in front. Fast enough to feel immediate when you
 *  switch application, slow enough to be nothing on a processor graph. */
const FOREGROUND_MS = 250;
/** The status cluster changes once a second at most; so does the clock. */
const POLL_MS = 1_000;

export function TopBar() {
  const { t, lang } = useI18n();

  /**
   * The application the bar is naming.
   *
   * Kept rather than re-read, because clicking the bar makes *us* the
   * foreground window and Rust answers `null` for anything of ours. Without
   * this the name would change to nothing at the exact moment someone looked
   * at it.
   */
  const [held, setHeld] = useState<AppWindow | null>(null);
  const [telemetry, setTelemetry] = useState<Telemetry | null>(null);
  const [now, setNow] = useState(() => new Date());
  const [menuOpen, setMenuOpen] = useState(false);
  /** False while the bar is hidden: a hidden window should ask nothing. */
  const [active, setActive] = useState(true);
  /**
   * Which Look is worn, for the one thing that is layout rather than colour.
   *
   * Cupertino puts the window commands behind a chevron instead of spelling
   * them out, because a menu bar with three buttons shouted at the left of it
   * is not the arrangement it is imitating. Everything else about a Look is a
   * block of CSS variables and never reaches this file.
   */
  const [look, setLook] = useState<LookId | null>(null);
  /** Open only under Cupertino, where the commands live in a menu. */
  const [commandsOpen, setCommandsOpen] = useState(false);

  useEffect(() => watchShellLook((config) => setLook(config.active)), []);

  useEffect(() => {
    let live = true;
    let stop: (() => void) | null = null;
    void onEvent<boolean>("top-bar-active", (on) => {
      if (live) setActive(on);
    }).then((unlisten) => {
      // The page may already have been torn down by the time the import
      // resolves, in which case the subscription is dropped rather than kept.
      if (live) stop = unlisten;
      else unlisten();
    });
    return () => {
      live = false;
      stop?.();
    };
  }, []);

  useEffect(() => {
    if (!active) return;
    let live = true;
    const tick = () => {
      barApi
        .foreground()
        .then((window) => {
          // `null` means our own window is in front. Keep the last real one.
          if (live && window) setHeld(window);
        })
        .catch((err) => trace(`foreground failed: ${err}`));
    };
    tick();
    const id = window.setInterval(tick, FOREGROUND_MS);
    return () => {
      live = false;
      window.clearInterval(id);
    };
  }, [active]);

  useEffect(() => {
    if (!active) return;
    let live = true;
    const tick = () => {
      setNow(new Date());
      barApi
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
  }, [active]);

  // A menu that cannot be dismissed by clicking away from it is a trap, and on
  // a bar that is always on top it is a trap over everything.
  useEffect(() => {
    if (!menuOpen && !commandsOpen) return;
    const close = () => {
      setMenuOpen(false);
      setCommandsOpen(false);
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") close();
    };
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", key);
    };
  }, [menuOpen, commandsOpen]);

  const act = useCallback(
    (run: (hwnd: number) => Promise<unknown>) => () => {
      if (!held) return;
      void run(held.hwnd).catch((err) => trace(`window command failed: ${err}`));
    },
    [held],
  );

  const battery = telemetry?.battery;
  /** Cupertino tucks the window commands away; every other Look spells them
   *  out, which is what a bar with room for them should do. */
  const tucked = look === "cupertino";

  const commands = held && [
    { key: "min", label: t("bar.minimize"), glyph: "\u2212", run: barApi.minimize },
    {
      key: "max",
      label: held.maximized ? t("bar.restore") : t("bar.maximize"),
      glyph: "\u25A1",
      run: barApi.toggleMaximize,
    },
    { key: "close", label: t("bar.close"), glyph: "\u00D7", run: barApi.close },
  ];

  return (
    <div className="bar" lang={lang}>
      <div className="bar__side">
        <span className="bar__mark" aria-hidden="true" />
        <strong className="bar__app">{held ? nameOf(held.exe) : t("bar.desktop")}</strong>

        {commands && !tucked && (
          <span className="bar__commands">
            {commands.map((command) => (
              <button
                key={command.key}
                type="button"
                className={`bar__btn${command.key === "close" ? " bar__btn--close" : ""}`}
                title={command.label}
                onClick={act(command.run)}
              >
                <span aria-hidden="true">{command.glyph}</span>
                <span className="bar__sr">{command.label}</span>
              </button>
            ))}
          </span>
        )}

        {commands && tucked && (
          <span className="bar__menu-wrap">
            <button
              type="button"
              className="bar__btn bar__chevron"
              aria-haspopup="menu"
              aria-expanded={commandsOpen}
              title={t("bar.window")}
              onMouseDown={(event) => event.stopPropagation()}
              onClick={() => setCommandsOpen((open) => !open)}
            >
              <span aria-hidden="true">&#8964;</span>
              <span className="bar__sr">{t("bar.window")}</span>
            </button>

            {commandsOpen && (
              <div
                className="bar__menu bar__menu--start"
                role="menu"
                onMouseDown={(event) => event.stopPropagation()}
              >
                {commands.map((command) => (
                  <button
                    key={command.key}
                    type="button"
                    role="menuitem"
                    className="bar__item"
                    onClick={() => {
                      setCommandsOpen(false);
                      act(command.run)();
                    }}
                  >
                    {command.label}
                  </button>
                ))}
              </div>
            )}
          </span>
        )}
      </div>

      <div className="bar__side bar__side--end">
        {telemetry && (
          <span className="bar__net" dir="ltr" title={t("hud.network")}>
            &darr; {rate(telemetry.net_down_bps)} &uarr; {rate(telemetry.net_up_bps)}
          </span>
        )}

        {battery && (
          <span className="bar__battery" dir="ltr" title={t("hud.power")} data-low={battery.percent <= 20 || undefined}>
            {battery.charging ? "\u26A1" : ""}
            {battery.percent}%
          </span>
        )}

        <span className="bar__clock" dir="ltr">
          {clockTime(now)}
        </span>

        <span className="bar__menu-wrap">
          <button
            type="button"
            className="bar__btn"
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            title={t("bar.menu")}
            // Stopped so the window-level listener that closes the menu does
            // not see the click that opened it.
            onMouseDown={(event) => event.stopPropagation()}
            onClick={() => setMenuOpen((open) => !open)}
          >
            <span aria-hidden="true">&#8943;</span>
            <span className="bar__sr">{t("bar.menu")}</span>
          </button>

          {menuOpen && (
            <div className="bar__menu" role="menu" onMouseDown={(event) => event.stopPropagation()}>
              <button
                type="button"
                role="menuitem"
                className="bar__item"
                onClick={() => {
                  setMenuOpen(false);
                  void barApi.openSettings().catch((err) => trace(`settings failed: ${err}`));
                }}
              >
                {t("bar.settings")}
              </button>
              <button
                type="button"
                role="menuitem"
                className="bar__item"
                onClick={() => {
                  setMenuOpen(false);
                  void barApi.quit().catch((err) => trace(`quit failed: ${err}`));
                }}
              >
                {t("bar.quit")}
              </button>
            </div>
          )}
        </span>
      </div>
    </div>
  );
}

/** `C:\Windows\notepad.exe` -> `Notepad`. Mirrors `display_name` in
 *  `mino-shell`, which is what the dock shows for the same program. */
function nameOf(exe: string): string {
  const stem = exe.split(/[\\/]/).pop() ?? exe;
  const base = stem.replace(/\.exe$/i, "");
  return base.charAt(0).toUpperCase() + base.slice(1);
}
