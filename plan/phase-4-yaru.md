# Phase 4 — Yaru

**Delivers:** the GNOME arrangement — a bar across the top and a fixed dock down
the left — in Ubuntu's aubergine and orange.

**New Win32:** none, if Phase 2's appbar module was written to take an edge.
This phase is mostly the dock learning to stand up.

**Depends on:** Phase 2 (the bar), Phase 3 (which shakes the bar's layout out).

## Why this one is cheap

Everything structural already exists by now. The bar is a surface with a
per-Look layout; the dock is a window that places itself. Yaru needs one real
capability the dock has never had — **a vertical placement** — and the rest is a
palette.

## The dock, standing up

`DockConfig` at `src-tauri/src/dock.rs:19` is bottom-only by construction: there
is no placement field, and `place_window` hard-codes centring along the bottom
of the work area.

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Edge { #[default] Bottom, Left, Right }
```

`DockConfig` gains `placement: Edge`, defaulted so every existing `dock.json`
keeps its bottom dock without being rewritten — the `serde(default)` container
attribute on `JarvisConfig` is the pattern to copy.

What follows from it:

- **`place_window` takes the edge.** Bottom centres along X at the bottom of the
  work area; Left centres along Y at `area.x`. The arithmetic is pure and
  belongs in `mino-shell` with a unit test, not inline in a Tauri module.
- **The page measures the other way.** `Dock.tsx` currently reports a width and
  a height for a horizontal strip; it needs to report the box for whichever axis
  it is stacking on. Keep the existing contract — the page measures, Rust
  places — because it is what keeps sizing next to the layout.
- **Magnification follows the cursor along the long axis**, whichever that is.
  One `axis` variable in the existing maths.
- **The running indicator moves to the inner edge** — under the icon on a bottom
  dock, beside it on a left one.
- **`dock.css` gets `flex-direction` from a data attribute**, not a separate
  stylesheet. The logical properties already in there do most of the mirroring.

Ubuntu's dock is *reserved*, not floating: windows maximize beside it, not under
it. That is the Phase 2 appbar module with `ABE_LEFT`, and it is the reason
Phase 2's `register` takes an edge rather than assuming the top. The same
`ABM_REMOVE` discipline and the same `mino shell-reset` recovery apply — a dock
that leaves a dead strip down the left of every screen is exactly as bad as a
bar that leaves one across the top.

## The bar, GNOME-style

Same surface, different arrangement: focused app's name at the left, clock and
date centred, status cluster at the right. Flat black, no translucency, square
corners — the visual opposite of Cupertino's vibrancy from the same component,
which is a decent test that the bar's layout is really per-Look and not
per-hardcode.

**Activities.** The overview has no counterpart we can draw, but Task View is
close and Windows will open it for a synthesised `Win+Tab` through `SendInput` —
documented API, no injection, in Tier A. Either the button opens Task View or it
does not exist. A dead "Activities" that does nothing is the same lie as a greyed
File menu.

## Layer 1 — `packs/yaru/manifest.json`

| Setting | Value | Note |
| --- | --- | --- |
| `appearance.dark_mode` | `true` | |
| `appearance.accent_color` | `#C7431A` | Not the drawn orange — below |
| `appearance.transparency` | `false` | Yaru is flat; acrylic fights it |
| `appearance.accent_on_titlebars` | `true` | |
| `taskbar.auto_hide` | `true` | Two of our bars are on screen; the third has to go |
| `taskbar.alignment` | `left` | |
| `taskbar.widgets`, `task_view`, `search` | off / hidden | Task View lives in the bar now |
| `start.*` | as the other packs | |
| `explorer.show_file_extensions` | `true` | |
| `desktop.wallpaper` | `assets/wallpaper.png` | Aubergine, generated |

**Two oranges, for the reason Phosphor has two greens.** The drawn layer uses
Ubuntu's `#E95420`, because it only ever sits on aubergine we painted. The
system accent is `#C7431A`, because Windows puts white text on the accent in
Start and on the taskbar and `#E95420` does not carry it. Check the contrast
rather than trusting the hex; the ratio is the argument.

**No shipped font.** The Ubuntu typeface is redistributable under its own
licence, but shipping a font binary brings a licence obligation and weight for a
cosmetic gain. Use the system sans stack with the same tracking and weight
choices; Yaru is recognisable by its colour and its layout, not its `g`.

**Wallpaper:** a new `tools/make-wallpapers.mjs` entry — an aubergine field with
one warm glow low and left. Geometry, like the others.

## Work

1. `Edge` on `DockConfig`, `place_window` per edge, the pure placement maths and
   its tests in `mino-shell`.
2. `Dock.tsx` and `dock.css`: axis-aware stacking, magnification and indicator.
3. Appbar registration for the dock when its placement reserves space.
4. Task View via `SendInput` in `mino-shell`, or the button does not ship.
5. `packs/yaru/`, wallpaper generated, strings in en and ar.
6. `LOOKS` entry: `Yaru`, theme `"yaru"`, surfaces `[TopBar, Dock]`, pack
   `com.mino.yaru`.
7. `data-theme="yaru"` blocks in `styles.css`, `dock.css`, `topbar.css`.
8. `.look__swatch--yaru`.

## RTL

A left-hand dock in Arabic belongs on the right. This is the first place where a
Look's *geometry* is a language question rather than a styling one: the edge
comes from the config, but the default offered for a right-to-left interface is
`Right`. Offer it, do not force it — someone who set `Left` meant it.

## Done when

The dock stands down the left edge with windows maximizing beside it, switching
between Yaru and Cupertino moves the dock and re-dresses the bar with no restart
and no leftover reserved space, Task View opens from the bar or the button is
absent, and both languages are checked with the dock on the correct edge for
each.

## What shipped, where it differs from the above

All of it, and the phase was as cheap as predicted — but "cheap" turned out to
mean one thing the plan did not see.

- **The appbar module had to learn to hold more than one strip.** Phase 2 kept a
  single registration for the process, because only the bar reserved. Yaru wears
  a bar *and* a reserving dock, so a second `register` would have overwritten
  the first and left a strip held by nothing that knew how to give it back —
  the exact failure the whole module is careful about. Registrations are now per
  window, with `unregister(hwnd)` and `unregister_all()`.
- **A reserved dock's window is bigger than its reservation.** They are two
  rectangles now. What is reserved is the panel's thickness; the window is wider,
  because a context menu opens beside the icons and has to land somewhere. If
  the window were the reservation, every maximized window on the desktop would
  move a little each time a menu opened. `dock_place` therefore takes the panel
  thickness as well as the window size.
- **`Edge` came from `mino-shell`, not a new enum.** The plan proposed a
  `DockConfig`-local `Edge`; Phase 2 had already put one in `mino-shell` for the
  appbar, and one type for "which side of the screen" is right.
- **How a Look wants the dock lives in the registry**, as `DockWish { edge,
  hover }` on `Look`. It was going to be a table in `App.tsx` keyed by Look id,
  which is exactly the second copy of the registry Phase 0 removed.
- **The dock's edge is a setting on Home**, which the plan did not call for.
  Without it, accepting Yaru's offer would move the dock to the left with no way
  back short of editing `dock.json` — a Look that can strand a preference is
  worse than one that asks for nothing.
- **Still no verification.** `src-tauri` does not compile here and no browser was
  available, so the standing dock, the second reservation and Task View have
  unit-tested arithmetic and a typechecker behind them, and nothing else.
