# Phase 1 — Green Phosphor

**Delivers:** the second Look, and the proof that Phase 0 put the seam in the
right place. A CRT terminal desktop: monochrome phosphor green, scanlines, a
curved-glass vignette, and the machine's readouts as a `top`-style table
instead of arcs.

**New Win32:** none. This is a pack, a CSS block and one overlay component —
which is the point. If Phase 0 was right, this phase touches no Rust at all
beyond a single `LOOKS` entry.

## The three layers

### Layer 1 — `packs/phosphor/manifest.json`

Every setting it uses already exists as a tweak, so **no new registry work and
no new Tier review**: `revert_is_exact.rs` covers this pack the day it lands.

| Setting | Value | Why |
| --- | --- | --- |
| `appearance.dark_mode` | `true` | |
| `appearance.accent_color` | `#1F9E1F` | Not the drawn green — see below |
| `appearance.transparency` | `false` | Acrylic blur is the opposite of a phosphor tube; a CRT has one plane |
| `appearance.accent_on_titlebars` | `true` | |
| `appearance.accent_on_start_taskbar` | `true` | |
| `desktop.wallpaper` | `assets/wallpaper.png` | |
| `desktop.wallpaper_fit` | `fill` | |
| `taskbar.auto_hide` | `true` | The trick every Look depends on: the drawn layer only reads as the real one when the real one is out of the way |
| `taskbar.alignment` | `left` | A terminal starts at the left margin |
| `taskbar.widgets` / `task_view` | `false` | |
| `taskbar.search` | `hidden` | |
| `taskbar.seconds_in_clock` | `true` | |
| `start.layout` | `more_pins` | |
| `start.recently_added_apps` / `recommended_files` | `false` | |
| `explorer.show_file_extensions` | `true` | A terminal that hides extensions is lying |
| `explorer.compact_view` | `true` | |
| `explorer.launch_to` | `this_pc` | |

**Two greens, deliberately.** The drawn layer is `#33FF66` — true phosphor,
because it only ever sits on black that we painted. The system accent is
`#1F9E1F`, several stops darker, because Windows puts white text *on* the accent
in Start and on the taskbar, and white on `#33FF66` is unreadable. Same reason
`packs/jarvis/manifest.json` sets `#00A8CC` rather than the `#6fe3ff` the HUD
draws with. Write the reason into the manifest description or it will get
"fixed" later.

**Wallpaper:** a new entry in `tools/make-wallpapers.mjs` — near-black, one very
faint green vignette from the bottom, and a scanline grid at the same period the
overlay uses so the two do not beat against each other. Drawn from geometry like
the others; nothing traced, nothing downloaded.

### Layer 2 — the skin

A `:root[data-theme="phosphor"]` block in `ui/src/styles.css`, and its
counterparts in `dock.css` and `hud.css`, added as siblings to the JARVIS blocks
without editing them.

- One ink colour and its dim variant. No second hue anywhere: warnings and
  errors are the same green at different intensities, plus a blink. A monochrome
  tube has no red, and cheating on that is what makes retro skins look like
  costume rather than machine.
- `--radius: 0`. Right angles everywhere.
- The panel ornament is not a corner bracket but a full hairline box with a
  title notched into the top rule, the way a TUI draws one.
- Type is `--j-mono`'s stack under a different variable name; uppercase for
  labels, sentence case for body — a real terminal is not shouting all the time.

### Layer 3 — `ui/src/hud/overlays/CrtOverlay.tsx`

Full-screen, click-through, and made of four fixed layers plus one table:

1. **Scanlines** — a single `repeating-linear-gradient`, 3px period, ~6% black.
2. **Drift** — that layer translated 3px on a 9-second linear loop. One
   composited transform; no per-frame JavaScript.
3. **Vignette + curvature** — a radial gradient darkening the corners, and a
   very slight `border-radius` on the whole plane so the edges read as glass.
4. **Bloom** — `text-shadow` on the readouts, nothing else.
5. **The readout** — the same `Sampler` data as the HUD, rendered as an aligned
   monospace table: `CPU  14%  ▓▓▁▁▁▁▁▁`, memory, disk, net, uptime, battery.
   Fixed-width bars out of block characters, so nothing reflows as numbers
   change.

**The boot sequence types.** A POST: memory count, disk check, network up, then
a prompt. It reuses the `--boot-ms` timing contract the HUD already has and the
`shell-boot` / `shell-shutdown` events from Phase 0; the shutdown is the CRT
collapsing to a line and then a dot, which is the one effect this look owes the
format.

## Three things to get right

**Cost.** This is a full-screen always-on-top transparent window animating
forever, over everything the user does. Keep it to composited properties —
`transform` and `opacity` only. No `backdrop-filter` (the HUD's 2px blur is
affordable on panels, not on the whole screen), no `filter` on a full-screen
layer, no `requestAnimationFrame` loop for the drift. Watch it in Task Manager
with a video playing before calling the phase done.

**Flicker, carefully.** A CRT flickers, and a full-screen high-frequency
brightness flash is a genuine hazard, not just a taste question. The flicker
here is a shallow opacity modulation — a few percent, over seconds, never a
strobe — and the whole animated set is off under
`@media (prefers-reduced-motion: reduce)`, which leaves a static, perfectly
usable phosphor look.

**Readability over a white document.** The same problem the HUD solved with a
patch of shade under each group of text. Green on transparent disappears over a
white page; the table needs its plate, and the check is the one already in the
README — drive it over a white background as well as a dark one.

## Work

1. `packs/phosphor/manifest.json`, plus `name`/`description` in en and ar.
2. `tools/make-wallpapers.mjs` — the phosphor entry; run it, commit the PNG.
3. `LOOKS` entry: `Phosphor`, theme `"phosphor"`, surfaces `[Overlay]`,
   pack `com.mino.phosphor`.
4. `data-theme="phosphor"` blocks in `styles.css`, `dock.css`, `hud.css`.
5. `ui/src/hud/overlays/CrtOverlay.tsx`, wired into the Phase 0 overlay host.
6. Locale strings in `en.json` and `ar.json`. The readout labels are the one
   place where Arabic should keep the Latin abbreviations (`CPU`, `RAM`) inside
   an otherwise Arabic panel — the table is `dir="ltr"` like the other numeric
   readouts in this app.
7. A swatch: `.look__swatch--phosphor` next to `--jarvis` in `styles.css:375`.

## Done when

Both languages check out in `pnpm --dir ui dev`, the overlay holds under a
playing video without a measurable frame cost, `prefers-reduced-motion` gives a
still version that is still legible, and switching Phosphor → JARVIS → None
leaves no trace of either in the app's own windows.
