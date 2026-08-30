/**
 * Shell Looks, as the three windows see it.
 *
 * The settings window, the dock and the overlay are three separate pages that
 * share nothing but the toolchain, so which Look is worn has to reach all of
 * them. Rust broadcasts one `shell-look` event when it changes and this module
 * is what each page listens with — which is why it imports nothing from
 * `lib/api`: the dock has its own bridge and must not pull the whole settings
 * API in behind a theme switch.
 *
 * The skin itself is one attribute. `data-theme="<look>"` on `<html>` swaps a
 * block of CSS variables the existing stylesheets already read, so nothing is
 * replaced and taking the Look off restores the Fluent look exactly.
 */

/**
 * The Looks this build has.
 *
 * The list of Looks the picker shows comes from Rust (`shellLooks()`), so this
 * union is not a second copy of the registry — it is the set of names the
 * *pages* know how to draw something for. A Look with an overlay needs a
 * component keyed by its id, so this grows by one line as each Look lands.
 */
export type LookId = "jarvis" | "cupertino" | "yaru";

/** One of our own windows. Mirrors `Surface` in `src-tauri/src/shell_look.rs`. */
export type Surface = "overlay" | "dock" | "top-bar";

/** Which edge a surface lives on. Mirrors `Edge` in `mino-shell`. */
export type Edge = "top" | "bottom" | "left" | "right";

/** How a Look wants the dock, if it is offered one and gets it. */
export interface DockWish {
  edge: Edge;
  hover: boolean;
}

/** A registry entry, straight from Rust. */
export interface LookInfo {
  id: LookId;
  theme: string;
  surfaces: Surface[];
  pack_id: string | null;
  dock: DockWish | null;
}

export interface ShellConfig {
  /** Which Look is worn. `null` is plain Fluent — the app as it ships. */
  active: LookId | null;
  sound: boolean;
  telemetry: boolean;
  /** What the greeting calls you. Empty means it says nothing after the hour. */
  address: string;
}

export const SHELL_DEFAULTS: ShellConfig = {
  active: null,
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
 * Puts a Look on, or takes the current one off with `null`.
 *
 * Also flips `color-scheme`, without which WebView2 keeps painting scrollbars
 * and form controls in the light palette over a black page. Every Look so far
 * is a dark one; when a light Look lands the scheme has to come from the
 * registry entry rather than from the fact that a Look is worn at all.
 */
export function applyLookTheme(id: LookId | null): void {
  const root = document.documentElement;
  if (id) {
    root.setAttribute("data-theme", id);
    root.style.colorScheme = "dark";
  } else {
    root.removeAttribute("data-theme");
    root.style.colorScheme = "";
  }
}

/**
 * Calls back whenever the Look changes anywhere, and once at the start with
 * what it is now. Returns an unsubscribe.
 *
 * In a plain browser tab there is no Rust to ask, so it reports the defaults and
 * never changes — enough for the layout work that gets done in `pnpm dev`.
 */
export function watchShellLook(onChange: (config: ShellConfig) => void): () => void {
  if (!inTauri) {
    onChange(SHELL_DEFAULTS);
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
    const unlisten = await listen<ShellConfig>("shell-look", (event) => {
      if (live) onChange(event.payload);
    });
    // The page may already have been torn down by the time the import resolves.
    if (!live) {
      unlisten();
      return;
    }
    stop = unlisten;

    try {
      const config = await invoke<ShellConfig>("shell_config");
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
