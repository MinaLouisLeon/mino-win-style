# Phase 6 — the run sheet

The checklist for the VM run, and a record of what has already been established
without one. [Phase 6](phase-6-verification.md) says *what* to verify and why;
this says *how*, in the order to do it, with what a failure looks like.

Work down it. Tick as you go, and when something fails, write what it did rather
than what it should have done.

---

## Already established, 2026-08-30, on real hardware

This machine is Windows 11 Pro 25H2 (build 26200.8106). Everything below was
run **read-only** — `mino --dry-run apply` builds a plan and returns before
`Engine::apply`, and the journal confirms nothing was written.

- [x] `mino os` — reads the build correctly, reports supported.
- [x] `mino list` — 28 settings read off a live registry.
- [x] `mino history` — empty. Nothing this program has ever done has touched a
      machine, which is what makes the rest of this document necessary.
- [x] **All five packs plan cleanly**, with **no skipped settings** in any of
      them: `jarvis` 19 changes, `macos` 14, `yaru` 18, `zen` 16,
      `midnight-cairo` 14. Every setting every pack names is one this build
      implements and this version of Windows supports. That was previously only
      true against the in-memory fake.
- [x] The accent encoding is right on real values. `#C7431A` plans as
      `AccentColorMenu: 4279911367` = `0xFF1A43C7` — `0xFF` in the high byte and
      BGR below it, exactly as the README's correction says.
- [x] Packs that name a setting the machine already has produce no change for
      it: the JARVIS pack lists `dark_mode: true` and this machine is already
      dark, and the plan says 19 changes rather than 21.

### Findings

**1. The JARVIS pack's accent fails the project's own contrast rule.**

The README argues twice — for Yaru's two oranges and Phosphor's two greens —
that a Look needs a *drawn* colour and a *system* colour, because Windows puts
white text on the accent in Start and on the taskbar, and "the ratio is the
argument". Measured:

| accent | | white on it | |
| --- | --- | ---: | --- |
| Yaru system | `#C7431A` | 4.95:1 | passes AA |
| Yaru drawn | `#E95420` | 3.65:1 | large text only — which is why it is not the system one |
| Zen | `#5E6B5E` | 5.61:1 | passes AA |
| Cupertino | `#0A84FF` | 3.65:1 | large text only; it is Apple's own system blue, used the same way |
| default | `#0F62C0` | 5.95:1 | passes AA |
| **JARVIS** | **`#00A8CC`** | **2.81:1** | **fails, including AA-large** |

So the rule the project wrote down for the newer packs is broken by the oldest
one. `#007E99` is the same hue at 4.73:1 and passes; `#0086A3` is 4.25:1 if the
brighter cyan matters more than the threshold. **Not changed here** — it is a
shipped colour and the call is the author's, not the verifier's.

**2. `taskbar.icon_size` reads as unavailable on 25H2.** Tier B, and no pack
uses it, so nothing in this plan is affected. Worth knowing before someone reads
it as a regression.

---

## Before the VM

- [ ] A toolchain that can build `src-tauri`. The GNU toolchain needs a complete
      MinGW `bin` on `PATH` — one with `as.exe` beside `dlltool.exe`, or
      `raw-dylib` crates fail and `windres` is missing for `tauri-winres`. MSVC
      needs the Build Tools with *Desktop development with C++* and has neither
      problem. See the README's toolchain notes.
- [ ] `cargo tauri build`, **not** `cargo build --release`. A plain cargo build
      still sets `cfg(dev)`, so the app looks for the frontend on
      `localhost:1420`; for a transparent window that shows as nothing at all.
- [ ] **Snapshot the VM.** Two of the tests below deliberately leave the machine
      in a bad state to prove it can be recovered from.
- [ ] Copy `mino.exe` somewhere on `PATH` inside the VM. Several steps need it
      when the app is *not* running, which is the point of it existing.

---

## 1. The write path

Nothing here involves a surface. If this is wrong, nothing after it matters.

- [ ] `reg export HKCU\Control Panel\Desktop before.reg` — and the same for
      `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced`,
      `...\Themes\Personalize`, and `HKCU\Software\Microsoft\Windows\DWM`.
- [ ] `mino --dry-run apply packs\zen\manifest.json` — read the plan. It should
      match what this machine produced above, allowing for a different starting
      state.
- [ ] `mino apply packs\zen\manifest.json`. Expect: an entry id printed, and the
      desktop visibly changing.
- [ ] `mino history` — one entry, status `applied`.
- [ ] `%LOCALAPPDATA%\mino-win-style\journal\<id>.json` exists, and `<id>.reg`
      beside it holds the *previous* values.
- [ ] `mino revert <id>`.
- [ ] `reg export` again and diff against `before.reg`. **Byte-identical, or the
      tweak does not ship.** Values that were absent before must be absent
      again, not present-and-zero.
- [ ] Repeat for `packs\jarvis\manifest.json`, which touches the most.

## 2. The recovery, before anything reserves

Do this *before* the surface tests, not after. It is the way out of the only
failure here that outlives the process, and it is worth knowing it works before
you need it.

- [ ] With nothing of ours running: `mino shell-reset`. Expect the message about
      the work area being the whole screen again, and the taskbar to stop
      reserving its strip until Explorer is restarted. That is the documented
      trade — confirm it is what actually happens.
- [ ] Restart Explorer from Task Manager. The taskbar takes its strip back.

## 3. The overlay — the gate the other surfaces were built on

- [ ] Start the app. The window opens at all. (It never has.)
- [ ] Switch on JARVIS. The HUD appears over the desktop.
- [ ] **Click through it.** Click the desktop, drag an icon, scroll a window
      underneath. Every click lands where it looks like it should. If it does
      not, `set_ignore_cursor_events` did not take, and the machine is
      effectively unusable until the mode is switched off — which is why this is
      the first surface test.
- [ ] It covers the taskbar rather than stopping above it.
- [ ] Switch off: the power-down plays for its full 1.4s and *then* the window
      goes. It should not vanish mid-fade.

## 4. The bar, and the strip it takes

- [ ] Switch the bar on. It sits across the top of the primary monitor.
- [ ] Maximize a window. It stops **below** the bar, not behind it. If it goes
      behind, `ABM_SETPOS` did not reserve.
- [ ] Click the bar. The application name does **not** change to *Mino*.
- [ ] Switch the bar off. The space comes back immediately.
- [ ] **The stuck work area drill.** Bar on, then kill `mino-win-style.exe` from
      Task Manager. Is the strip still reserved — is there a band of dead screen
      at the top? Then `mino shell-reset`, and check it comes back. Do this one
      twice; it is the failure that survives a reboot.
- [ ] **Explorer restarting.** Bar on, then restart Explorer from Task Manager.
      The strip should still be reserved afterwards. This is the single test
      that proves the `TaskbarCreated` handler works, and applying any Look
      triggers the same event, so a failure here is not an edge case.

## 5. Look by Look

For **JARVIS, Cupertino, Yaru, Zen** (and Green Phosphor if Phase 1 has landed):

| | JARVIS | Cupertino | Yaru | Zen |
| --- | --- | --- | --- | --- |
| Skin, surfaces and pack offer appear together | ☐ | ☐ | ☐ | ☐ |
| Decline the pack — skin and surfaces stay, desktop untouched | ☐ | ☐ | ☐ | ☐ |
| Decline the surfaces — skin only, nothing drawn | — | ☐ | ☐ | — |
| Accept the pack — one journal entry, one Revert undoes it all | ☐ | ☐ | ☐ | ☐ |
| Switch straight to another Look, not via None | ☐ | ☐ | ☐ | ☐ |
| Switch off — Fluent returns exactly | ☐ | ☐ | ☐ | ☐ |
| Restart the app with it active — comes back the same | ☐ | ☐ | ☐ | ☐ |
| English and Arabic, mirrored | ☐ | ☐ | ☐ | ☐ |

Zen has no surfaces, so the offer dialog must **not appear at all** for it —
not appear empty.

### Per-Look specifics

- [ ] **Cupertino:** a maximized window sits between the bar and the dock with
      nothing overlapping it. The dock hides, and comes back when the pointer
      reaches the bottom edge — without flickering when the pointer crosses the
      gap between two icons.
- [ ] **Cupertino:** the dock's reveal does not fight the auto-hidden taskbar
      for the same edge. Both should be reachable.
- [ ] **Yaru:** the dock stands down the left and windows maximize **beside**
      it. Two of our surfaces now hold reservations at once — the new thing in
      Phase 4 — so check the bar's strip survives the dock taking its own.
- [ ] **Yaru:** Activities opens Task View.
- [ ] **Yaru:** switch Yaru → Cupertino directly. The dock moves from the left
      to the bottom **and gives the left strip back**. A band down the left
      afterwards is the multi-registration bug.
- [ ] **Yaru in Arabic:** the dock is offered on the right.
- [ ] **Zen:** nothing anywhere in the app is dark. This is the audit; look at
      every page, both languages, and the dock if it is on.
- [ ] **Zen:** its Revert restores what it set and demonstrably has **not**
      touched Start or Explorer — it never sets them.

## 6. The environment

- [ ] **Full screen.** A game, a video, a presentation. Our surfaces draw over
      all of them, as documented — but check nothing breaks the
      exclusive-fullscreen path or forces a game to borderless.
- [ ] **Scaling.** Change display scale with every surface up. Every `place`
      divides by `scale_factor()`; a wrong divisor shows as a bar half off the
      screen.
- [ ] **Resolution**, and undocking a laptop.
- [ ] **A second monitor.** Not supported — primary only — but it must *degrade*
      rather than misplace: surfaces stay on the primary, the secondary's work
      area is untouched.
- [ ] **Remote Desktop and Fast User Switching**, one connect/disconnect cycle
      each. Sessions that lose and regain a desktop are where always-on-top and
      appbar registration historically go wrong.
- [ ] **Cost.** Task Manager with an overlay up and a video playing. Composited
      `transform` and `opacity` only; measurable GPU from a full-screen animated
      layer is this phase's own acceptance test failing.

## 7. The build matrix

22H2 / 23H2 / 24H2 / 25H2 for `mino apply` and `mino revert` — that is where the
build-gated settings live. One build is enough for the drawn layer:
`SHAppBarMessage` and `EnumWindows` have behaved the same for twenty years.

## 8. Afterwards

- [ ] Update *What has and has not been verified* in the README with what
      actually happened, including anything that failed. That section is one of
      the more valuable things in this repository and it is only valuable while
      it is true.
- [ ] Move any failure into a phase of its own rather than patching it here.
