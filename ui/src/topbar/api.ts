/**
 * The bar's own narrow bridge.
 *
 * Like the dock's and the overlay's, it is separate from `lib/api` on purpose:
 * a strip across the top of the screen has no business being able to reach the
 * registry.
 *
 * Three of the calls below are the dock's. That is deliberate — `dock_minimize`,
 * `dock_toggle_maximize` and `dock_close` are one line each into `mino-shell`
 * and are not about docks at all; a second identical set of commands under a
 * different prefix would be more to keep true, not less.
 */

import type { Telemetry } from "../lib/shell-look";

export interface AppWindow {
  hwnd: number;
  title: string;
  exe: string;
  minimized: boolean;
  maximized: boolean;
}

export interface TopBarConfig {
  enabled: boolean;
  height: number;
}

const inTauri = "__TAURI_INTERNALS__" in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke: call } = await import("@tauri-apps/api/core");
  return call<T>(cmd, args);
}

/** A plausible machine, so `pnpm dev` in a browser shows a bar to lay out
 *  against rather than a row of dashes. */
function pretend(): Telemetry {
  const t = Date.now() / 1000;
  const wave = (period: number, phase: number) => (Math.sin(t / period + phase) + 1) / 2;
  return {
    cpu_percent: 12,
    memory_used_bytes: 10 * 1024 ** 3,
    memory_total_bytes: 16 * 1024 ** 3,
    disk_used_bytes: 251 * 1024 ** 3,
    disk_total_bytes: 476 * 1024 ** 3,
    net_down_bps: wave(2.3, 2) ** 3 * 2.4e6,
    net_up_bps: wave(1.7, 4) ** 3 * 4e5,
    uptime_seconds: 211_620,
    battery: { percent: 86, charging: true },
  };
}

/** Something to put a name to in a browser tab, where there is no desktop. */
const PRETEND_WINDOW: AppWindow = {
  hwnd: 1,
  title: "Notes",
  exe: "C:\\Windows\\notepad.exe",
  minimized: false,
  maximized: false,
};

export const barApi = {
  foreground: (): Promise<AppWindow | null> =>
    inTauri ? invoke<AppWindow | null>("top_bar_foreground") : Promise.resolve(PRETEND_WINDOW),

  telemetry: (): Promise<Telemetry> =>
    inTauri ? invoke<Telemetry>("shell_telemetry") : Promise.resolve(pretend()),

  minimize: (hwnd: number): Promise<boolean> =>
    inTauri ? invoke<boolean>("dock_minimize", { hwnd }) : Promise.resolve(true),

  toggleMaximize: (hwnd: number): Promise<boolean> =>
    inTauri ? invoke<boolean>("dock_toggle_maximize", { hwnd }) : Promise.resolve(true),

  close: (hwnd: number): Promise<boolean> =>
    inTauri ? invoke<boolean>("dock_close", { hwnd }) : Promise.resolve(true),

  /** Task View, for the Activities button. False when Windows refused, which
   *  is what stops the button pretending it did something. */
  taskView: (): Promise<boolean> =>
    inTauri ? invoke<boolean>("top_bar_task_view") : Promise.resolve(true),

  openSettings: (): Promise<void> =>
    inTauri ? invoke<void>("top_bar_open_settings") : Promise.resolve(),

  quit: (): Promise<void> => (inTauri ? invoke<void>("top_bar_quit") : Promise.resolve()),
};

/**
 * Subscribes to one Rust event. Resolves to the unsubscribe.
 *
 * Outside the app there is no event source, so this hands back a no-op — which
 * leaves the browser preview polling, which is what you want when you are
 * looking at it.
 */
export async function onEvent<T>(
  name: string,
  handler: (payload: T) => void,
): Promise<() => void> {
  if (!inTauri) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(name, (event) => handler(event.payload));
}

/** Reports a broken page the only way a window with no devtools can. */
export function trace(line: string): void {
  if (!inTauri) return;
  void invoke("dock_trace", { line: `topbar: ${line}` }).catch(() => {});
}
