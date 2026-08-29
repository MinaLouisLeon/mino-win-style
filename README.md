# mino-win-style

Change how Windows 11 looks — accent colour, dark mode, taskbar, Start, File
Explorer — and put every change back exactly as it was.

A Rust core (`mino-core`) drives the registry and the Win32 API directly. The
interface is HTML and TypeScript running in WebView2, hosted by Tauri 2.

> **Status: M0 and most of M1. Builds, tests pass, reads a real machine.**
> 37 tests green, clippy clean at `-D warnings`, and the CLI has been run
> read-only against Windows 11 25H2 (build 26200.8106). Nothing has been
> *applied* to a real machine yet — that is what a VM is for. See
> [What has and has not been verified](#what-has-and-has-not-been-verified).

## Looks, and the ceiling above them

A **Look** is a pack applied in one step: `packs/macos/` sets dark mode, a system
blue accent, its own wallpaper, an auto-hiding taskbar and a Start menu with the
recommendations turned off. It goes through the same confirmation screen as a
single switch, and lands as one journal entry, so one Revert undoes all of it.

What a Look **cannot** do is make Windows look like macOS, and it is worth being
plain about why: this app changes settings Windows exposes, and Windows exposes
no dock, no menu bar, and no way to restyle its own shell chrome. That is a
ceiling in the approach, not a gap in the implementation.

Getting past it means doing what Seelen UI does — not restyling the Windows
shell but **drawing our own on top of it**. That layer is `mino-shell`, and the
first piece of it exists:

## The dock

A macOS-style dock in a window of our own: transparent, undecorated, always on
top, out of the taskbar and out of Alt+Tab. It shows pinned apps and whatever is
running, with a dot under anything with open windows, and magnifies under the
cursor. Clicking brings that app's window forward, or launches it.

None of it is injection. `mino-shell` enumerates top-level windows
(`EnumWindows`, minus tool windows, owned windows and the cloaked ghosts Windows
keeps for suspended Store apps), reads each icon out of its executable with
`PrivateExtractIconsW`, and calls `SetForegroundWindow` to raise one. All
documented API, nothing loaded into another process.

Turn it on from the app's Home screen. Settings live in
`%LOCALAPPDATA%\mino-win-style\dock.json`, which is also where the pinned list
is kept — there is no drag-to-pin yet.

Still to come: pinning from the dock itself, a launcher, per-monitor placement,
and replacing the 1.2-second poll with `SetWinEventHook`.

## The bar

The third surface, and the first that does not sit *on* the desktop but takes a
piece of it: a strip across the top of the primary monitor carrying the name of
whatever you are working in, the window buttons, and a clock.

A dock at the bottom can float, because a maximized window going under it costs
nothing. A bar across the top cannot — a maximized window would put its own
title bar and close button underneath ours, where they cannot be reached. So
Windows is asked to keep the strip through the appbar protocol
(`SHAppBarMessage`), the same one the taskbar uses, and everything maximizes
below it.

**What it cannot show is another application's menus.** There is no supported
way to read the File/Edit/View of an arbitrary window from outside its process,
and this project does not go inside one. So the bar carries the focused
application's name, the three window commands we genuinely implement — minimise,
maximise, close — and our own menu. Greyed-out menus that did nothing would be
the one dishonest thing in this app.

Two things it has to hear about or the reservation quietly stops being true, and
both arrive as window messages rather than as anything that could be polled for:
`TaskbarCreated`, which Explorer broadcasts after a restart having destroyed
every appbar on the way (applying a Look restarts Explorer, so this is a normal
event here, not a rare one), and `ABN_POSCHANGED`, which says our rectangle has
moved. Hearing them means `SetWindowSubclass` on **our own window**, forwarding
everything to `DefSubclassProc` — nothing hooked, nothing injected, no other
process touched.

**The hazard is worth stating plainly.** An appbar that is registered and never
removed leaves a band of dead screen that survives a reboot with nothing on it to
explain itself. Removal is wired to the switch, to the window being destroyed and
to the process exiting; for the case where all three were missed there is
`mino shell-reset`, which does not need the app to start. See
[Recovery](#recovery).

Turn it on from the app's Home screen. Settings live in
`%LOCALAPPDATA%\mino-win-style\topbar.json`.

Still to come: the Cupertino and Yaru layouts that the bar exists for, and
per-monitor placement — like the dock, it is the primary monitor only.

## JARVIS mode

The second thing `mino-shell` draws, and the first that uses all three layers at
once. One switch on the Home screen, and:

- **A HUD over the desktop.** A full-screen, transparent, always-on-top window
  of our own: corner brackets, a turning arc reactor, a scanning sweep, a clock,
  and live readouts of processor, memory, disk, network, battery and uptime. It
  powers up with a boot sequence and powers down like a CRT going out.
- **The same skin on the app and the dock.** `data-theme="jarvis"` over the CSS
  variables the stylesheets already read — cyan on black, monospace readouts,
  hairline brackets on every panel. Nothing is replaced, so switching the mode
  off restores the Fluent look exactly, with nothing to undo.
- **The JARVIS Look, offered.** `packs/jarvis/` sets a cyan accent, dark mode,
  transparency, an arc-reactor wallpaper and a taskbar that gets out of the way.
  It is *offered*, not applied: turning the switch on opens the same
  confirmation screen as any other change, and declining it leaves the overlay
  and the skin running with the desktop untouched.

**The overlay is click-through.** `WS_EX_TRANSPARENT`, set through Tauri's
`set_ignore_cursor_events`, so every click, scroll and hover passes to whatever
is underneath. This is the line the whole feature rests on: a full-screen
always-on-top window without it swallows every click on the desktop, and the
machine becomes unusable in a way that looks exactly like a crash.

Being always on top, it also draws over anything that goes full-screen — a game,
a video, a presentation. That is what "over everything" means; switch it off
first.

### The readouts

`mino-shell::Sampler` reads six documented kernel calls — `GetSystemTimes`,
`GlobalMemoryStatusEx`, `GetDiskFreeSpaceExW`, `GetIfTable2`, `GetTickCount64`
and `GetSystemPowerStatus` — once a second. No new dependency: `sysinfo` would
have brought a second copy of `windows-sys` along to do what those six already
do. Processor load and network throughput are rates, so the sampler keeps the
previous reading; the first tick after a start reports zero and primes itself.

    cargo run -p mino-shell --example telemetry

prints five seconds of it without drawing anything.

### The voice

Off by default, and deliberately: a machine that starts talking on its own in a
meeting is a bug whatever the intent. Switched on, the greeting is spoken with
the SAPI voice Windows already has (through the Web Speech API) and the
interface blips are oscillators drawn as they play. No audio files ship, nothing
is downloaded, and it works offline.

The sound lives in the **settings window**, not the HUD. The HUD is
click-through and never takes focus, so nothing in it can ever be the user
gesture a browser requires before it will play audio — the click that turns the
mode on happens in the settings window, which is the only place that has one.
Rust's `jarvis-mode` event starts both at the same moment, so they stay in step.

Preferences live in `%LOCALAPPDATA%\mino-win-style\jarvis.json`.

## Why it is built this way

- **Reversible by construction.** No change is applied before its previous value
  is written to a journal on disk, together with a `.reg` file you could import
  by hand. `plan()` is pure and never writes; `Engine::apply` is the only path
  to a write in the whole program.
- **Supported mechanisms only.** Documented APIs and per-user registry values
  (Tier A), a small number of undocumented-but-stable keys behind build gating
  and a warning (Tier B). No `uxtheme` patching, no DLL injection into
  `explorer.exe` (Tier C) — a test in `tweaks/mod.rs` fails the build if a Tier C
  setting ever appears in this binary.
- **The UI cannot name a registry key.** There is no `write_value` command. The
  front end can only ask for settings the engine already implements.

## Layout

```
crates/
  mino-core/   engine, tweaks, journal, packs, compatibility gating — no Windows deps
  mino-win/    the only code that calls Win32: registry, shell refresh, OS detection
  mino-cli/    `mino` — headless apply/revert, and the safe-restore path
src-tauri/     Tauri 2 shell: commands, window, bundler config
ui/            React 19 + TypeScript + Vite, English and Arabic with real RTL
                 four pages, one per window: index.html, dock.html, hud.html,
                 topbar.html
packs/         style packs (`manifest.json` + assets)
```

`mino-core` has no dependency on `windows`, so the entire planner, journal and
pack parser are unit-testable on any machine in milliseconds. Everything that
touches the OS sits behind two traits, faked by `MemoryRegistry` in tests.

## Building

```
cargo test -p mino-core        # the part that matters; no Windows APIs involved
cargo clippy --workspace --all-targets -- -D warnings
pnpm --dir ui install
cargo tauri dev                # needs: cargo install tauri-cli --version "^2"
cargo tauri build              # installers, in target/release/bundle
```

> **Build the app with `cargo tauri build`, never `cargo build --release`.**
> A plain cargo build still compiles with `cfg(dev)` set, so the app looks for
> the frontend at `http://localhost:1420` instead of using the embedded copy.
> With no Vite server running you get a window showing "localhost refused to
> connect" — or, for a transparent window like the dock, nothing at all. Both
> look exactly like a broken feature, and neither is.

`pnpm --dir ui dev` runs the interface on its own in a browser at
<http://localhost:1420>, backed by the mock in `ui/src/lib/mock.ts`. Useful for
layout work; it never touches the registry.

### Toolchain notes

The intended target is **MSVC** (`stable-x86_64-pc-windows-msvc`), which needs
the Visual Studio Build Tools with the *Desktop development with C++* workload.
That is what release builds should use.

Everything so far was built on the **GNU** toolchain instead, which works but
needs two things to be known:

- **The `dlltool` bundled with rustup's GNU toolchain has no assembler beside
  it**, so any crate using `raw-dylib` import libraries fails to compile. Put a
  complete MinGW's `bin` directory on `PATH` — one that has `as.exe` next to
  `dlltool.exe` — and it works.

  Two dependencies were dropped before that was understood: `chrono` from
  `mino-core` (it brings in `windows-link`) and `clap`'s `color` feature (it
  brings in `windows-sys` via `anstream`). Neither is required to stay dropped —
  but both should. Losing `chrono` is what makes `mino-core`'s "no OS
  dependency" claim literally true, and a recovery CLI whose output gets piped
  into a log is better off without colour codes.
- **`windres` breaks on spaces in paths.** A Windows user folder with a space in
  it is enough to trigger it. Building `src-tauri` on GNU then needs a
  space-free target directory, which the 8.3 short name provides without moving
  anything:

  ```powershell
  $fso = New-Object -ComObject Scripting.FileSystemObject
  $env:CARGO_TARGET_DIR = $fso.GetFolder("$PWD\target").ShortPath
  cargo build -p mino-win-style
  ```

  The other half of the same problem is `windres` not being on `PATH` at all,
  which fails differently and less helpfully: `tauri-winres` panics with
  `NotAttempted("windres")` from inside a build script, so it reads as a broken
  dependency rather than a missing tool. Both are the same fix — put a complete
  MinGW `bin` on `PATH` before building `src-tauri`:

  ```powershell
  $env:PATH = "C:\path\to\mingw64\bin;$env:PATH"
  ```

  MSVC does not use `windres` at all, so this disappears with the intended
  toolchain.

Icons: `src-tauri/icons/` holds a generated placeholder, because `tauri-build`
refuses to run without `icon.ico`. Regenerate with
`node tools/make-placeholder-icons.mjs`, or replace the lot with
`pnpm tauri icon path\to\logo.png` once there is artwork.

## What has and has not been verified

**Verified**

- 37 tests pass, including apply-then-revert byte-exactness against the
  in-memory registry.
- `cargo clippy -p mino-core -p mino-win -p mino-cli --all-targets -- -D warnings`
  is clean.
- The UI typechecks and builds, and was driven end to end in a browser against
  the mock: both languages, RTL mirroring, the pending-changes bar and the
  confirmation dialog.
- **The Win32 read path works against a real machine.** `mino os` and
  `mino list` read Windows 11 Pro 25H2 (26200.8106) correctly, and
  `mino --dry-run apply` planned 14 registry writes without touching anything.
- `AccentColorTweak` was corrected against the live registry: the accent DWORDs
  carry `0xFF` in the high byte (not `0x00`), the `AccentPalette` ramp puts the
  base at index 3, and the eighth entry is an unrelated colour that we now
  preserve instead of overwriting.
- **The HUD's telemetry reads a real machine.** `cargo run -p mino-shell
  --example telemetry` returns correct processor, memory, disk, network,
  uptime and battery figures on Windows 11 25H2.
- The HUD, the app skin and the dock skin were driven in a browser against the
  mock, in both languages, and checked over a white background as well as a dark
  one — which is what the patch of shade under each group of HUD text is for.

**Not verified — this is what the VM is for**

- No change has ever been *applied* to a real machine. The write path,
  the journal on disk, the `.reg` backup, the real revert, `WM_SETTINGCHANGE`
  actually repainting the shell, and the Explorer restart are all untested
  outside the fakes.
- The Tauri window has never been opened. It compiles; nobody has watched it
  start. That now includes the HUD window: the click-through call, the
  full-screen placement over the taskbar, and whether a transparent
  always-on-top overlay behaves itself in front of real windows are all
  untested outside the browser.
- **Nothing about the bar has run.** `mino-shell`'s half of it compiles and its
  arithmetic is unit-tested, but no `SHAppBarMessage` call in this repository
  has ever been made: whether the strip is granted, whether a maximized window
  really stops below it, whether the subclass sees `TaskbarCreated` when
  Explorer restarts, and whether `mino shell-reset` recovers a reservation left
  behind by a killed process are all untested. The last of those is the one to
  check first, because it is the recovery for every other way this can go
  wrong.
- Suggested first VM run: `mino --dry-run apply`, then `mino apply`, then
  compare against `mino history` and `mino safe-restore`, checking with
  `reg export` before and after that the revert really is byte-exact. Then the
  bar: switch it on, maximize a window against it, kill the app from Task
  Manager, and check `mino shell-reset` gives the strip back.

## Testing

`crates/mino-core/tests/revert_is_exact.rs` is the test that decides whether a
tweak may ship: apply everything, revert everything, and require the registry to
be byte-identical to where it started — including values that were absent before
and must be absent again.

The VM matrix (22H2 / 23H2 / 24H2 / 25H2) described in the project plan is M3
work and is not in the repository yet.

## Recovery

Every applied batch writes to
`%LOCALAPPDATA%\mino-win-style\journal\<id>.json`, plus `<id>.reg` holding the
previous values.

```
mino history                 # what has been changed
mino revert <id>             # undo one batch
mino safe-restore            # undo the most recent batch
mino shell-reset             # give the desktop its full work area back
```

`mino safe-restore` deliberately does not depend on the app starting.

`mino shell-reset` is for one specific failure: the bar reserves a strip at the
top of the screen, and if this program is killed before it can hand that strip
back, the space stays reserved — a band of dead screen with nothing on it to say
why. This clears it. It clears *every* reservation, the taskbar's included, so
windows may maximize under the taskbar until Explorer takes its own back;
restarting Explorer, or signing out and in, does that. It touches no registry
value and no journal entry, and it runs before the engine is even built, so a
broken registry or an unsupported build of Windows cannot stop it.

## Licence

MIT — see [LICENSE](LICENSE).
