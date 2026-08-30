# Phase 5 — Zen

**Delivers:** the Look that works by taking things away. Warm paper, generous
space, a quiet system accent, and nothing drawn on top of the desktop at all.

**New Win32:** none.

**Depends on:** Phase 0 only. It can be built at any point after the refactor,
and it is the cheapest way to find out whether the framework can express *less*
as well as *more*.

## Why it earns a place

The other four are additive: each one draws a surface and fills the screen with
instrument. A suite of four maximalist dark desktops is one idea in four
palettes. Zen is the counterweight, and it does three useful things at once:

- It is the only **light** Look, which will surface every place the app's CSS
  quietly assumed a dark background since JARVIS landed.
- It is the only Look with `surfaces: []`, which proves a Look does not need a
  drawn layer — the registry entry, the theme and the pack are enough.
- It is the only Look whose pack sets *fewer* settings than the others, which is
  worth demonstrating: a pack is not obliged to have an opinion about
  everything.

## No overlay, and the reason

The obvious Zen overlay is a small clock that fades away while you work and
returns when you stop. It cannot be built here, and the reason is worth writing
down rather than rediscovering:

**The overlay window is click-through** (`WS_EX_TRANSPARENT`), so it receives no
mouse messages at all. "Fades when the pointer moves" is not a thing a
click-through window can know. The alternatives are all worse than the feature:
making it non-click-through would put a full-screen click sink over the desktop,
and a pointer hook to get the events anyway is the kind of thing this project
does not do.

There is a legitimate version of it later — `foreground()` from Phase 2 gives an
"is the user working, and in what" signal without touching the mouse — and if
the clock is ever wanted, that is the way in. Zen ships with no surface, which
is also the most honest reading of the name.

## Layer 1 — `packs/zen/manifest.json`

Deliberately short. Anything not listed here is left exactly as the user has it.

| Setting | Value | Why |
| --- | --- | --- |
| `appearance.dark_mode` | `false` | The one light Look |
| `appearance.accent_color` | `#5E6B5E` | A sage that recedes; white text on it holds |
| `appearance.transparency` | `false` | Blur is busy |
| `appearance.accent_on_titlebars` | `false` | Nothing shouts |
| `appearance.accent_on_start_taskbar` | `false` | |
| `desktop.wallpaper` | `assets/wallpaper.png` | Warm off-white, one soft gradient |
| `desktop.wallpaper_fit` | `fill` | |
| `taskbar.auto_hide` | `true` | |
| `taskbar.alignment` | `center` | |
| `taskbar.widgets`, `task_view` | `false` | |
| `taskbar.search` | `hidden` | |
| `taskbar.seconds_in_clock` | `false` | The exact inversion of the JARVIS pack's line, on purpose |
| `start.recently_added_apps`, `start.recommended_files` | `false` | |

Not set, on purpose: `start.layout`, every `explorer.*` key. Someone who has
arranged their Start menu and their file listing has already made those
decisions, and a Look about calm should not undo them. Put that sentence in the
manifest description — it is the pack's whole argument.

**Wallpaper:** the quietest entry in `tools/make-wallpapers.mjs` — a warm
off-white, one very soft gradient low in the frame, no rings and no grid.

## Layer 2 — the skin

`:root[data-theme="zen"]`, and the first theme block in this app written for a
light background since the default one.

- Paper `#F7F5F0`, ink `#2B2A28`, secondary `#5A574F`, rules at 8% ink. Warm
  greys throughout — a neutral grey next to a warm paper reads as dirty.
- A serif for headings from the system stack, sans for body and controls.
- `--radius: 4px`. No shadows at all; separation comes from space and a hairline.
- Line-height and padding up by roughly a third against the default theme. The
  whole effect is spacing, not colour, and it will look wrong until the spacing
  moves.
- Transitions slower and fewer. Nothing pulses, nothing glows.
- If the user has the dock on, `dock.css` under `data-theme="zen"` is monochrome
  and unmagnified — a row of icons at rest.

**This block is the audit.** Anything in `styles.css`, `dock.css` or a component
that hardcoded a colour rather than reading a variable will show up here as a
dark patch on paper, because JARVIS and Phosphor would both have hidden it. Fix
those at the source — replace the literal with the variable — rather than
patching them inside the Zen block. That is most of the real work in this phase.

## Work

1. `packs/zen/manifest.json` and the generated wallpaper.
2. `LOOKS` entry: `Zen`, theme `"zen"`, surfaces `[]`, pack `com.mino.zen`.
3. `data-theme="zen"` blocks in `styles.css` and `dock.css`.
4. The hardcoded-colour audit described above.
5. Strings in en and ar. Arabic gets a proper serif choice for headings, not the
   Latin serif with an Arabic fallback bolted on — check it in RTL before
   settling on the stack.
6. `.look__swatch--zen`.
7. Confirm the Phase 0 surface-offer dialog handles `surfaces: []` by not
   appearing at all, rather than by appearing empty.

## Done when

Switching to Zen from any other Look leaves nothing dark anywhere in the app,
the dock (if on) is legible against a light wallpaper, `surfaces: []` shows no
offer dialog, both languages are checked, and the pack's Revert restores the
settings it set and demonstrably has not touched Start or Explorer.

## What shipped, where it differs from the above

All of it, and the audit found rather less than the plan expected — which is
itself the finding.

- **`color-scheme` was the real bug, and the plan did not predict it.**
  `applyLookTheme` set it to `dark` for *any* Look, which was true for the four
  that existed and would have painted Zen's scrollbars and form controls for a
  black page over paper. It moved into the theme blocks, beside the palettes,
  and `applyLookTheme` is now one attribute and nothing else. The default
  `:root` gained `color-scheme: light dark`, which it should always have had —
  its palette already followed `prefers-color-scheme`.
- **The hardcoded-colour audit turned up three literals, not a pile.** All three
  were `#fff` painted on the accent — a button label, a badge, a switch thumb —
  and all three now read `--on-accent`. The other literals in the stylesheet are
  either palette definitions inside a media query, the Look swatches (which are
  pictures *of* a Look and should not be variables), or `#000` used as a
  darkening operand, which has no light or dark about it.
- **One thing was fixed in the Zen block rather than at the source.** The brand
  mark is the accent graded into black, which on paper is a blot rather than a
  mark. That is a per-Look aesthetic and not a hidden bug, so Zen sets it flat
  instead of the base rule growing a variable nobody else would use.
- **"Unmagnified" needed TypeScript, not CSS.** The plan put a calm dock in
  `dock.css`; magnification is arithmetic, so `Dock.tsx` holds the cursor at
  `null` under Zen. It is the same one-line shape as the bar's Cupertino
  branch — the only two places a Look reaches past CSS into behaviour.
- **`surfaces: []` needed no work at all.** The Phase 0 offer filters the list
  and shows nothing when it is empty, which was the behaviour asked for; it is
  now exercised by a Look rather than only by argument.
- **Still no verification.** `src-tauri` does not compile here and no browser
  was available, so the one Look whose whole point is how it looks has a
  typechecker behind it and nothing else. The spacing, the serif, the Arabic
  serif stack and the audit's result are all unseen.
