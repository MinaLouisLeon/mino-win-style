/**
 * The HUD's own narrow bridge.
 *
 * It needs three things and nothing else: the mode's settings, a reading of the
 * machine, and to be told when to boot and when to power down. Keeping it
 * separate from `lib/api` is the same call the dock made — an overlay has no
 * business being able to reach the registry.
 */

import { JARVIS_DEFAULTS, type JarvisConfig, type Telemetry } from "../lib/jarvis";

const inTauri = "__TAURI_INTERNALS__" in window;

/** When the stand-in below started counting, so its uptime is an uptime rather
 *  than the seconds since 1970. */
const started = Date.now() / 1000;

/** A plausible machine, so `pnpm dev` in a browser shows a moving HUD to lay
 *  out against rather than a page of zeroes. */
function pretend(): Telemetry {
  const t = Date.now() / 1000;
  const wave = (period: number, phase: number) => (Math.sin(t / period + phase) + 1) / 2;
  return {
    cpu_percent: 8 + wave(3.1, 0) * 46,
    memory_used_bytes: (9.2 + wave(11, 1) * 1.6) * 1024 ** 3,
    memory_total_bytes: 16 * 1024 ** 3,
    disk_used_bytes: 251 * 1024 ** 3,
    disk_total_bytes: 476 * 1024 ** 3,
    net_down_bps: wave(2.3, 2) ** 3 * 2.4e6,
    net_up_bps: wave(1.7, 4) ** 3 * 4e5,
    uptime_seconds: 211_620 + Math.floor(t - started),
    battery: { percent: 86, charging: true },
  };
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke: call } = await import("@tauri-apps/api/core");
  return call<T>(cmd, args);
}

export const hudApi = {
  // `enabled` is forced on outside the app. In the app the HUD draws nothing
  // until the mode is switched on, which is what keeps the window that exists
  // from startup from animating for a mode nobody asked for — but someone who
  // has opened `hud.html` in a browser has asked for it by opening it, and a
  // blank page would be a strange answer.
  config: (): Promise<JarvisConfig> =>
    inTauri
      ? invoke<JarvisConfig>("jarvis_config")
      : Promise.resolve({ ...JARVIS_DEFAULTS, enabled: true }),

  telemetry: (): Promise<Telemetry> =>
    inTauri ? invoke<Telemetry>("jarvis_telemetry") : Promise.resolve(pretend()),
};

/**
 * Subscribes to one Rust event. Resolves to the unsubscribe.
 *
 * Outside the app there is no event source, so this hands back a no-op — which
 * is what leaves the browser preview sitting in the state it starts in.
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
  void invoke("dock_trace", { line: `hud: ${line}` }).catch(() => {});
}
