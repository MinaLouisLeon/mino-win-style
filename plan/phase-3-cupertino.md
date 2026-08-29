# Phase 3 — Cupertino

**Delivers:** the flagship. Menu bar at the top, dock at the bottom, taskbar out
of the way — the arrangement people recognise from across a room, and the one
thing the README currently admits a Look "cannot do".

**New Win32:** none. Phase 2 spent it all; this phase is a pack, a skin, and the
bar's Cupertino layout.

**Depends on:** Phase 2.

## Layer 1 — the pack

`packs/macos/manifest.json` already exists and is close to right. It needs two
changes and nothing else:

- **`taskbar.auto_hide` is already `true`** — keep it, and note in the
  description that it now matters twice as much: with a reserved strip at the
  top and a dock at the bottom, a permanent taskbar makes three bars on a screen
  built for one.
- **`appearance.transparency: true`** stays. The bar and the dock both use a
  translucent fill, and they read as the same material as the system's own
  surfaces only when acrylic is on elsewhere.

Whether the Look keeps the id `com.mino.macos` or gains a `com.mino.cupertino`
sibling is a naming decision, not an engineering one. Prefer keeping the
existing id: the pack is already shipped, already has a wallpaper, and a second
near-identical pack is a maintenance cost with no user visible in it. The
*Look* is called Cupertino; the *pack* stays macOS, and `LOOKS` maps one to the
other. Say so in the picker so nobody thinks a setting went missing.

## Layer 2 — the skin

`:root[data-theme="cupertino"]` — the one Look here that is not a monospace
instrument panel, and the one that will expose any place where the app's own CSS
assumed the JARVIS direction of travel.

- Segoe UI Variable as it ships, at a slightly wider tracking; no monospace
  outside the numeric readouts.
- `--radius: 10px`, `--radius-lg: 14px`. Generous corners, soft shadow, high
  contrast between layer and background.
- Vibrancy: `backdrop-filter: blur(20px) saturate(180%)` on the bar, the dock
  and dialogs — *only* on those bounded surfaces, never a full-screen layer.
- Buttons keep the system accent, which the pack has just set to `#0A84FF`, so
  the app matches the desktop rather than overriding it. Same rule as JARVIS:
  `--accent` is not set in the theme block.

## Layer 3 — the surfaces

### The menu bar

The Phase 2 strip, laid out for this Look: 26px tall, translucent, with

- **left:** our mark, then the focused application's name in semibold, then the
  window commands (Minimise, Maximise, Close) as a small menu under a chevron.
  Nothing pretends to be the app's own File/Edit/View — see Phase 2 for why that
  line is not negotiable.
- **right:** the status cluster — network, battery, clock — and our own menu.

### The dock

Already built. What it needs for this Look:

1. **A hover-reveal at the screen edge**, so the dock behaves like the thing it
   is imitating: hidden until the pointer reaches the bottom edge, then slides
   up. This is the one genuinely new dock behaviour in the phase, and it is not
   free — a hidden window cannot see the pointer. The cheap, honest version is a
   1px-tall always-on-top trigger window along the bottom edge whose only job is
   to notice `mouseenter` and ask Rust to show the dock; the dock hides again on
   `mouseleave` after a short grace period. It reuses the window plumbing that
   already exists rather than adding a mouse hook, which would be Tier C
   territory in spirit even where it is documented API.
2. **Separator before the trash-equivalent** — or no trash at all. Recycle Bin
   is reachable via `explorer.exe shell:RecycleBinFolder`, which `launch`
   already handles; if it goes in, it goes in as a normal pinned entry.
3. Rounded translucent tray, larger magnification, the running dot the dock
   already draws.

Both are behind `data-theme="cupertino"` in `dock.css` — the dock's geometry is
shared, its dressing is per-Look.

## The honest limits, to write in the README

This Look gets closer than anything else here, and it will still not be macOS:

- Title bars stay Windows title bars, at the top right of every window. There is
  no supported way to move or restyle them, and traffic lights drawn by us on
  someone else's window would be a lie that stops working the moment the window
  moves.
- Alt+Tab is Alt+Tab. Cmd+Tab's application-level switching is not something a
  window on top can provide.
- The Start menu is still the Start menu. A launcher of our own is a possible
  later surface, not part of this phase.

Say this on the Look's card, not only in the README. The gap between what a Look
promises and what Windows allows is exactly where a user's trust in the rest of
the app gets spent.

## Work

1. `LOOKS` entry: `Cupertino`, theme `"cupertino"`, surfaces
   `[TopBar, Dock]`, pack `com.mino.macos`.
2. `data-theme="cupertino"` blocks in `styles.css`, `dock.css`, `topbar.css`.
3. The bar's Cupertino layout in `ui/src/topbar/TopBar.tsx`.
4. Dock edge-reveal: the trigger window in `src-tauri/src/dock.rs`, a
   `reveal` mode in `DockConfig`, and the slide in `dock.css`.
5. Locale strings for the bar's menus and the Look's card, en and ar.
6. `.look__swatch--macos` already exists; add `--cupertino` if the ids diverge.
7. README: the menu bar, the Look, and the limits above.

## RTL

Arabic mirrors the whole bar: app name and commands on the right, status cluster
on the left. The stylesheets already use logical properties (`inset-inline-*`,
`border-inline-*`) throughout — keep to them and this is free. The clock and the
throughput figures stay `dir="ltr"` inside the mirrored layout, like the HUD's
readouts.

## Done when

A maximized window sits between the bar and the dock with nothing overlapping
it, the dock reveals and hides on the bottom edge without flicker, the bar's app
name follows focus and survives being clicked, both languages are checked, and
the Look's card states plainly what it does not do.
