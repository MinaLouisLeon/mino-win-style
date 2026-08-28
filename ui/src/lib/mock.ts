/**
 * A stand-in for the Rust side, used when the UI runs in a plain browser tab.
 *
 * It exists so the interface can be built and reviewed without a Rust toolchain
 * installed. It mirrors the tweak list in `crates/mino-core/src/tweaks/`, and
 * when the two disagree the Rust side is right.
 */

import type {
  Api,
  ApplyReport,
  Category,
  JournalEntry,
  Plan,
  Support,
  Tier,
  TweakState,
  Value,
  ValueKind,
} from "./api";

interface Def {
  id: string;
  category: Category;
  tier: Tier;
  kind: ValueKind;
  value: Value;
  support?: Support;
  restartShell?: boolean;
}

const bool = (): ValueKind => ({ kind: "bool" });
const choice = (...choices: string[]): ValueKind => ({ kind: "choice", choices });

const defs: Def[] = [
  { id: "appearance.dark_mode", category: "appearance", tier: "a", kind: bool(), value: true },
  { id: "appearance.accent_color", category: "appearance", tier: "b", kind: { kind: "color" }, value: "#0F62C0" },
  { id: "appearance.transparency", category: "appearance", tier: "a", kind: bool(), value: true },
  { id: "appearance.accent_on_titlebars", category: "appearance", tier: "a", kind: bool(), value: false },
  {
    id: "appearance.accent_on_start_taskbar",
    category: "appearance",
    tier: "a",
    kind: bool(),
    value: false,
    support: { level: "partial", note: { key: "dark_mode_only", en: "Windows only shows this while dark mode is on." } },
  },

  { id: "taskbar.alignment", category: "taskbar", tier: "a", kind: choice("left", "center"), value: "center" },
  { id: "taskbar.widgets", category: "taskbar", tier: "b", kind: bool(), value: true },
  { id: "taskbar.task_view", category: "taskbar", tier: "a", kind: bool(), value: true },
  {
    id: "taskbar.search",
    category: "taskbar",
    tier: "a",
    kind: choice("hidden", "icon", "box", "icon_and_label"),
    value: "box",
  },
  {
    id: "taskbar.icon_size",
    category: "taskbar",
    tier: "b",
    kind: choice("small", "medium", "large"),
    value: "medium",
    support: { level: "unsupported", note: { key: "changed_in_later_build", en: "Windows changed this setting in a later build." } },
  },
  { id: "taskbar.seconds_in_clock", category: "taskbar", tier: "a", kind: bool(), value: false },
  {
    id: "taskbar.end_task",
    category: "taskbar",
    tier: "a",
    kind: bool(),
    value: false,
    support: { level: "partial", note: { key: "dev_end_task", en: "Mirrors the End Task switch in Settings for developers." } },
  },

  {
    id: "start.layout",
    category: "start",
    tier: "b",
    kind: choice("default", "more_pins", "more_recommendations"),
    value: "default",
  },
  { id: "start.recently_added_apps", category: "start", tier: "a", kind: bool(), value: true },
  { id: "start.recommended_files", category: "start", tier: "a", kind: bool(), value: true },

  { id: "explorer.show_file_extensions", category: "explorer", tier: "a", kind: bool(), value: false },
  { id: "explorer.show_hidden_files", category: "explorer", tier: "a", kind: bool(), value: false },
  {
    id: "explorer.show_protected_os_files",
    category: "explorer",
    tier: "a",
    kind: bool(),
    value: false,
    support: {
      level: "partial",
      note: {
        key: "system_files",
        en: "Shows system files. Useful for troubleshooting, easy to break things with.",
      },
    },
  },
  {
    id: "explorer.launch_to",
    category: "explorer",
    tier: "a",
    kind: choice("home", "this_pc", "downloads"),
    value: "home",
  },
  { id: "explorer.compact_view", category: "explorer", tier: "a", kind: bool(), value: false },
  {
    id: "explorer.classic_context_menu",
    category: "explorer",
    tier: "b",
    kind: bool(),
    value: false,
    restartShell: true,
  },
];

const current = new Map<string, Value>(defs.map((d) => [d.id, d.value]));
const entries: JournalEntry[] = [];

const wait = <T,>(value: T): Promise<T> =>
  new Promise((resolve) => setTimeout(() => resolve(value), 120));

function states(): TweakState[] {
  return defs.map((def) => ({
    id: def.id,
    category: def.category,
    tier: def.tier,
    kind: def.kind,
    privilege: "user",
    refresh: def.restartShell ? { kind: "restart_shell" } : { kind: "broadcast", area: "ImmersiveColorSet" },
    support: def.support ?? { level: "full" },
    value: def.support?.level === "unsupported" ? null : (current.get(def.id) as Value),
    error: null,
  }));
}

function buildPlan(label: string, settings: Record<string, Value>): Plan {
  const items: Plan["items"] = [];
  const skipped: Plan["skipped"] = [];

  for (const [id, want] of Object.entries(settings)) {
    const def = defs.find((d) => d.id === id);
    if (!def) {
      skipped.push({
        tweak: id,
        reason: `This build of the app does not know the setting \`${id}\`.`,
        reason_key: "unknown_setting",
      });
      continue;
    }
    if (def.support?.level === "unsupported") {
      skipped.push({ tweak: id, reason: def.support.note.en, reason_key: def.support.note.key });
      continue;
    }
    const from = current.get(id) as Value;
    if (from === want) continue;
    items.push({
      tweak: id,
      from,
      to: want,
      tier: def.tier,
      privilege: "user",
      refresh: def.restartShell ? { kind: "restart_shell" } : { kind: "broadcast", area: "ImmersiveColorSet" },
      changes: [
        {
          op: "value",
          loc: {
            hive: "current_user",
            path: "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced",
            name: id.split(".")[1] ?? id,
          },
          from,
          to: want,
        },
      ],
    });
  }

  return {
    label,
    items,
    skipped,
    needs_elevation: false,
    needs_shell_restart: items.some((i) => i.refresh.kind === "restart_shell"),
    needs_sign_out: false,
  };
}

export const mockApi: Api = {
  osInfo: () =>
    wait({ build: 26200, ubr: 8106, display_version: "25H2", product_name: "Windows 11 Pro (mock)" }),

  listTweaks: () => wait(states()),

  planChanges: (label, settings) => wait(buildPlan(label, settings)),

  applyChanges: (label, settings) => {
    const plan = buildPlan(label, settings);
    for (const item of plan.items) current.set(item.tweak, item.to);

    const entry: JournalEntry = {
      id: String(Date.now()),
      when: new Date().toISOString(),
      label,
      status: "applied",
      os_build: 26200,
      tweaks: plan.items.map((i) => i.tweak),
      changes: plan.items.flatMap((i) => i.changes),
    };
    entries.unshift(entry);
    return wait<ApplyReport>({
      entry,
      shell_restart_pending: plan.needs_shell_restart,
      sign_out_pending: false,
    });
  },

  history: () => wait([...entries]),

  revertEntry: (id) => {
    const entry = entries.find((e) => e.id === id);
    if (entry) {
      entry.status = "reverted";
      for (const change of entry.changes) {
        if (change.op === "value") current.set(entryTweakFor(entry, change), change.from as Value);
      }
    }
    return wait<ApplyReport>({
      entry: entry ?? entries[0],
      shell_restart_pending: false,
      sign_out_pending: false,
    });
  },

  revertAll: () => {
    const reports = entries
      .filter((e) => e.status === "applied")
      .map((e) => {
        e.status = "reverted" as const;
        return { entry: e, shell_restart_pending: false, sign_out_pending: false };
      });
    for (const def of defs) current.set(def.id, def.value);
    return wait(reports);
  },

  restartExplorer: () => wait(undefined),

  journalDir: () => wait("C:\\Users\\you\\AppData\\Local\\mino-win-style\\journal"),
};

/** The mock keeps one change per tweak, so position is enough to pair them up. */
function entryTweakFor(entry: JournalEntry, change: JournalEntry["changes"][number]): string {
  return entry.tweaks[entry.changes.indexOf(change)] ?? entry.tweaks[0];
}
