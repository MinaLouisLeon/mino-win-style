/**
 * What every overlay is handed, and the two durations they all share.
 *
 * The host (`Hud.tsx`) owns the window's whole life — which Look is being
 * drawn, the boot and power-down timing, the clock and the readings — and each
 * Look's overlay owns only what it puts on screen. Keeping the contract in a
 * module of its own is what stops the host and its overlays importing each
 * other in a circle.
 */

import type { ShellConfig, Telemetry } from "../lib/shell-look";

/**
 * Three real states rather than three class names: `boot` runs the power-up
 * once, `live` is the ambient display, and `down` plays the power-off.
 */
export type Phase = "boot" | "live" | "down";

/** How long the power-up runs. Overlays deal their own content across it. */
export const BOOT_MS = 2_600;

/** Kept in step with `SHUTDOWN_MS` in `src-tauri/src/shell_look.rs`, which is
 *  how long Rust leaves the window up after asking for the power-down. */
export const DOWN_MS = 1_400;

export interface OverlayProps {
  /** Preferences, including `telemetry` and the name the greeting uses. */
  config: ShellConfig;
  phase: Phase;
  /** `null` until the first reading lands, and while readouts are switched off. */
  telemetry: Telemetry | null;
  /** Ticked once a second by the host, so every overlay shows the same time. */
  now: Date;
  /** `BOOT_MS`, passed rather than imported so an overlay cannot drift from the
   *  wrapper's animation. */
  bootMs: number;
}
