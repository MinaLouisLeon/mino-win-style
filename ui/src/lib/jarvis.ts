/**
 * JARVIS mode, as the three windows see it.
 *
 * The settings window, the dock and the HUD are three separate pages that share
 * nothing but the toolchain, so the mode has to reach all of them. Rust
 * broadcasts one `jarvis-mode` event when it changes and this module is what
 * each page listens with — which is why it imports nothing from `lib/api`: the
 * dock has its own bridge and must not pull the whole settings API in behind a
 * theme switch.
 *
 * The skin itself is one attribute. `data-theme="jarvis"` on `<html>` swaps a
 * block of CSS variables the existing stylesheets already read, so nothing is
 * replaced and turning the mode off restores the Fluent look exactly.
 */

export interface JarvisConfig {
  enabled: boolean;
  sound: boolean;
  telemetry: boolean;
  /** What the greeting calls you. Empty means it says nothing after the hour. */
  address: string;
}

export const JARVIS_DEFAULTS: JarvisConfig = {
  enabled: false,
  sound: false,
  telemetry: true,
  address: "",
};

/** Raw units, straight from `mino-shell`; the page decides how to write them. */
export interface Telemetry {
  cpu_percent: number;
  memory_used_bytes: number;
  memory_total_bytes: number;
  disk_used_bytes: number;
  disk_total_bytes: number;
  net_down_bps: number;
  net_up_bps: number;
  uptime_seconds: number;
  battery: { percent: number; charging: boolean } | null;
}

const inTauri = "__TAURI_INTERNALS__" in window;

/**
 * Puts the skin on or takes it off.
 *
 * Also flips `color-scheme`, without which WebView2 keeps painting scrollbars
 * and form controls in the light palette over a black page.
 */
export function applyJarvisTheme(on: boolean): void {
  const root = document.documentElement;
  if (on) {
    root.setAttribute("data-theme", "jarvis");
    root.style.colorScheme = "dark";
  } else {
    root.removeAttribute("data-theme");
    root.style.colorScheme = "";
  }
}

/**
 * Calls back whenever the mode changes anywhere, and once at the start with
 * what it is now. Returns an unsubscribe.
 *
 * In a plain browser tab there is no Rust to ask, so it reports the defaults and
 * never changes — enough for the layout work that gets done in `pnpm dev`.
 */
export function watchJarvisMode(onChange: (config: JarvisConfig) => void): () => void {
  if (!inTauri) {
    onChange(JARVIS_DEFAULTS);
    return () => {};
  }

  let live = true;
  let stop: (() => void) | null = null;

  void (async () => {
    const [{ invoke }, { listen }] = await Promise.all([
      import("@tauri-apps/api/core"),
      import("@tauri-apps/api/event"),
    ]);

    // Subscribed before the first read, so a change landing between the two is
    // not missed.
    const unlisten = await listen<JarvisConfig>("jarvis-mode", (event) => {
      if (live) onChange(event.payload);
    });
    // The page may already have been torn down by the time the import resolves.
    if (!live) {
      unlisten();
      return;
    }
    stop = unlisten;

    try {
      const config = await invoke<JarvisConfig>("jarvis_config");
      if (live) onChange(config);
    } catch {
      // A window that cannot reach the bridge keeps the look it has rather than
      // flickering back to the default one.
    }
  })();

  return () => {
    live = false;
    stop?.();
  };
}
