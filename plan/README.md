# Four more Looks

The plan for taking what JARVIS mode proved and turning it into four more
desktops: **Green Phosphor**, **Cupertino**, **Yaru** and **Zen**.

Read this file first, then the phase you are about to build. Each phase file is
self-contained enough to be picked up cold — what it touches, what it must not
break, and how you know it is done.

## What a Look is made of

JARVIS is three layers stacked, and every Look below is the same three in
different proportions:

| Layer | What it is | Where it lives | Reversible by |
| --- | --- | --- | --- |
| 1. The pack | Registry settings Windows already exposes | `packs/<id>/manifest.json` | The journal — one entry, one Revert |
| 2. The skin | CSS variables under `data-theme="<id>"` | `ui/src/styles.css`, `dock.css`, `hud.css` | Removing an attribute; nothing to undo |
| 3. The surface | Our own windows, drawn on top | `crates/mino-shell`, `src-tauri/src/*.rs`, `ui/src/<surface>/` | Hiding a window |

Only layer 1 writes to the machine, and it goes through `Engine::apply` and the
journal like any single switch. Layers 2 and 3 change nothing outside our own
process, which is why turning a Look off is instant and total.

## The ceiling, stated once

None of this restyles the Windows shell. There is no way to change a title bar,
the Start menu's chrome or Explorer's ribbon from outside `explorer.exe`, and
this project does not go inside it (Tier C is banned by a test in
`crates/mino-core/src/tweak.rs`). What makes a Look convincing is the pairing of
**what we draw** with **what we hide** — the taskbar auto-hide in
`packs/jarvis/manifest.json` is what lets the HUD read as the real interface
rather than as a widget floating over one. Every Look here depends on that same
trick, and every pack below sets it.

## The phases

| Phase | What it delivers | New Win32? | Depends on |
| --- | --- | --- | --- |
| [0](phase-0-shell-looks.md) | **Shell Looks** — JARVIS mode generalised into a registry of looks | No | — |
| [1](phase-1-green-phosphor.md) | **Green Phosphor** — a CRT terminal desktop | No | 0 |
| [2](phase-2-top-bar.md) | **The top bar** — a second `mino-shell` surface, and space reserved for it | Yes: `SHAppBarMessage`, `GetForegroundWindow` | 0 |
| [3](phase-3-cupertino.md) | **Cupertino** — menu bar + the existing dock | No | 2 |
| [4](phase-4-yaru.md) | **Yaru** — top bar + a vertical dock | No, but the dock grows a placement | 2, 3 |
| [5](phase-5-zen.md) | **Zen** — the Look that works by subtraction | No | 0 |
| [6](phase-6-verification.md) | The VM run that makes all of it true | — | 1–5 |

Build them in that order. Phases 1 and 5 are the cheap ones and exist partly to
prove the Phase 0 seam is in the right place before Phase 2 spends real Win32
risk on it.

## Gates

**Before Phase 2, the HUD must have been seen running on a real machine.** As of
this branch it never has — the window compiles and was driven in a browser, but
click-through, always-on-top and full-screen placement over the taskbar are
untested outside the fakes (see *What has and has not been verified* in the
root README). Phase 2 adds a second always-on-top window that, unlike the HUD,
takes clicks and reserves desktop space. Building it on top of three unverified
assumptions means that when it misbehaves there is no way to tell which layer
did it. Phase 6 is the formal run; a smoke test of the HUD in a VM is the gate
for Phase 2 specifically.

**Every phase ends green.** Nothing is merged with:

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm --dir ui build
```

failing, and `crates/mino-core/tests/revert_is_exact.rs` stays the test that
decides whether a setting may ship.

## Standing rules for every phase

- **Two languages or it is not done.** Every string goes in
  `ui/src/locales/en.json` *and* `ar.json`, and the layout is checked in RTL.
  Looks are named in both; `packs/*/manifest.json` carries `name` and
  `description` maps, not strings.
- **Windows are built once, at startup, on the main thread.** Building a webview
  window from a command handler blocks on the event loop: the window appears and
  every line after `build()` silently never runs. Both `dock::create` and
  `jarvis::create` carry the comment; any new surface follows the same shape —
  create hidden in `setup()`, and let the toggle only show and hide.
- **Nothing new goes in Tier C.** No `uxtheme` patching, no injection. If a Look
  seems to need it, the Look is wrong, not the rule.
- **A preferences file that will not parse is not an error.** It falls back to
  the default and the app starts. See `JarvisConfig::load`.
- **Assets are ours.** `tools/make-wallpapers.mjs` draws every wallpaper from
  geometry. Nothing is traced from Apple, Canonical, or a film frame; each new
  Look adds a generator entry rather than a downloaded PNG.
