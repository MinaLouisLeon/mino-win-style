# Phase 6 — The run that makes it true

**Delivers:** nothing new. This is the phase where everything above stops being
"it compiles" and becomes "it was watched working", on a machine that can be
thrown away afterwards.

Most of it cannot be a test. `mino-core` is unit-testable because it has no
dependency on the OS; every surface in this plan exists precisely because it
touches the OS, and the parts that matter — a reserved strip, a click-through
overlay, an Explorer restart — have no fake to stand in for them.

## Standing debt this clears

From the root README's *What has and has not been verified*:

- **No change has ever been applied to a real machine.** The write path, the
  journal on disk, the `.reg` backup, the real revert, `WM_SETTINGCHANGE`
  repainting the shell and the Explorer restart are all untested outside the
  fakes. Every Look in this plan is a batch of registry writes, so this is the
  root of it.
- **The Tauri window has never been opened.** Including the HUD: click-through,
  full-screen placement over the taskbar, and how a transparent always-on-top
  window behaves in front of real ones.

The HUD half is the **gate for Phase 2** and should be done early rather than
saved for here.

## The order

1. `mino --dry-run apply` — no writes, read the plan.
2. `mino apply`, then `mino history`, then `mino revert <id>`.
3. `reg export` before and after; the revert is byte-exact or the tweak does not
   ship. This is the same contract `crates/mino-core/tests/revert_is_exact.rs`
   enforces against the in-memory registry, checked once against a real one.
4. Then the surfaces, Look by Look.

## Per-Look checklist

For each of JARVIS, Green Phosphor, Cupertino, Yaru and Zen:

- [ ] Switch on: skin, surfaces and the pack offer all appear together.
- [ ] Decline the pack offer — the skin and surfaces still work, desktop
      untouched.
- [ ] Accept it — one journal entry, one Revert undoes all of it.
- [ ] Switch to another Look directly, without passing through None.
- [ ] Switch off: Fluent returns exactly, nothing left to undo.
- [ ] Restart the app with the Look active — it comes back the same.
- [ ] Both languages, and the layout mirrored in Arabic.

## The things only a real machine will tell you

**The stuck work area drill.** With the top bar up, kill the process from Task
Manager. Is the strip still reserved? Run `mino shell-reset`. Is it back? Do the
same for Yaru's left-hand dock. This is the worst failure anything here can
produce and the only proof is doing it.

**Explorer restarting.** Applying most Looks restarts Explorer — which is also
the event that destroys every registered appbar. A Look that reserves a strip
and then restarts the shell will lose its reservation *as part of being applied*
unless the bar re-registers on the `TaskbarCreated` broadcast. Apply Cupertino
from a clean desktop and watch what happens to the strip; that single test is
what proves the handler in Phase 2 works.

**Full screen.** A game, a video, a presentation. Always-on-top surfaces draw
over all of them — that is what "over everything" means, and the README says so
— but check that the HUD/bar/dock do not also break the exclusive-fullscreen
path or cause the game to flip to borderless.

**Scale and resolution.** Change display scaling with every surface up. Change
resolution. Undock a laptop. Every `place` in this codebase divides by
`scale_factor()`; this is where a wrong divisor shows up as a bar half off the
screen.

**A second monitor.** Not supported — primary only, for all three surfaces — but
verify it *degrades* rather than misplaces: the bar stays on the primary, the
work area of the secondary is untouched.

**Remote Desktop and Fast User Switching.** Sessions that lose and regain a
desktop are where always-on-top and appbar registration historically go wrong.
One connect/disconnect cycle each.

**Cost, watched.** Task Manager with Phosphor's overlay up and a video playing.
Composited transform and opacity only; if a full-screen animated layer is
costing measurable GPU, that is the phase's own acceptance test failing.

## The build matrix

The project plan's M3 matrix — 22H2 / 23H2 / 24H2 / 25H2 — applies to the packs,
because that is where the build-gated settings live. The surfaces are far less
version-sensitive: `SHAppBarMessage` and `EnumWindows` have behaved the same for
twenty years. Do the full matrix for `mino apply` and `revert`; one build is
enough for the drawn layer.

## Documentation, at the end

- The root README: the top bar under the dock, the four new Looks after JARVIS
  mode, `mino shell-reset` in Recovery, and the verified/not-verified lists
  brought up to date. That section is one of the more valuable things in the
  repository and it only stays valuable if it keeps being true.
- Each Look's card says what it does not do. Cupertino's especially — title
  bars, Alt+Tab, and the Start menu are not going anywhere, and the card is
  where a user finds that out before they are disappointed.

## Status

**Not done, and not doable from here.** This phase needs a machine that can be
thrown away and a build of `src-tauri`, and the session it was started in had
neither: the toolchain on hand cannot compile the Tauri crate, and the machine
was the author's own rather than a VM.

What was done instead was the part that changes nothing — see
[the run sheet](phase-6-runsheet.md), which now carries both halves: what the
read-only pass established on real hardware (every pack plans cleanly against a
live registry, with nothing skipped, which had only ever been true against the
fake), and the ordered checklist for the VM run that is still owed.

One finding came out of it: the JARVIS pack's accent fails the white-on-accent
contrast rule this plan's own Yaru and Phosphor phases argue for. It is written
up in the run sheet and in the README, and deliberately not changed — a shipped
colour is the author's call.
