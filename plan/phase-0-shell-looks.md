# Phase 0 — Shell Looks

**Delivers:** the mechanism. No new desktop, no visible feature — JARVIS mode
becomes *the first entry in a registry of looks* instead of being the registry.

**New Win32:** none.

## Why this comes first

Layer 3 is currently one hardcoded mode. `src-tauri/src/jarvis.rs` holds the
window, the config file, the commands and the event name; `ui/src/lib/jarvis.ts`
holds the client half; `applyJarvisTheme(on: boolean)` takes a boolean because
there is only one thing it can be. Adding a second look today means copying all
of that, and a third means copying it again — with `jarvis.json`,
`phosphor.json` and `cupertino.json` all able to disagree about which one is on.

There is exactly one thing worn at a time. That is a *selection*, not four
booleans, and the type should say so.

## The shape

### One selection, not many toggles

```rust
// src-tauri/src/shell_look.rs   (replaces jarvis.rs)

/// Which Look is worn right now. `None` is plain Fluent — the app as it ships.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellConfig {
    pub active: Option<LookId>,
    /// Preferences that outlive a switch: set them under JARVIS, move to
    /// Phosphor and back, and they are still there.
    pub sound: bool,
    pub telemetry: bool,
    pub address: String,
}
```

`LookId` is a `#[serde(rename_all = "kebab-case")]` enum — `Jarvis`, `Phosphor`,
`Cupertino`, `Yaru`, `Zen` — and **not** a `String`. A config written by a newer
build must not be able to select a look this build does not have, so the field
is `Option<LookId>` and an unparseable value falls back to `None` rather than
failing the whole file.

### A look is a description, not code

```rust
pub struct Look {
    pub id: LookId,
    /// The `data-theme` value. Same string as the CSS block and the enum.
    pub theme: &'static str,
    /// Which of our own surfaces this look draws. Empty is legal — see Zen.
    pub surfaces: &'static [Surface],
    /// The pack *offered* when the look is switched on. Never applied here.
    pub pack_id: Option<&'static str>,
}

pub enum Surface { Overlay, Dock, TopBar }

pub const LOOKS: &[Look] = &[ /* one entry per look */ ];
```

`LOOKS` is the whole registry. A new look is an entry, a CSS block, and — if it
draws one — an overlay component. No new module, no new command, no new event.

### Surfaces are offered, not commandeered

The dock has its own switch on Home and its own `dock.json` today, and a look
must not quietly take it over: someone who turns Cupertino on with the dock
already running should not find their pinned list rearranged, and someone who
turns it off again should not lose their dock.

So `surfaces` drives an **offer**, in the same register as the pack: switching a
look on shows one confirmation saying *this Look also wants the dock and the
menu bar*, with a checkbox, and declining leaves the skin and nothing else. This
is the rule already stated in the `jarvis.rs` module comment — "only the first
two are ours to do without asking" — extended to cover surfaces as well as the
registry.

## Work

### Rust

1. **`src-tauri/src/shell_look.rs`** — new module; `jarvis.rs` goes away. Move
   `create`/`show`/`hide`/`place` across, generalised: `show(app, look)` emits
   `shell-boot` carrying the look id so the page knows which overlay to draw.
   Keep `SHUTDOWN_MS` and the spawned-thread shutdown as they are, and keep the
   `set_ignore_cursor_events` call **and its comment** verbatim — that call is
   the line the whole overlay rests on and a refactor is not the place to
   reword it.
2. **Commands.** `jarvis_config` → `shell_config`; `jarvis_set_enabled(bool)` →
   `shell_set_look(Option<LookId>)`; `jarvis_set_options` → `shell_set_options`;
   `jarvis_telemetry` → `shell_telemetry`. Registered at `src-tauri/src/lib.rs:58`.
3. **The event.** One `shell-look` event carrying `ShellConfig`, replacing
   `jarvis-mode`. Emitted on every change exactly as now — this is how three
   separate pages re-skin without polling.
4. **Migration.** `shell.json` is the new file. On load, when it is absent and
   `%LOCALAPPDATA%\mino-win-style\jarvis.json` is present, read the old one:
   `enabled: true` becomes `active: Some(Jarvis)`, and `sound`, `telemetry` and
   `address` carry across. Write `shell.json` and leave `jarvis.json` alone — a
   user who downgrades gets their mode back rather than an empty desktop.
5. **Startup.** `src-tauri/src/lib.rs:81` reads `JarvisConfig` and shows the HUD
   if enabled. It becomes: read `ShellConfig`, create every surface window
   hidden, then show the ones the active look asks for.

### UI

6. **`ui/src/lib/shell-look.ts`** replaces `lib/jarvis.ts`.
   `applyLookTheme(id: LookId | null)` sets or removes `data-theme` and keeps
   the `colorScheme` line — the comment about WebView2 painting light scrollbars
   over a black page is load-bearing, carry it over.
7. **`watchShellLook`** replaces `watchJarvisMode`, same contract: subscribe
   before the first read, report defaults in a plain browser tab.
8. **`ui/src/hud/Hud.tsx` becomes an overlay host.** It reads the active look
   and renders the matching overlay — `<JarvisOverlay/>` today, `<CrtOverlay/>`
   in Phase 1, nothing at all for a look with no `Overlay` surface. Move the
   current 363 lines into `ui/src/hud/overlays/JarvisOverlay.tsx` unchanged; the
   host owns only the boot/shutdown timing and the switch between them.
9. **`ui/src/components/JarvisPanel.tsx` → `LookPanel.tsx`** — a picker of the
   looks plus "None", with the JARVIS-specific options (sound, greeting,
   address) shown only when the active look uses them. `Home.tsx:117` swaps one
   component for the other.
10. **`ui/src/lib/mock.ts`** grows the same commands so `pnpm --dir ui dev` keeps
    working. Every phase after this one does its layout work there first.

### CSS

11. `:root[data-theme="jarvis"]` at `ui/src/styles.css:810`, and its counterparts
    in `dock.css` and `hud.css`, stay exactly as they are. The attribute value is
    already the look id, which is the whole reason this refactor is cheap. Later
    phases add sibling blocks; none of them edit this one.

### Strings

12. `looks.*` in both locale files gains the picker: a name and a line of
    description per look, plus the surface-offer dialog. Pack names stay in
    `packs/*/manifest.json` — a look's name and its pack's name are the same
    words in two places on purpose, because a pack has to stand alone for
    `mino apply` with no UI running.

## What must not break

- Turning a look off restores Fluent exactly, with nothing to undo. Removing an
  attribute is still the entire mechanism.
- The overlay stays click-through. Nothing in this phase touches that call.
- The HUD never takes focus, so sound still starts from the settings window —
  the only surface with a user gesture behind it. `ui/src/lib/sound.ts` is
  unchanged, but the reason it works now spans looks; keep the comment current.
- A `jarvis.json` written by the current build still produces a JARVIS desktop
  after the upgrade.

## Tests

- `ShellConfig::default()` is `active: None`, `sound: false`.
- A `jarvis.json` of `{"enabled":true,"sound":true}` migrates to `Some(Jarvis)`
  with `sound: true`.
- A `shell.json` naming a look this build does not have parses to `None` rather
  than failing.
- Table test over `LOOKS`: every `theme` matches its `LookId` serialisation and
  every id is unique — so a typo in a new entry fails the build instead of
  producing a look with no CSS.
- Every `pack_id` in `LOOKS` names a folder that exists under `packs/`.

## Done when

`cargo test --workspace` and `pnpm --dir ui build` are green, the JARVIS desktop
behaves exactly as it did before the refactor with the old config file in place,
and `LOOKS` has one entry.

## What shipped, where it differs from the above

Four decisions were made while building it, all in the same direction: do not
write a Look's worth of machinery before the Look that needs it exists.

- **`LookId` has one variant, not five.** A variant with no `LOOKS` entry would
  be selectable with no CSS behind it, so each phase adds its own — and
  `every_look_has_one_entry_and_answers_to_one_name` fails the build if it
  forgets. Forward compatibility comes from `lenient_look` instead: a
  `shell.json` written by a newer build that names `phosphor` parses to `None`
  and keeps every other preference in the file, which is what the plan actually
  wanted from the `Option`.
- **`Surface` has `Overlay` and `Dock`; `TopBar` arrives with Phase 2.**
- **The surface-offer dialog is not built.** JARVIS's only surface is the
  overlay, which is ours to show without asking, so there is nothing to offer
  yet and no way to test one. Cupertino is the first Look that wants the dock;
  the offer lands with it, in Phase 3. The registry already carries `surfaces`,
  and `apply_surfaces` is the single place that acts on it.
- **One command was added that the plan did not list: `shell_looks`.** Without
  it the picker needs its own copy of the registry in TypeScript, and every new
  Look would mean remembering to add it in two places — which is exactly the
  drift this phase exists to remove. `LookId` in `lib/shell-look.ts` still grows
  by a line per Look, because a Look with an overlay needs a component keyed by
  its id; the *list* comes from Rust.
