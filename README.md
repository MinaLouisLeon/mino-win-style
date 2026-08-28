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
pnpm --dir ui build
cargo tauri dev                # needs: cargo install tauri-cli --version "^2"
```

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
  complete MinGW `bin` on `PATH` — on this machine
  `C:\nuwb-toolchain\mingw64\bin` — and it works.

  Two dependencies were dropped before that was understood: `chrono` from
  `mino-core` (it brings in `windows-link`) and `clap`'s `color` feature (it
  brings in `windows-sys` via `anstream`). Neither is required to stay dropped —
  but both should. Losing `chrono` is what makes `mino-core`'s "no OS
  dependency" claim literally true, and a recovery CLI whose output gets piped
  into a log is better off without colour codes.
- **`windres` breaks on spaces in paths**, and this checkout lives under
  `C:\Users\Mina Louis\…`. Building `src-tauri` on GNU therefore needs a
  space-free target directory:

  ```powershell
  $fso = New-Object -ComObject Scripting.FileSystemObject
  $env:CARGO_TARGET_DIR = $fso.GetFolder("$PWD\target").ShortPath
  cargo build -p mino-win-style
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

**Not verified — this is what the VM is for**

- No change has ever been *applied* to a real machine. The write path,
  the journal on disk, the `.reg` backup, the real revert, `WM_SETTINGCHANGE`
  actually repainting the shell, and the Explorer restart are all untested
  outside the fakes.
- The Tauri window has never been opened. It compiles; nobody has watched it
  start.
- Suggested first VM run: `mino --dry-run apply`, then `mino apply`, then
  compare against `mino history` and `mino safe-restore`, checking with
  `reg export` before and after that the revert really is byte-exact.

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
```

`mino safe-restore` deliberately does not depend on the app starting.

## Licence

MIT — see [LICENSE](LICENSE).
