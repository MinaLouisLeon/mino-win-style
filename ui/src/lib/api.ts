/**
 * The typed edge of the Rust bridge.
 *
 * These types mirror what `mino-core` serialises. They are hand-written for now
 * — M1 replaces this file with output from `tauri-specta` so the two sides
 * cannot drift. Until then, this file and `crates/mino-core/src/tweak.rs` are
 * changed together.
 */

import { mockApi } from "./mock";
import type { Edge, LookId, LookInfo, ShellConfig, Telemetry } from "./shell-look";

export type {
  DockWish,
  Edge,
  LookId,
  LookInfo,
  ShellConfig,
  Surface,
  Telemetry,
} from "./shell-look";

export type Category = "appearance" | "desktop" | "taskbar" | "start" | "explorer";
export type Tier = "a" | "b" | "c";
export type Privilege = "user" | "elevated";

/** Untagged on the Rust side: `true`, `"#0F62C0"` or `"left"`. */
export type Value = boolean | string;

export type ValueKind =
  | { kind: "bool" }
  | { kind: "color" }
  | { kind: "choice"; choices: string[] }
  | { kind: "path"; extensions: string[] };

/**
 * `note.key` is a translation key (`support.note.<key>`); `note.en` is the
 * English text the Rust side ships so the CLI has something to print and the UI
 * has a fallback for anything not translated yet.
 */
export interface SupportNote {
  key: string;
  en: string;
}

export type Support =
  | { level: "full" }
  | { level: "partial"; note: SupportNote }
  | { level: "unsupported"; note: SupportNote };

export type Refresh =
  | { kind: "none" }
  | { kind: "broadcast"; area: string }
  | { kind: "assoc_changed" }
  | { kind: "cursors" }
  | { kind: "restart_shell" }
  | { kind: "sign_out" };

export interface RegLoc {
  hive: "current_user" | "local_machine" | "classes_root";
  path: string;
  name: string;
}

export type Change =
  | { op: "value"; loc: RegLoc; from: unknown; to: unknown }
  | { op: "key"; hive: string; path: string; from_present: boolean; to_present: boolean };

export interface TweakState {
  id: string;
  category: Category;
  tier: Tier;
  kind: ValueKind;
  privilege: Privilege;
  refresh: Refresh;
  support: Support;
  value: Value | null;
  error: string | null;
}

export interface PlanItem {
  tweak: string;
  from: Value;
  to: Value;
  tier: Tier;
  privilege: Privilege;
  refresh: Refresh;
  changes: Change[];
}

export interface Plan {
  label: string;
  items: PlanItem[];
  skipped: { tweak: string; reason: string; reason_key: string | null }[];
  needs_elevation: boolean;
  needs_shell_restart: boolean;
  needs_sign_out: boolean;
}

export interface JournalEntry {
  id: string;
  when: string;
  label: string;
  status: "pending" | "applied" | "rolled_back" | "reverted";
  os_build: number;
  tweaks: string[];
  changes: Change[];
}

export interface ApplyReport {
  entry: JournalEntry;
  shell_restart_pending: boolean;
  sign_out_pending: boolean;
}

/** A Look: a folder with a manifest and its assets. */
export interface PackSummary {
  id: string;
  dir: string;
  name: Record<string, string>;
  description: Record<string, string>;
  author: string | null;
  settings: number;
  applicable: boolean;
}

/** When the dock is on screen. `hover` keeps it at the bottom edge until the
 *  pointer comes for it, which is what Cupertino wants. */
export type Reveal = "always" | "hover";

/** The dock is our own window, not a Windows setting, so it has its own config. */
export interface DockConfig {
  enabled: boolean;
  pinned: string[];
  icon_size: number;
  reveal: Reveal;
  /** Which edge it lives on. A dock down a side reserves its strip. */
  placement: Edge;
}

/** The bar, likewise. `height` is logical pixels, and it is what gets reserved
 *  out of the desktop — which is why Rust clamps it rather than trusting it. */
export interface TopBarConfig {
  enabled: boolean;
  height: number;
}

export interface OsBuild {
  build: number;
  ubr: number;
  display_version: string;
  product_name: string;
}

export interface Api {
  osInfo(): Promise<OsBuild>;
  listTweaks(): Promise<TweakState[]>;
  planChanges(label: string, settings: Record<string, Value>): Promise<Plan>;
  applyChanges(label: string, settings: Record<string, Value>): Promise<ApplyReport>;
  history(): Promise<JournalEntry[]>;
  revertEntry(id: string): Promise<ApplyReport>;
  revertAll(): Promise<ApplyReport[]>;
  restartExplorer(): Promise<void>;
  journalDir(): Promise<string>;
  listPacks(): Promise<PackSummary[]>;
  planPack(dir: string): Promise<Plan>;
  applyPack(dir: string): Promise<ApplyReport>;
  dockConfig(): Promise<DockConfig>;
  dockSetEnabled(enabled: boolean): Promise<DockConfig>;
  dockSetReveal(hover: boolean): Promise<DockConfig>;
  dockSetPlacement(edge: Edge): Promise<DockConfig>;
  topBarConfig(): Promise<TopBarConfig>;
  topBarSetEnabled(enabled: boolean): Promise<TopBarConfig>;
  shellConfig(): Promise<ShellConfig>;
  /** The registry: every Look this build has. The UI keeps no copy of it. */
  shellLooks(): Promise<LookInfo[]>;
  /** Wears a Look, or takes the current one off with `null`. */
  shellSetLook(id: LookId | null): Promise<ShellConfig>;
  /** Any subset; anything left out keeps the value it has. */
  shellSetOptions(options: {
    sound?: boolean;
    telemetry?: boolean;
    address?: string;
  }): Promise<ShellConfig>;
  shellTelemetry(): Promise<Telemetry>;
}

/**
 * True when running inside the app rather than in a plain browser tab. Without
 * it `pnpm dev` on its own would throw on the first call instead of showing the
 * interface, which is how most of the layout work gets done.
 */
export const inTauri = "__TAURI_INTERNALS__" in window;

function tauriApi(): Api {
  // Imported lazily so a browser tab never loads the Tauri bundle.
  const invoke = async <T,>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
    const { invoke: call } = await import("@tauri-apps/api/core");
    return call<T>(cmd, args);
  };

  return {
    osInfo: () => invoke<OsBuild>("os_info"),
    listTweaks: () => invoke<TweakState[]>("list_tweaks"),
    planChanges: (label, settings) => invoke<Plan>("plan_changes", { label, settings }),
    applyChanges: (label, settings) => invoke<ApplyReport>("apply_changes", { label, settings }),
    history: () => invoke<JournalEntry[]>("history"),
    revertEntry: (id) => invoke<ApplyReport>("revert_entry", { id }),
    revertAll: () => invoke<ApplyReport[]>("revert_all"),
    restartExplorer: () => invoke<void>("restart_explorer"),
    journalDir: () => invoke<string>("journal_dir"),
    listPacks: () => invoke<PackSummary[]>("list_packs"),
    planPack: (dir) => invoke<Plan>("plan_pack", { dir }),
    applyPack: (dir) => invoke<ApplyReport>("apply_pack", { dir }),
    dockConfig: () => invoke<DockConfig>("dock_config"),
    dockSetEnabled: (enabled) => invoke<DockConfig>("dock_set_enabled", { enabled }),
    dockSetReveal: (hover) => invoke<DockConfig>("dock_set_reveal", { hover }),
    dockSetPlacement: (edge) => invoke<DockConfig>("dock_set_placement", { edge }),
    topBarConfig: () => invoke<TopBarConfig>("top_bar_config"),
    topBarSetEnabled: (enabled) => invoke<TopBarConfig>("top_bar_set_enabled", { enabled }),
    shellConfig: () => invoke<ShellConfig>("shell_config"),
    shellLooks: () => invoke<LookInfo[]>("shell_looks"),
    shellSetLook: (id) => invoke<ShellConfig>("shell_set_look", { id }),
    shellSetOptions: (options) => invoke<ShellConfig>("shell_set_options", options),
    shellTelemetry: () => invoke<Telemetry>("shell_telemetry"),
  };
}

export const api: Api = inTauri ? tauriApi() : mockApi;
